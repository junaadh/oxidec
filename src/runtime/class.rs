//! Class creation and inheritance for the `OxideC` runtime.
//!
//! This module implements the class system with:
//! - Class registration and metadata
//! - Single inheritance with cycle detection
//! - Method registration and lookup
//! - Inheritance chain walking
//!
//! # Architecture
//!
//! Classes are **globally registered** and never deallocated:
//! - Each class name maps to exactly one `Class` instance
//! - Classes have `'static` lifetime (live for program duration)
//! - Immutable after creation (except method addition)
//! - Single inheritance chain to root class (OxObject)
//!
//! # Thread Safety
//!
//! The class registry is thread-safe and supports concurrent class creation
//! from multiple threads. Uses `RwLock` for registry access and method table
//! protection.

use crate::error::{Error, Result};
use crate::runtime::{get_global_arena, RuntimeString, Selector};
use std::collections::HashMap;
use std::fmt;
use std::ptr::NonNull;
use std::sync::RwLock;
use std::sync::OnceLock;

/// Internal class data stored in global arena.
///
/// This struct is allocated in the global arena and never deallocated.
#[repr(C)]
pub(crate) struct ClassInner {
    /// Class name (e.g., "OxObject", "MutableArray")
    name: RuntimeString,
    /// Superclass pointer (null for root class OxObject)
    super_class: Option<NonNull<ClassInner>>,
    /// Method table: selector hash -> Method
    /// Protected by RwLock for thread-safe method addition
    methods: RwLock<HashMap<u64, Method>>,
    /// Class flags (reserved for future use)
    flags: u32,
}

/// Global class registry.
///
/// Ensures unique class names and provides fast lookup by name.
struct ClassRegistry {
    /// Map of class name -> Class pointer
    /// Protected by RwLock for thread-safe class registration
    classes: RwLock<HashMap<RuntimeString, NonNull<ClassInner>>>,
}

// SAFETY: ClassRegistry is Send + Sync because:
// - ClassInner pointers point to arena memory (never deallocated)
// - RwLock provides synchronized access
// - Arena ensures proper alignment and validity
unsafe impl Send for ClassRegistry {}
unsafe impl Sync for ClassRegistry {}

/// Global class registry instance.
static REGISTRY: OnceLock<ClassRegistry> = OnceLock::new();

/// Method representation (placeholder for Phase 2).
///
/// In Phase 1, Method is a minimal placeholder. In Phase 2 (Dispatch),
/// it will include function pointers and implementation details.
#[derive(Clone, Debug)]
pub struct Method {
    /// Method selector
    pub selector: Selector,
    /// Reserved for future implementation pointer
    pub _imp: (),
    /// Reserved for future encoding/type info
    pub _types: (),
}

/// Class represents a runtime class definition with methods and inheritance.
///
/// Classes are **globally registered** and never deallocated. They provide:
/// - Class metadata (name, superclass, methods)
/// - Inheritance chain walking
/// - Method lookup by selector
///
/// # Memory Management
///
/// Classes use manual memory management:
/// - Allocated in global arena (stable pointers)
/// - Never deallocated (live for program duration)
/// - Immutable after creation (except method addition)
///
/// # Thread Safety
///
/// Classes are `Send + Sync`:
/// - Methods table protected by RwLock
/// - Class metadata is immutable after construction
///
/// # Example
///
/// ```rust
/// use oxidec::Class;
///
/// // Create root class
/// let root = Class::new_root("MyRootClass").unwrap();
///
/// // Create subclass
/// let subclass = Class::new("MySubclass", &root).unwrap();
///
/// assert!(subclass.is_subclass_of(&root));
/// ```
pub struct Class {
    /// Pointer to class data in global arena.
    /// Never null, valid for entire program lifetime.
    pub(crate) inner: NonNull<ClassInner>,
}

