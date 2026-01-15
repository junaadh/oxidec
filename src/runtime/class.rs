//! `Class` creation and inheritance for the ``OxideC`` runtime.
//!
//! This module implements the class system with:
//! - `Class` registration and metadata
//! - Single inheritance with cycle detection
//! - `Method` registration and lookup
//! - Inheritance chain walking
//!
//! # Architecture
//!
//! `Class`es are **globally registered** and never deallocated:
//! - Each class name maps to exactly one ``Class`` instance
//! - `Class`es have `'static` lifetime (live for program duration)
//! - Immutable after creation (except method addition)
//! - Single inheritance chain to root class (Ox`Object`)
//!
//! # Thread Safety
//!
//! The class registry is thread-safe and supports concurrent class creation
//! from multiple threads. Uses `RwLock` for registry access and method table
//! protection.

use crate::error::{Error, Result};
use crate::runtime::selector::SelectorHandle;
use crate::runtime::{Protocol, RuntimeString, Selector, get_global_arena};
use std::collections::HashMap;
use std::fmt;
use std::ptr::NonNull;
use std::sync::OnceLock;
use std::sync::RwLock;

/// `Method` implementation function pointer type.
///
/// This is the C ABI-compatible function pointer type used for all method
/// implementations in the `OxideC` runtime. It follows the `Object`ive-C calling
/// convention where:
///
/// - First argument: receiver object (self)
/// - Second argument: method selector (_cmd)
/// - Third argument: pointer to array of argument pointers
/// - Fourth argument: pointer to return value storage
///
/// # Safety
///
/// Function pointers of this type MUST:
/// - Be `extern "C"` for proper calling convention
/// - Properly validate all pointer arguments before dereference
/// - Only write to `return_value_ptr` if the return type is non-void
/// - Handle argument marshalling based on method's type encoding
///
/// # Note
///
/// `Imp` is the low-level function pointer type for method implementations.
/// You typically don't work with this directly unless you're implementing
/// the dispatch system or custom method registration.
///
/// Uses `ObjectPtr` which is an opaque wrapper around the raw object data.
pub type Imp = unsafe extern "C" fn(
    _self: crate::runtime::object::ObjectPtr,
    _cmd: SelectorHandle,
    _args: *const *mut u8,
    _ret: *mut u8,
);

/// Internal class data stored in global arena.
///
/// This struct is allocated in the global arena and never deallocated.
#[repr(C)]
pub(crate) struct ClassInner {
    /// Class name (e.g., "`OxObject`", "`MutableArray`")
    name: RuntimeString,
    /// Superclass pointer (null for root class Ox`Object`)
    super_class: Option<NonNull<ClassInner>>,
    /// `Method` table: selector hash -> Method
    /// Protected by `RwLock` for thread-safe method addition
    methods: RwLock<HashMap<u64, Method>>,
    /// Method cache for fast dispatch: selector hash -> (`class_ptr`, imp)
    /// Protected by `RwLock` for thread-safe cache access
    /// Stores `class_ptr` to handle method swizzling (cache invalidation)
    cache: RwLock<HashMap<u64, (NonNull<ClassInner>, Imp)>>,
    /// `Class` flags (reserved for future use)
    flags: u32,
    /// Categories attached to this class
    /// Protected by `RwLock` for thread-safe category addition
    pub(crate) categories:
        RwLock<Vec<NonNull<crate::runtime::category::CategoryInner>>>,
    /// Protocols this class conforms to
    /// Protected by `RwLock` for thread-safe protocol addition
    pub(crate) protocols:
        RwLock<Vec<NonNull<crate::runtime::protocol::ProtocolInner>>>,
    /// Per-class forwarding hook (called when selector not found in instances)
    /// Protected by `RwLock` for thread-safe hook access
    pub(crate) forwarding_hook:
        RwLock<Option<crate::runtime::forwarding::ClassForwardingHook>>,
}

/// Global class registry.
///
/// Ensures unique class names and provides fast lookup by name.
struct ClassRegistry {
    /// Map of class name -> Class pointer
    /// Protected by `RwLock` for thread-safe class registration
    classes: RwLock<HashMap<RuntimeString, NonNull<ClassInner>>>,
}

