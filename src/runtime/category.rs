//! `Category` implementation for the ``OxideC`` runtime.
//!
//! This module implements **Categories**, which allow adding methods to existing
//! classes at runtime without subclassing. Categories are inspired by Objective-C's
//! category system.
//!
//! # Architecture
//!
//! Categories are **globally registered** and never deallocated:
//! - Each category name maps to exactly one ``Category`` instance per class
//! - Categories have `'static` lifetime (live for program duration)
//! - Immutable after creation (except method addition)
//! - Methods are added to the target class's method resolution chain
//!
//! # Method Resolution Order
//!
//! When looking up a method, the runtime searches in this order:
//! 1. Class's own methods
//! 2. Category methods (in order of addition)
//! 3. Superclass methods
//!
//! # Thread Safety
//!
//! Categories are thread-safe and support concurrent method addition from
//! multiple threads. Uses `RwLock` for method table protection.

use crate::error::{Error, Result};
use crate::runtime::{Class, Method, RuntimeString, Selector, get_global_arena};
use std::collections::HashMap;
use std::fmt;
use std::ptr::NonNull;
use std::sync::RwLock;

/// Internal category data stored in global arena.
///
/// This struct is allocated in the global arena and never deallocated.
#[repr(C)]
pub(crate) struct CategoryInner {
    /// Category name (e.g., "NSStringExtensions")
    name: RuntimeString,
    /// Methods in this category: selector hash -> Method
    /// Protected by `RwLock` for thread-safe method addition
    pub(crate) methods: RwLock<HashMap<u64, Method>>,
    /// Class this category extends
    associated_class: NonNull<crate::runtime::class::ClassInner>,
}

/// `Category` represents a collection of methods added to an existing class.
///
/// Categories allow you to add methods to existing classes without subclassing,
/// which is useful for:
/// - Adding functionality to system classes
/// - Organizing code into logical groups
/// - Distributing class extensions across multiple modules
///
/// # Memory Management
///
/// Categories use manual memory management:
/// - Allocated in global arena (stable pointers)
/// - Never deallocated (live for program duration)
/// - Methods are stored in the category's method table
///
/// # Thread Safety
///
/// Categories are `Send + Sync`:
/// - Methods table protected by `RwLock`
/// - Category metadata is immutable after construction
///
/// # Example
///
/// ```rust
/// use oxidec::{Category, Class, Method, RuntimeString, Selector};
/// use oxidec::runtime::get_global_arena;
/// use std::str::FromStr;
///
/// // Create a class
/// let class = Class::new_root("MyClass").unwrap();
///
/// // Create a category
/// let category = Category::new("MyCategory", &class).unwrap();
///
/// // Add a method to the category
/// let sel = Selector::from_str("categoryMethod").unwrap();
/// let arena = get_global_arena();
/// let method = Method {
///     selector: sel.clone(),
///     imp: my_method_impl,
///     types: RuntimeString::new("v@:", arena),
/// };
/// category.add_method(method).unwrap();
///
/// // Method is now available on instances of MyClass
/// # unsafe extern "C" fn my_method_impl(
/// #     _self: oxidec::runtime::object::ObjectPtr,
/// #     _cmd: oxidec::runtime::selector::SelectorHandle,
/// #     _args: *const *mut u8,
/// #     _ret: *mut u8,
/// # ) {}
/// ```
pub struct Category {
    /// Pointer to category data in global arena.
    /// Never null, valid for entire program lifetime.
    inner: NonNull<CategoryInner>,
}

unsafe impl Send for Category {}
unsafe impl Sync for Category {}