impl Class {
    /// Creates a new root class (no superclass).
    ///
    /// # Arguments
    ///
    /// * `name` - Class name (must be unique in runtime)
    ///
    /// # Returns
    ///
    /// Returns `Ok(Class)` if name is unique, `Err` if class already exists.
    ///
    /// # Thread Safety
    ///
    /// Multiple threads can create classes concurrently. Registry ensures
    /// unique class names.
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::Class;
    ///
    /// let root = Class::new_root("MyRootClass").unwrap();
    /// assert!(root.super_class().is_none());
    /// ```
    pub fn new_root(name: &str) -> Result<Self> {
        Self::create_class(name, None)
    }

    /// Creates a new class with a superclass.
    ///
    /// # Arguments
    ///
    /// * `name` - Class name (must be unique)
    /// * `super_class` - Superclass to inherit from
    ///
    /// # Returns
    ///
    /// Returns `Ok(Class)` if successful, `Err` on:
    /// - Class name already exists
    /// - Inheritance cycle detected
    ///
    /// # Thread Safety
    ///
    /// Multiple threads can create classes concurrently.
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::Class;
    ///
    /// let root = Class::new_root("Root").unwrap();
    /// let child = Class::new("Child", &root).unwrap();
    ///
    /// assert!(child.is_subclass_of(&root));
    /// ```
    pub fn new(name: &str, super_class: &Class) -> Result<Self> {
        // Check for inheritance cycles
        Self::check_inheritance_cycle(name, super_class)?;

        Self::create_class(name, Some(super_class))
    }

    /// Internal helper to create a class.
    fn create_class(name: &str, super_class: Option<&Class>) -> Result<Self> {
        // Initialize registry on first use
        let registry = REGISTRY.get_or_init(|| {
            // Pre-allocate with some capacity
            let classes = HashMap::with_capacity(64);
            ClassRegistry {
                classes: RwLock::new(classes),
            }
        });

        // Allocate class name in arena
        let arena = get_global_arena();
        let name_str = RuntimeString::new(name, arena);

        // Check if class already exists
        {
            let classes = registry.classes.read().unwrap();
            if classes.contains_key(&name_str) {
                return Err(Error::ClassAlreadyExists);
            }
        } // Release read lock

        // Create ClassInner
        let super_ptr = super_class.map(|sc| sc.inner);
        let class_inner = ClassInner {
            name: name_str,
            super_class: super_ptr,
            methods: RwLock::new(HashMap::new()),
            flags: 0,
        };

        // Allocate in global arena
        let inner_ptr: *mut ClassInner = arena.alloc(class_inner);

        // SAFETY: inner_ptr is not null and properly aligned (arena ensures)
        let inner_nn = NonNull::new(inner_ptr).expect("Arena allocation returned null");

        // Register in global registry
        {
            let mut classes = registry.classes.write().unwrap();

            // Double-check: Another thread might have created it while we waited
            let name_check = RuntimeString::new(name, arena);
            if classes.contains_key(&name_check) {
                return Err(Error::ClassAlreadyExists);
            }

            // SAFETY: inner_ptr is valid and will never be deallocated
            classes.insert(name_check, inner_nn);
        }

        Ok(Class { inner: inner_nn })
    }

    /// Checks for inheritance cycles when creating a subclass.
    ///
    /// Walks the superclass chain to ensure we're not creating a cycle.
    fn check_inheritance_cycle(new_class_name: &str, super_class: &Class) -> Result<()> {
        let mut current_ptr = Some(super_class.inner.as_ptr());

        while let Some(ptr) = current_ptr {
            // SAFETY: ptr points to valid ClassInner in arena
            let inner = unsafe { &*ptr };

            // Check if we found the new class name in the superclass chain
            // unwrap() is safe because RuntimeString in arena is always valid
            if inner.name.as_str().unwrap() == new_class_name {
                return Err(Error::InheritanceCycle);
            }

            // Move to superclass
            current_ptr = inner.super_class.map(|nn| nn.as_ptr());
        }

        Ok(())
    }