// SAFETY: ClassRegistry is Send + Sync because:
// - `Class`Inner pointers point to arena memory (never deallocated)
// - RwLock provides synchronized access
// - `Arena` ensures proper alignment and validity
unsafe impl Send for ClassRegistry {}
unsafe impl Sync for ClassRegistry {}

/// Global class registry instance.
static REGISTRY: OnceLock<ClassRegistry> = OnceLock::new();

/// `Method` representation with implementation and type encoding.
///
/// # Memory Layout
///
/// `Method`s are stored in class method tables and have `'static` lifetime
/// (allocated in global arena).
///
/// # Thread Safety
///
/// `Method`s are immutable after creation and safe to share between threads.
#[derive(Clone)]
pub struct Method {
    /// `Method` selector
    pub selector: Selector,
    /// Function pointer to method implementation
    pub imp: Imp,
    /// Type encoding string (e.g., "v@:" for void return, id self, SEL _cmd)
    /// Stored as interned string in arena
    pub types: RuntimeString,
}

impl fmt::Debug for Method {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("`Method`")
            .field("selector", &self.selector)
            .field("imp", &format!("{:p}", self.imp as *const ()))
            .field("types", &self.types.as_str().unwrap_or("<invalid>"))
            .finish()
    }
}

/// `Class` represents a runtime class definition with methods and inheritance.
///
/// `Class`es are **globally registered** and never deallocated. They provide:
/// - `Class` metadata (name, superclass, methods)
/// - Inheritance chain walking
/// - `Method` lookup by selector
///
/// # Memory Management
///
/// `Class`es use manual memory management:
/// - Allocated in global arena (stable pointers)
/// - Never deallocated (live for program duration)
/// - Immutable after creation (except method addition)
///
/// # Thread Safety
///
/// `Class`es are `Send + Sync`:
/// - Methods table protected by `RwLock`
/// - `Class` metadata is immutable after construction
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
    /// * `name` - `Class` name (must be unique in runtime)
    ///
    /// # Returns
    ///
    /// Returns `Ok(`Class`)` if name is unique, `Err` if class already exists.
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
    ///
    /// # Errors
    ///
    /// Returns [`Error::ClassExists`] if a root class with this name already exists
    /// in the runtime.
    pub fn new_root(name: &str) -> Result<Self> {
        Self::create_class(name, None)
    }

    /// Creates a new class with a superclass.
    ///
    /// # Arguments
    ///
    /// * `name` - `Class` name (must be unique)
    /// * `super_class` - Superclass to inherit from
    ///
    /// # Returns
    ///
    /// Returns `Ok(`Class`)` if successful, `Err` on:
    /// - `Class` name already exists
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
    ///
    /// # Errors
    ///
    /// Returns [`Error::ClassExists`] if a class with this name already exists,
    /// or [`Error::InheritanceCycle`] if adding this class would create a cycle
    /// in the inheritance hierarchy.
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

        // Create `Class`Inner
        let super_ptr = super_class.map(|sc| sc.inner);
        let class_inner = ClassInner {
            name: name_str,
            super_class: super_ptr,
            methods: RwLock::new(HashMap::new()),
            cache: RwLock::new(HashMap::new()),
            flags: 0,
            categories: RwLock::new(Vec::new()),
            protocols: RwLock::new(Vec::new()),
            forwarding_hook: RwLock::new(None),
        };

        // Allocate in global arena
        let inner_ptr: *mut ClassInner = arena.alloc(class_inner);

        // SAFETY: inner_ptr is not null and properly aligned (arena ensures)
        let inner_nn =
            NonNull::new(inner_ptr).expect("`Arena` allocation returned null");

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
    fn check_inheritance_cycle(
        new_class_name: &str,
        super_class: &Class,
    ) -> Result<()> {
        let mut current_ptr = Some(super_class.inner.as_ptr());

        while let Some(ptr) = current_ptr {
            // SAFETY: ptr points to valid `Class`Inner in arena
            let inner = unsafe { &*ptr };

            // Check if we found the new class name in the superclass chain
            // unwrap() is safe because `RuntimeString` in arena is always valid
            if inner.name.as_str().unwrap() == new_class_name {
                return Err(Error::InheritanceCycle);
            }

            // Move to superclass
            current_ptr = inner.super_class.map(NonNull::as_ptr);
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
    ///
    /// # Panics
    ///
    /// Panics if the class's name string in the arena is invalid UTF-8 (which
    /// should never happen under normal circumstances).
    #[must_use]
    pub fn name(&self) -> &str {
        // SAFETY: self.inner points to valid `Class`Inner in arena
        // unwrap() is safe because `RuntimeString` in arena is always valid
        unsafe { &(*self.inner.as_ptr()).name }.as_str().unwrap()
    }

    /// Returns the superclass (if any).
    ///
    /// # Returns
    ///
    /// - `Some(`Class`)` if this class has a superclass
    /// - `None` if this is a root class (Ox`Object`)
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
        // SAFETY: self.inner points to valid `Class`Inner in arena
        let inner = unsafe { &*self.inner.as_ptr() };

        // Return a new `Class` wrapping the superclass pointer
        inner.super_class.map(|ptr| Class { inner: ptr })
    }

    /// Adds a method to the class.
    ///
    /// # Arguments
    ///
    /// * `method` - `Method` to add
    ///
    /// # Thread Safety
    ///
    /// Multiple threads can add methods concurrently. `RwLock` ensures
    /// synchronized access to the methods table.
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::{Class, Selector};
    /// use oxidec::runtime::get_global_arena;
    /// use std::str::FromStr;
    ///
    /// let class = Class::new_root("MyClass").unwrap();
    /// let sel = Selector::from_str("doSomething:").unwrap();
    /// let arena = get_global_arena();
    ///
    /// // Note: Method creation requires an implementation function pointer.
    /// // This is typically done through the method registration API.
    /// // See the dispatch module for full message sending implementation.
    /// ```
    ///
    /// # Errors
    ///
    /// This function currently always returns `Ok(())`. The `Result` type is
    /// used for future extensibility.
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned (indicates a concurrent
    /// access error or panic in another thread).
    pub fn add_method(&self, method: Method) -> Result<()> {
        // SAFETY: self.inner points to valid `Class`Inner
        let inner = unsafe { &*self.inner.as_ptr() };

        let mut methods = inner.methods.write().unwrap();
        let hash = method.selector.hash();

        methods.insert(hash, method);

        Ok(())
    }

    /// Invalidates the method cache for this class.
    ///
    /// This is called internally when categories are added or methods are
    /// swizzled to ensure the dispatch system uses the updated method list.
    ///
    /// # Thread Safety
    ///
    /// This method acquires the cache write lock and clears all cached entries.
    /// Multiple threads can call this concurrently.
    pub(crate) fn invalidate_cache(&self) {
        // SAFETY: self.inner points to valid `Class`Inner
        let inner = unsafe { &*self.inner.as_ptr() };
        let mut cache = inner.cache.write().unwrap();
        cache.clear();
    }

    /// Looks up a method by selector (searches inheritance chain).
    ///
    /// # Arguments
    ///
    /// * `selector` - `Method` selector to lookup
    ///
    /// # Returns
    ///
    /// - `Some(&`Method`)` if found in this class or ancestor
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
    /// use oxidec::{Class, Selector};
    /// use std::str::FromStr;
    ///
    /// let parent = Class::new_root("Parent").unwrap();
    /// let sel = Selector::from_str("inheritedMethod").unwrap();
    ///
    /// // Add method to parent (implementation omitted)
    /// // parent.add_method(method).unwrap();
    ///
    /// let child = Class::new("Child", &parent).unwrap();
    ///
    /// // Method would be found in parent (if added)
    /// // assert!(child.lookup_method(&sel).is_some());
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned (indicates a concurrent
    /// access error or panic in another thread).
    #[must_use]
    pub fn lookup_method(&self, selector: &Selector) -> Option<&Method> {
        let mut current_ptr = Some(self.inner.as_ptr());

        while let Some(ptr) = current_ptr {
            // SAFETY: ptr points to valid `Class`Inner
            let inner = unsafe { &*ptr };

            // Try to find method in this class
            let methods = inner.methods.read().unwrap();
            let hash = selector.hash();

            if let Some(method) = methods.get(&hash) {
                // Found! Return reference with extended lifetime
                // SAFETY: The method is in the arena and never deallocated
                // We're creating a reference with the same lifetime as &self
                return unsafe { Some(&*std::ptr::from_ref::<Method>(method)) };
            }
            drop(methods);

            // Check category methods (Phase 3.1)
            let categories = inner.categories.read().unwrap();
            for cat_ptr in categories.iter() {
                // SAFETY: cat_ptr points to valid CategoryInner
                let cat = unsafe { &*cat_ptr.as_ptr() };
                let cat_methods = cat.methods.read().unwrap();
                if let Some(method) = cat_methods.get(&hash) {
                    // Found in category!
                    // SAFETY: The method is in the arena and never deallocated
                    return unsafe {
                        Some(&*std::ptr::from_ref::<Method>(method))
                    };
                }
            }
            drop(categories);

            // Not found, try superclass
            current_ptr = inner.super_class.map(NonNull::as_ptr);
        }

        None
    }

    /// Looks up a method implementation pointer with caching.
    ///
    /// This is the fast-path for message dispatch. It checks the cache first,
    /// then falls back to walking the inheritance chain if needed.
    ///
    /// # Arguments
    ///
    /// * `selector` - `Method` selector to lookup
    ///
    /// # Returns
    ///
    /// - `Some(Imp)` if method is found
    /// - `None` if not found in entire inheritance chain
    ///
    /// # Performance
    ///
    /// - Cache hit: ~20-30ns (`HashMap` lookup)
    /// - Cache miss: ~100-150ns (inheritance walk + cache update)
    ///
    /// # Note
    ///
    /// This method is used internally by the message dispatch system.
    /// The returned `Imp` function pointer can be called with the appropriate
    /// arguments after marshalling them based on the method's type encoding.
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned (indicates a concurrent
    /// access error or panic in another thread).
    #[must_use]
    pub fn lookup_imp(&self, selector: &Selector) -> Option<Imp> {
        let hash = selector.hash();

        // Fast path: Check cache
        {
            // SAFETY: self.inner points to valid `Class`Inner
            let inner = unsafe { &*self.inner.as_ptr() };
            let cache = inner.cache.read().unwrap();

            if let Some((cached_class, imp)) = cache.get(&hash) {
                // Verify cache is still valid (handles method swizzling)
                if std::ptr::eq(cached_class.as_ptr(), self.inner.as_ptr()) {
                    return Some(*imp);
                }
                // Cache entry is stale, fall through to slow path
            }
        } // Release read lock

        // Slow path: Walk inheritance chain
        if let Some(method) = self.lookup_method(selector) {
            let imp = method.imp;

            // Update cache
            {
                // SAFETY: self.inner points to valid `Class`Inner
                let inner = unsafe { &*self.inner.as_ptr() };
                let mut cache = inner.cache.write().unwrap();
                cache.insert(hash, (self.inner, imp));
            }

            Some(imp)
        } else {
            None
        }
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

    /// Adds protocol conformance to this class.
    ///
    /// # Arguments
    ///
    /// * `protocol` - Protocol to adopt
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if successful, `Err` if protocol already adopted.
    ///
    /// # Thread Safety
    ///
    /// Multiple threads can add protocols concurrently.
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::{Class, Protocol};
    ///
    /// let class = Class::new_root("MyClass").unwrap();
    /// let protocol = Protocol::new("MyProtocol", None).unwrap();
    ///
    /// class.add_protocol(&protocol).unwrap();
    /// assert!(class.conforms_to(&protocol));
    /// ```
    ///
    /// # Errors
    ///
    /// Returns `Err(Error::ProtocolAlreadyAdopted)` if this class already
    /// conforms to the protocol.
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned (indicates a concurrent
    /// access error or panic in another thread).
    pub fn add_protocol(&self, protocol: &Protocol) -> Result<()> {
        // SAFETY: self.inner points to valid ClassInner
        let inner = unsafe { &*self.inner.as_ptr() };

        // Check for duplicate protocol adoption
        let protocols = inner.protocols.read().unwrap();
        for proto_ptr in protocols.iter() {
            if proto_ptr.as_ptr() == protocol.inner.as_ptr() {
                return Err(Error::ProtocolAlreadyAdopted);
            }
        }
        drop(protocols);

        // Add protocol to class
        {
            let mut protocols = inner.protocols.write().unwrap();
            protocols.push(protocol.inner);
        }

        // Invalidate method cache
        self.invalidate_cache();

        Ok(())
    }

    /// Checks if this class conforms to a protocol.
    ///
    /// # Arguments
    ///
    /// * `protocol` - Protocol to check
    ///
    /// # Returns
    ///
    /// Returns `true` if this class conforms to the protocol, `false` otherwise.
    ///
    /// # Thread Safety
    ///
    /// Multiple threads can check conformance concurrently.
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::{Class, Protocol};
    ///
    /// let class = Class::new_root("MyClass").unwrap();
    /// let protocol = Protocol::new("MyProtocol", None).unwrap();
    ///
    /// assert!(!class.conforms_to(&protocol));
    /// class.add_protocol(&protocol).unwrap();
    /// assert!(class.conforms_to(&protocol));
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned (indicates a concurrent
    /// access error or panic in another thread).
    #[must_use]
    pub fn conforms_to(&self, protocol: &Protocol) -> bool {
        // SAFETY: self.inner points to valid ClassInner
        let inner = unsafe { &*self.inner.as_ptr() };

        // Check this class's protocols
        let protocols = inner.protocols.read().unwrap();
        for proto_ptr in protocols.iter() {
            if proto_ptr.as_ptr() == protocol.inner.as_ptr() {
                return true;
            }
        }
        drop(protocols);

        // Check superclasses (conformance is transitive through inheritance)
        if let Some(superclass) = self.super_class() {
            superclass.conforms_to(protocol)
        } else {
            false
        }
    }

    /// Returns all protocols this class conforms to.
    ///
    /// Includes protocols from superclasses (conformance is transitive).
    ///
    /// # Returns
    ///
    /// Vector of protocols this class conforms to.
    ///
    /// # Thread Safety
    ///
    /// Multiple threads can query protocols concurrently.
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::{Class, Protocol};
    ///
    /// let class = Class::new_root("MyClass").unwrap();
    /// let protocol1 = Protocol::new("Protocol1", None).unwrap();
    /// let protocol2 = Protocol::new("Protocol2", None).unwrap();
    ///
    /// class.add_protocol(&protocol1).unwrap();
    /// class.add_protocol(&protocol2).unwrap();
    ///
    /// let protocols = class.protocols();
    /// assert_eq!(protocols.len(), 2);
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned (indicates a concurrent
    /// access error or panic in another thread).
    #[must_use]
    pub fn protocols(&self) -> Vec<Protocol> {
        let mut result = Vec::new();

        // Add protocols from this class
        // SAFETY: self.inner points to valid ClassInner
        let inner = unsafe { &*self.inner.as_ptr() };
        let protocols = inner.protocols.read().unwrap();
        for &proto_ptr in protocols.iter() {
            result.push(Protocol { inner: proto_ptr });
        }
        drop(protocols);

        // Add protocols from superclasses
        if let Some(superclass) = self.super_class() {
            result.extend(superclass.protocols());
        }

        // Deduplicate while preserving order
        let mut seen = std::collections::HashSet::new();
        result.retain(|p| seen.insert(p.inner.as_ptr() as usize));

        result
    }

    /// Validates that this class implements all required protocol methods.
    ///
    /// This is the **optional runtime validation** part of the hybrid validation
    /// approach. It checks that the class implements all methods required by
    /// the protocol.
    ///
    /// # Arguments
    ///
    /// * `protocol` - Protocol to validate against
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the class conforms to the protocol, `Err` if any
    /// required methods are missing.
    ///
    /// # Thread Safety
    ///
    /// Multiple threads can validate concurrently.
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::{Class, Method, Protocol, RuntimeString, Selector};
    /// use oxidec::runtime::get_global_arena;
    /// use std::str::FromStr;
    ///
    /// let protocol = Protocol::new("MyProtocol", None).unwrap();
    /// let sel = Selector::from_str("requiredMethod").unwrap();
    /// protocol.add_required(sel.clone(), "v@:", get_global_arena()).unwrap();
    ///
    /// let class = Class::new_root("MyClass").unwrap();
    /// class.add_protocol(&protocol).unwrap();
    ///
    /// // Validation will fail (method not implemented)
    /// assert!(class.validate_protocol_conformance(&protocol).is_err());
    ///
    /// // Add the required method
    /// let method = Method {
    ///     selector: sel.clone(),
    ///     imp: required_method_impl,
    ///     types: RuntimeString::new("v@:", get_global_arena()),
    /// };
    /// class.add_method(method).unwrap();
    ///
    /// // Now validation passes
    /// class.validate_protocol_conformance(&protocol).unwrap();
    /// #
    /// # unsafe extern "C" fn required_method_impl(
    /// #     _self: oxidec::runtime::object::ObjectPtr,
    /// #     _cmd: oxidec::runtime::selector::SelectorHandle,
    /// #     _args: *const *mut u8,
    /// #     _ret: *mut u8,
    /// # ) {}
    /// ```
    ///
    /// # Errors
    ///
    /// Returns `Err(Error::MissingProtocolMethod)` if the class doesn't implement
    /// a required protocol method.
    pub fn validate_protocol_conformance(
        &self,
        protocol: &Protocol,
    ) -> Result<()> {
        // Get all required methods from protocol (including base protocols)
        let required_methods = protocol.all_required();

        // Check each required method
        for (_hash, selector) in required_methods {
            // Check if class has this method (walks: local → categories → superclass)
            if self.lookup_method(&selector).is_none() {
                return Err(Error::MissingProtocolMethod {
                    selector: selector.name().to_string(),
                });
            }
        }

        Ok(())
    }

    /// Replaces the implementation of a method with a new function pointer.
    ///
    /// This is the **method swizzling** operation, which allows runtime replacement
    /// of method implementations. Common use cases:
    ///
    /// - Runtime patching (hotfixing)
    /// - Debugging/profiling injection
    /// - AOP (aspect-oriented programming)
    /// - Testing mocks/stubs
    ///
    /// # Arguments
    ///
    /// * `selector` - The method selector to swizzle
    /// * `new_imp` - The new implementation function pointer
    ///
    /// # Returns
    ///
    /// Returns `Ok(original_imp)` - the original implementation (can be restored later).
    ///
    /// # Thread Safety
    ///
    /// This method is thread-safe:
    /// - Acquires method table write lock
    /// - Invalidates method cache before returning
    /// - Atomic swap prevents race conditions
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::{Class, Method, Selector, RuntimeString};
    /// use oxidec::runtime::get_global_arena;
    /// use std::str::FromStr;
    ///
    /// let class = Class::new_root("MyClass").unwrap();
    /// let sel = Selector::from_str("doSomething").unwrap();
    /// let arena = get_global_arena();
    ///
    /// // Add original method
    /// let original_imp = my_original_impl;
    /// let method = Method {
    ///     selector: sel.clone(),
    ///     imp: original_imp,
    ///     types: RuntimeString::new("v@:", arena),
    /// };
    /// class.add_method(method).unwrap();
    ///
    /// // Swizzle with replacement
    /// let replacement_imp = my_replacement_impl;
    /// let saved_original = class.swizzle_method(&sel, replacement_imp).unwrap();
    ///
    /// // Now calls to doSomething invoke replacement_imp
    /// // Can restore: class.swizzle_method(&sel, saved_original)?;
    /// #
    /// # unsafe extern "C" fn my_original_impl(
    /// #     _self: oxidec::runtime::object::ObjectPtr,
    /// #     _cmd: oxidec::runtime::selector::SelectorHandle,
    /// #     _args: *const *mut u8,
    /// #     _ret: *mut u8,
    /// # ) {}
    /// # unsafe extern "C" fn my_replacement_impl(
    /// #     _self: oxidec::runtime::object::ObjectPtr,
    /// #     _cmd: oxidec::runtime::selector::SelectorHandle,
    /// #     _args: *const *mut u8,
    /// #     _ret: *mut u8,
    /// # ) {}
    /// ```
    ///
    /// # Errors
    ///
    /// Returns `Err(Error::SelectorNotFound)` if the selector doesn't exist
    /// in this class's method table (excluding inherited methods).
    ///
    /// # Safety
    ///
    /// The caller MUST ensure that `new_imp` has the exact same function signature
    /// as the original implementation. Using a mismatched signature is **undefined behavior**.
    /// This is the same as Objective-C's `method_setImplementation`.
    ///
    /// **Note:** Swizzling only affects the target class's method table. Inherited methods
    /// from parent classes cannot be swizzled directly (must swizzle on the parent class).
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned (indicates a concurrent
    /// access error or panic in another thread).
    pub fn swizzle_method(
        &self,
        selector: &Selector,
        new_imp: Imp,
    ) -> Result<Imp> {
        let hash = selector.hash();

        // SAFETY: self.inner points to valid ClassInner allocated in arena
        // Lifetime: ClassInner is arena-allocated and lives for entire program duration
        let inner = unsafe { &*self.inner.as_ptr() };

        // Acquire write lock for thread-safe modification
        let mut methods = inner.methods.write().unwrap();

        // Find method in this class's method table (does not search superclass)
        // Rationale: Swizzling should only affect this class, not parent
        let method = methods.get_mut(&hash).ok_or(Error::SelectorNotFound)?;

        // Swap implementation
        let original_imp = method.imp;
        method.imp = new_imp;

        // Drop write lock before invalidating cache (avoid deadlock)
        drop(methods);

        // Invalidate method cache for this class
        // This forces the next message send to re-walk the method lookup chain
        // and find the new implementation
        self.invalidate_cache();

        Ok(original_imp)
    }

    /// Returns the hash of this class's inner pointer.
    ///
    /// This is used by the forwarding cache to create a unique key for
    /// caching forwarded targets.
    pub(crate) fn inner_hash(&self) -> u64 {
        self.inner.as_ptr() as usize as u64
    }

    /// Sets a forwarding hook for all instances of this class.
    ///
    /// The hook is called when a selector is not found in any instance's class.
    /// If it returns Some(target), the message is retried on the target.
    ///
    /// # Priority
    ///
    /// Per-object hooks > Per-class hooks > Global hooks
    ///
    /// # Thread Safety
    ///
    /// This method is thread-safe. The last hook set wins.
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::{Class, Selector};
    ///
    /// let class = Class::new_root("MyClass").unwrap();
    ///
    /// class.set_forwarding_hook(|obj, sel| {
    ///     // Forward unknown messages to a proxy object
    ///     None // Return Some(target) to forward
    /// });
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned (indicates a concurrent
    /// access error or panic in another thread).
    pub fn set_forwarding_hook(
        &self,
        hook: crate::runtime::forwarding::ClassForwardingHook,
    ) {
        // SAFETY: self.inner points to valid ClassInner allocated in arena
        let inner = unsafe { &*self.inner.as_ptr() };
        *inner.forwarding_hook.write().unwrap() = Some(hook);
    }

    /// Clears this class's forwarding hook.
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned (indicates a concurrent
    /// access error or panic in another thread).
    pub fn clear_forwarding_hook(&self) {
        // SAFETY: self.inner points to valid ClassInner allocated in arena
        let inner = unsafe { &*self.inner.as_ptr() };
        *inner.forwarding_hook.write().unwrap() = None;
    }

    /// Gets this class's forwarding hook (if set).
    ///
    /// This is used internally by the forwarding resolution system.
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned (indicates a concurrent
    /// access error or panic in another thread).
    pub(crate) fn get_forwarding_hook(
        &self,
    ) -> Option<crate::runtime::forwarding::ClassForwardingHook> {
        // SAFETY: self.inner points to valid ClassInner allocated in arena
        let inner = unsafe { &*self.inner.as_ptr() };
        *inner.forwarding_hook.read().unwrap()
    }
}

// SAFETY: Class is Send because:
// - `Class`Inner is in arena (never moves, 'static lifetime)
// - `Method`s table protected by RwLock
// - All pointers are valid for entire program duration
unsafe impl Send for Class {}

// SAFETY: Class is Sync because:
// - `Method`s table access is protected by RwLock
// - `Class` metadata is immutable after construction
// - `Arena` provides stable pointers
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
        f.debug_struct("`Class`")
            .field("name", &self.name())
            .field("super_class", &super_name)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    /// Test helper function that does nothing (no-op method implementation)
    unsafe extern "C" fn test_method_noop(
        _self: crate::runtime::object::ObjectPtr,
        _cmd: SelectorHandle,
        _args: *const *mut u8,
        _ret: *mut u8,
    ) {
        // No-op for testing
    }

    #[test]
    fn test_root_class_creation() {
        let class = Class::new_root("Test`Class`").unwrap();
        assert_eq!(class.name(), "Test`Class`");
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
        assert_eq!(
            d.super_class().unwrap().super_class().unwrap().name(),
            "DeepChainB"
        );
        assert_eq!(
            d.super_class()
                .unwrap()
                .super_class()
                .unwrap()
                .super_class()
                .unwrap()
                .name(),
            "DeepChainA"
        );
    }

    #[test]
    fn test_method_registration() {
        let class = Class::new_root("`Method`sTest").unwrap();
        let sel = Selector::from_str("doSomething:").unwrap();

        let arena = get_global_arena();
        let method = Method {
            selector: sel.clone(),
            imp: test_method_noop,
            types: RuntimeString::new("v@:", arena),
        };

        class.add_method(method).unwrap();

        let found = class.lookup_method(&sel);
        assert!(found.is_some());
        assert_eq!(found.unwrap().selector.name(), "doSomething:");
    }

    #[test]
    fn test_inheritance_lookup() {
        let parent = Class::new_root("InheritLookupParent").unwrap();
        let sel = Selector::from_str("inherited`Method`").unwrap();

        let arena = get_global_arena();
        let method = Method {
            selector: sel.clone(),
            imp: test_method_noop,
            types: RuntimeString::new("v@:", arena),
        };

        parent.add_method(method).unwrap();

        let child = Class::new("InheritLookupChild", &parent).unwrap();

        // `Method` found in parent
        let found = child.lookup_method(&sel);
        assert!(found.is_some());
        assert_eq!(found.unwrap().selector.name(), "inherited`Method`");
    }

    #[test]
    fn test_method_not_found() {
        let class = Class::new_root("NotFoundTest").unwrap();
        let sel = Selector::from_str("nonExistent`Method`").unwrap();

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

        let debug_str = format!("{child:?}");

        assert!(debug_str.contains("DebugChild"));
        assert!(debug_str.contains("DebugRoot"));
    }

    #[test]
    fn test_method_override() {
        let parent = Class::new_root("OverrideParent").unwrap();
        let child = Class::new("OverrideChild", &parent).unwrap();
        let sel = Selector::from_str("override`Method`").unwrap();

        let arena = get_global_arena();

        // Add method to parent
        let parent_method = Method {
            selector: sel.clone(),
            imp: test_method_noop,
            types: RuntimeString::new("v@:", arena),
        };
        parent.add_method(parent_method).unwrap();

        // Override in child
        let child_method = Method {
            selector: sel.clone(),
            imp: test_method_noop,
            types: RuntimeString::new("v@:", arena),
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