impl Category {
    /// Creates a new category for a class.
    ///
    /// # Arguments
    ///
    /// * `name` - Category name (must be unique per class)
    /// * `class` - The class to extend
    ///
    /// # Returns
    ///
    /// Returns `Ok(Category)` if successful, `Err` if category name already
    /// exists for this class.
    ///
    /// # Thread Safety
    ///
    /// Multiple threads can create categories concurrently. The class ensures
    /// unique category names.
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::{Category, Class};
    ///
    /// let class = Class::new_root("MyClass").unwrap();
    /// let category = Category::new("MyExtensions", &class).unwrap();
    /// assert_eq!(category.name(), "MyExtensions");
    /// ```
    ///
    /// # Errors
    ///
    /// Returns `Err(Error::CategoryAlreadyExists)` if a category with this
    /// name already exists for the given class.
    pub fn new(name: &str, class: &Class) -> Result<Self> {
        // SAFETY: class.inner points to valid ClassInner
        let class_inner = unsafe { &*class.inner.as_ptr() };

        // Check for duplicate category names
        {
            let categories_lock = class_inner.categories.read().unwrap();
            for cat_ptr in categories_lock.iter() {
                let cat = unsafe { &*cat_ptr.as_ptr() };
                if cat.name.as_str().ok() == Some(name) {
                    return Err(Error::CategoryAlreadyExists);
                }
            }
        }

        // Allocate CategoryInner in global arena
        let arena = get_global_arena();
        let name_str = RuntimeString::new(name, arena);

        let category_inner = CategoryInner {
            name: name_str,
            methods: RwLock::new(HashMap::new()),
            associated_class: class.inner,
        };

        // Allocate in arena
        let ptr = arena.alloc(category_inner);
        if ptr.is_null() {
            return Err(Error::OutOfMemory);
        }
        let inner = unsafe { NonNull::new_unchecked(ptr as *mut CategoryInner) };

        // Register with class
        {
            let mut categories_lock = class_inner.categories.write().unwrap();
            categories_lock.push(inner);
        }

        // Invalidate class's method cache
        class.invalidate_cache();

        Ok(Category { inner })
    }

    /// Adds a method to this category.
    ///
    /// # Arguments
    ///
    /// * `method` - Method to add
    ///
    /// # Thread Safety
    ///
    /// Multiple threads can add methods concurrently.
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::{Category, Class, Method, RuntimeString, Selector};
    /// use oxidec::runtime::get_global_arena;
    /// use std::str::FromStr;
    ///
    /// let class = Class::new_root("MyClass").unwrap();
    /// let category = Category::new("MyCategory", &class).unwrap();
    ///
    /// let sel = Selector::from_str("myMethod").unwrap();
    /// let arena = get_global_arena();
    /// let method = Method {
    ///     selector: sel.clone(),
    ///     imp: my_impl,
    ///     types: RuntimeString::new("v@:", arena),
    /// };
    ///
    /// category.add_method(method).unwrap();
    /// # unsafe extern "C" fn my_impl(
    /// #     _self: oxidec::runtime::object::ObjectPtr,
    /// #     _cmd: oxidec::runtime::selector::SelectorHandle,
    /// #     _args: *const *mut u8,
    /// #     _ret: *mut u8,
    /// # ) {}
    /// ```
    pub fn add_method(&self, method: Method) -> Result<()> {
        // SAFETY: self.inner points to valid CategoryInner
        let inner = unsafe { &*self.inner.as_ptr() };

        let hash = method.selector.hash();

        // Add method to category's method table
        {
            let mut methods = inner.methods.write().unwrap();
            methods.insert(hash, method);
        }

        // Invalidate associated class's method cache
        // Reconstruct Class from ClassInner pointer
        let class = Class {
            inner: inner.associated_class,
        };
        class.invalidate_cache();

        Ok(())
    }

    /// Returns the category name.
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::{Category, Class};
    ///
    /// let class = Class::new_root("MyClass").unwrap();
    /// let category = Category::new("MyCategory", &class).unwrap();
    /// assert_eq!(category.name(), "MyCategory");
    /// ```
    #[must_use]
    pub fn name(&self) -> &str {
        // SAFETY: self.inner points to valid CategoryInner
        let inner = unsafe { &*self.inner.as_ptr() };
        inner.name.as_str().unwrap_or("<invalid>")
    }

    /// Looks up a method in this category by selector.
    ///
    /// # Arguments
    ///
    /// * `selector` - Method selector to look up
    ///
    /// # Returns
    ///
    /// Returns `Some(&Method)` if found, `None` otherwise.
    #[must_use]
    pub(crate) fn lookup_method(&self, selector: &Selector) -> Option<&Method> {
        // SAFETY: self.inner points to valid CategoryInner
        let inner = unsafe { &*self.inner.as_ptr() };
        let methods = inner.methods.read().unwrap();
        let hash = selector.hash();

        if let Some(method) = methods.get(&hash) {
            // SAFETY: The method is in the arena and never deallocated
            // We're creating a reference with extended lifetime
            return unsafe { Some(&*std::ptr::from_ref::<Method>(method)) };
        }

        None
    }
}