    /// Returns the class name.
    ///
    /// # Returns
    ///
    /// String slice with the class name.
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::Class;
    ///
    /// let class = Class::new_root("MyClass").unwrap();
    /// assert_eq!(class.name(), "MyClass");
    /// ```
    #[must_use]
    pub fn name(&self) -> &str {
        // SAFETY: self.inner points to valid ClassInner in arena
        // unwrap() is safe because RuntimeString in arena is always valid
        unsafe { &(*self.inner.as_ptr()).name }.as_str().unwrap()
    }

    /// Returns the superclass (if any).
    ///
    /// # Returns
    ///
    /// - `Some(Class)` if this class has a superclass
    /// - `None` if this is a root class (OxObject)
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::Class;
    ///
    /// let root = Class::new_root("Root").unwrap();
    /// assert!(root.super_class().is_none());
    ///
    /// let child = Class::new("Child", &root).unwrap();
    /// assert!(child.super_class().is_some());
    /// ```
    #[must_use]
    pub fn super_class(&self) -> Option<Class> {
        // SAFETY: self.inner points to valid ClassInner in arena
        let inner = unsafe { &*self.inner.as_ptr() };

        // Return a new Class wrapping the superclass pointer
        inner.super_class.map(|ptr| Class { inner: ptr })
    }

    /// Adds a method to the class.
    ///
    /// # Arguments
    ///
    /// * `method` - Method to add
    ///
    /// # Thread Safety
    ///
    /// Multiple threads can add methods concurrently. RwLock ensures
    /// synchronized access to the methods table.
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::{Class, Selector, Method};
    ///
    /// let class = Class::new_root("MyClass").unwrap();
    /// let sel = Selector::from_str("doSomething:").unwrap();
    ///
    /// let method = Method {
    ///     selector: sel.clone(),
    ///     _imp: (),
    ///     _types: (),
    /// };
    ///
    /// class.add_method(method).unwrap();
    /// ```
    pub fn add_method(&self, method: Method) -> Result<()> {
        // SAFETY: self.inner points to valid ClassInner
        let inner = unsafe { &*self.inner.as_ptr() };

        let mut methods = inner.methods.write().unwrap();
        let hash = method.selector.hash();

        methods.insert(hash, method);

        Ok(())
    }

    /// Looks up a method by selector (searches inheritance chain).
    ///
    /// # Arguments
    ///
    /// * `selector` - Method selector to lookup
    ///
    /// # Returns
    ///
    /// - `Some(&Method)` if found in this class or ancestor
    /// - `None` if not found in entire inheritance chain
    ///
    /// # Note
    ///
    /// This walks the inheritance chain from the current class up to the root.
    /// In Phase 2, method caching will be added for O(1) lookup.
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::{Class, Selector, Method};
    ///
    /// let parent = Class::new_root("Parent").unwrap();
    /// let sel = Selector::from_str("inheritedMethod").unwrap();
    ///
    /// let method = Method {
    ///     selector: sel.clone(),
    ///     _imp: (),
    ///     _types: (),
    /// };
    ///
    /// parent.add_method(method).unwrap();
    ///
    /// let child = Class::new("Child", &parent).unwrap();
    ///
    /// // Method found in parent
    /// assert!(child.lookup_method(&sel).is_some());
    /// ```
    #[must_use]
    pub fn lookup_method(&self, selector: &Selector) -> Option<&Method> {
        let mut current_ptr = Some(self.inner.as_ptr());

        while let Some(ptr) = current_ptr {
            // SAFETY: ptr points to valid ClassInner
            let inner = unsafe { &*ptr };

            // Try to find method in this class
            let methods = inner.methods.read().unwrap();
            let hash = selector.hash();

            if let Some(method) = methods.get(&hash) {
                // Found! Return reference with extended lifetime
                // SAFETY: The method is in the arena and never deallocated
                // We're creating a reference with the same lifetime as &self
                return unsafe {
                    Some(&*(method as *const Method))
                };
            }

            // Not found, try superclass
            current_ptr = inner.super_class.map(|nn| nn.as_ptr());
        }

        None
    }

    /// Checks if this class inherits from the given class.
    ///
    /// # Arguments
    ///
    /// * `class` - Potential ancestor class
    ///
    /// # Returns
    ///
    /// - `true` if `class` is in this class's inheritance chain
    /// - `false` otherwise
    ///
    /// # Note
    ///
    /// A class is considered to be a subclass of itself.
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::Class;
    ///
    /// let root = Class::new_root("Root").unwrap();
    /// let child = Class::new("Child", &root).unwrap();
    /// let grandchild = Class::new("GrandChild", &child).unwrap();
    ///
    /// assert!(grandchild.is_subclass_of(&child));
    /// assert!(grandchild.is_subclass_of(&root));
    /// assert!(!root.is_subclass_of(&child));
    /// ```
    #[must_use]
    pub fn is_subclass_of(&self, class: &Class) -> bool {
        // Check if this is the same class
        if std::ptr::eq(self.inner.as_ptr(), class.inner.as_ptr()) {
            return true;
        }

        // Walk the superclass chain
        let mut current = self.super_class();

        while let Some(superclass) = current {
            if std::ptr::eq(superclass.inner.as_ptr(), class.inner.as_ptr()) {
                return true;
            }

            current = superclass.super_class();
        }

        false
    }
}

// SAFETY: Class is Send because:
// - ClassInner is in arena (never moves, 'static lifetime)
// - Methods table protected by RwLock
// - All pointers are valid for entire program duration
unsafe impl Send for Class {}

// SAFETY: Class is Sync because:
// - Methods table access is protected by RwLock
// - Class metadata is immutable after construction
// - Arena provides stable pointers
unsafe impl Sync for Class {}

impl Clone for Class {
    fn clone(&self) -> Self {
        Class { inner: self.inner }
    }
}

impl PartialEq for Class {
    fn eq(&self, other: &Self) -> bool {
        // Pointer equality: same name = same class (registry guarantee)
        std::ptr::eq(self.inner.as_ptr(), other.inner.as_ptr())
    }
}

impl Eq for Class {}

impl fmt::Debug for Class {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let super_name = self.super_class().map(|c| c.name().to_string());
        f.debug_struct("Class")
            .field("name", &self.name())
            .field("super_class", &super_name)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_root_class_creation() {
        let class = Class::new_root("TestClass").unwrap();
        assert_eq!(class.name(), "TestClass");
        assert!(class.super_class().is_none());
    }

    #[test]
    fn test_subclass_creation() {
        let root = Class::new_root("SubTestRoot").unwrap();
        let sub = Class::new("SubTestSub", &root).unwrap();

        assert_eq!(sub.name(), "SubTestSub");
        assert!(sub.super_class().is_some());
        assert_eq!(sub.super_class().unwrap().name(), "SubTestRoot");
    }

    #[test]
    fn test_duplicate_class_name_error() {
        Class::new_root("DuplicateTest").unwrap();
        let result = Class::new_root("DuplicateTest");

        assert!(matches!(result, Err(Error::ClassAlreadyExists)));
    }

    #[test]
    fn test_inheritance_cycle_detection() {
        let a = Class::new_root("CycleTestA").unwrap();
        let b = Class::new("CycleTestB", &a).unwrap();

        // Try to create A -> B -> A cycle
        let result = Class::new("CycleTestA", &b);

        // Should detect cycle and reject
        assert!(matches!(result, Err(Error::InheritanceCycle)));
    }

    #[test]
    fn test_deep_inheritance_chain() {
        let a = Class::new_root("DeepChainA").unwrap();
        let b = Class::new("DeepChainB", &a).unwrap();
        let c = Class::new("DeepChainC", &b).unwrap();
        let d = Class::new("DeepChainD", &c).unwrap();

        assert_eq!(d.name(), "DeepChainD");
        assert_eq!(d.super_class().unwrap().name(), "DeepChainC");
        assert_eq!(d.super_class().unwrap().super_class().unwrap().name(), "DeepChainB");
        assert_eq!(
            d.super_class().unwrap().super_class().unwrap().super_class().unwrap().name(),
            "DeepChainA"
        );
    }