impl fmt::Debug for Category {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // SAFETY: self.inner points to valid CategoryInner
        let inner = unsafe { &*self.inner.as_ptr() };
        let methods = inner.methods.read().unwrap();

        f.debug_struct("Category")
            .field("name", &inner.name.as_str().unwrap_or("<invalid>"))
            .field("method_count", &methods.len())
            .finish()
    }
}

impl Clone for Category {
    fn clone(&self) -> Self {
        Category { inner: self.inner }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::Selector;
    use std::str::FromStr;

    // Test method implementation
    unsafe extern "C" fn test_method_noop(
        _self: crate::runtime::object::ObjectPtr,
        _cmd: crate::runtime::selector::SelectorHandle,
        _args: *const *mut u8,
        _ret: *mut u8,
    ) {
    }

    #[test]
    fn test_category_creation() {
        let class = Class::new_root("CategoryTestClass").unwrap();
        let cat = Category::new("MyCategory", &class).unwrap();
        assert_eq!(cat.name(), "MyCategory");
    }

    #[test]
    fn test_category_add_method() {
        let class = Class::new_root("CategoryAddMethod").unwrap();
        let cat = Category::new("AddMethodCat", &class).unwrap();
        let sel = Selector::from_str("categoryMethod:").unwrap();

        let method = Method {
            selector: sel.clone(),
            imp: test_method_noop,
            types: RuntimeString::new("v@:i", get_global_arena()),
        };

        cat.add_method(method).unwrap();
        assert!(cat.lookup_method(&sel).is_some());
    }

    #[test]
    fn test_category_method_resolution() {
        // Test that category methods are found during lookup
        let class = Class::new_root("CategoryResolution").unwrap();
        let cat = Category::new("ResolutionCat", &class).unwrap();
        let sel = Selector::from_str("catMethod").unwrap();

        let method = Method {
            selector: sel.clone(),
            imp: test_method_noop,
            types: RuntimeString::new("v@:", get_global_arena()),
        };

        cat.add_method(method).unwrap();

        // Method should be found via class lookup
        assert!(class.lookup_method(&sel).is_some());
    }

    #[test]
    fn test_duplicate_category() {
        let class = Class::new_root("DupCategoryClass").unwrap();
        Category::new("DupCat", &class).unwrap();

        let result = Category::new("DupCat", &class);
        assert!(matches!(result, Err(Error::CategoryAlreadyExists)));
    }

    #[test]
    fn test_multiple_categories() {
        // Test multiple categories on same class
        let class = Class::new_root("MultiCategory").unwrap();
        let cat1 = Category::new("Cat1", &class).unwrap();
        let cat2 = Category::new("Cat2", &class).unwrap();

        let sel1 = Selector::from_str("method1").unwrap();
        let sel2 = Selector::from_str("method2").unwrap();

        cat1.add_method(Method {
            selector: sel1.clone(),
            imp: test_method_noop,
            types: RuntimeString::new("v@:", get_global_arena()),
        }).unwrap();

        cat2.add_method(Method {
            selector: sel2.clone(),
            imp: test_method_noop,
            types: RuntimeString::new("v@:", get_global_arena()),
        }).unwrap();

        assert!(class.lookup_method(&sel1).is_some());
        assert!(class.lookup_method(&sel2).is_some());
    }

    #[test]
    fn test_category_debug() {
        let class = Class::new_root("CategoryDebugClass").unwrap();
        let cat = Category::new("DebugCat", &class).unwrap();
        let debug_str = format!("{:?}", cat);
        assert!(debug_str.contains("DebugCat"));
    }

    #[test]
    fn test_category_clone() {
        let class = Class::new_root("CategoryCloneClass").unwrap();
        let cat1 = Category::new("CloneCat", &class).unwrap();
        let cat2 = cat1.clone();
        assert_eq!(cat1.name(), cat2.name());
    }
}