    #[test]
    fn test_method_registration() {
        let class = Class::new_root("MethodsTest").unwrap();
        let sel = Selector::from_str("doSomething:").unwrap();

        let method = Method {
            selector: sel.clone(),
            _imp: (),
            _types: (),
        };

        class.add_method(method).unwrap();

        let found = class.lookup_method(&sel);
        assert!(found.is_some());
        assert_eq!(found.unwrap().selector.name(), "doSomething:");
    }

    #[test]
    fn test_inheritance_lookup() {
        let parent = Class::new_root("InheritLookupParent").unwrap();
        let sel = Selector::from_str("inheritedMethod").unwrap();

        let method = Method {
            selector: sel.clone(),
            _imp: (),
            _types: (),
        };

        parent.add_method(method).unwrap();

        let child = Class::new("InheritLookupChild", &parent).unwrap();

        // Method found in parent
        let found = child.lookup_method(&sel);
        assert!(found.is_some());
        assert_eq!(found.unwrap().selector.name(), "inheritedMethod");
    }

    #[test]
    fn test_method_not_found() {
        let class = Class::new_root("NotFoundTest").unwrap();
        let sel = Selector::from_str("nonExistentMethod").unwrap();

        let found = class.lookup_method(&sel);
        assert!(found.is_none());
    }

    #[test]
    fn test_is_subclass_of() {
        let root = Class::new_root("IsSubclassRoot").unwrap();
        let child = Class::new("IsSubclassChild", &root).unwrap();
        let grandchild = Class::new("IsSubclassGrandChild", &child).unwrap();

        // Direct subclass
        assert!(child.is_subclass_of(&root));

        // Indirect subclass
        assert!(grandchild.is_subclass_of(&child));
        assert!(grandchild.is_subclass_of(&root));

        // Not a subclass
        assert!(!root.is_subclass_of(&child));
        assert!(!root.is_subclass_of(&grandchild));

        // A class is a subclass of itself
        assert!(root.is_subclass_of(&root));
        assert!(child.is_subclass_of(&child));
    }

    #[test]
    fn test_class_equality() {
        let class1 = Class::new_root("EqualityTest1").unwrap();
        let class2 = Class::new_root("EqualityTest2").unwrap();

        // Different classes are not equal
        assert_ne!(class1, class2);

        // Same class is equal to itself
        assert_eq!(class1, class1);
        assert_eq!(class2, class2);

        // Test that duplicate names are rejected
        let result = Class::new_root("EqualityTest1");
        assert!(matches!(result, Err(Error::ClassAlreadyExists)));
    }

    #[test]
    fn test_class_clone() {
        let class1 = Class::new_root("CloneTest").unwrap();
        let class2 = class1.clone();

        // Clone shares the same pointer
        assert!(std::ptr::eq(class1.inner.as_ptr(), class2.inner.as_ptr()));
        assert_eq!(class1, class2);
    }

    #[test]
    fn test_class_debug() {
        let root = Class::new_root("DebugRoot").unwrap();
        let child = Class::new("DebugChild", &root).unwrap();

        let debug_str = format!("{:?}", child);

        assert!(debug_str.contains("DebugChild"));
        assert!(debug_str.contains("DebugRoot"));
    }

    #[test]
    fn test_method_override() {
        let parent = Class::new_root("OverrideParent").unwrap();
        let child = Class::new("OverrideChild", &parent).unwrap();
        let sel = Selector::from_str("overrideMethod").unwrap();

        // Add method to parent
        let parent_method = Method {
            selector: sel.clone(),
            _imp: (),
            _types: (),
        };
        parent.add_method(parent_method).unwrap();

        // Override in child
        let child_method = Method {
            selector: sel.clone(),
            _imp: (),
            _types: (),
        };
        child.add_method(child_method).unwrap();

        // Child should find its own method first
        let found = child.lookup_method(&sel);
        assert!(found.is_some());

        // Parent still has its method
        let found_parent = parent.lookup_method(&sel);
        assert!(found_parent.is_some());
    }
}
