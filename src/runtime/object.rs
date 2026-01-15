//! `Object` allocation and lifecycle management for the ``OxideC`` runtime.
//!
//! This module implements the object system with:
//! - Reference counting with atomic operations
//! - Automatic memory management (retain/release)
//! - Thread-safe object lifecycle
//! - `Class` isa pointers for dynamic dispatch
//!
//! # Architecture
//!
//! `Object`s are heap-allocated with manual memory management:
//! - Each object has an atomic reference count
//! - `Object`s are deallocated when refcount reaches 0
//! - Thread-safe via atomic operations (`AcqRel` ordering)
//! - Clone is shallow (pointer duplication with refcount increment)
//!
//! # Thread Safety
//!
//! `Object`s are `Send + Sync` when reference counting is atomic:
//! - Multiple threads can hold references to same object
//! - retain/release are thread-safe (atomic operations)
//! - `Object` data access requires external synchronization (Phase 2)

use crate::error::Result;
use crate::runtime::Class;
use crate::runtime::MessageArgs;
use crate::runtime::Selector;
use std::fmt;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicU32, Ordering};

// Raw pointer to ClassInner (defined in class.rs)
// We use raw pointer to avoid circular dependency
type ClassInnerPtr = *const ();

/// Opaque pointer to a raw object.
///
/// This type wraps a raw pointer to `RawObject` while keeping the internal
/// type private. It is used in public APIs (like method implementations)
/// to avoid exposing the `RawObject` struct directly.
///
/// # Type Safety
///
/// `ObjectPtr` is opaque - users of the API cannot directly access or
/// manipulate the underlying `RawObject`. This maintains encapsulation
/// while still allowing low-level code (like the dispatch system) to
/// work with raw pointers.
///
/// # Thread Safety
///
/// `ObjectPtr` is `Send + Sync` when the underlying `RawObject` is
/// reference-counted with atomic operations.
///
/// # Example
///
/// ```rust,no_run
/// use oxidec::runtime::ObjectPtr;
///
/// // In method implementations, ObjectPtr is used as the self parameter
/// unsafe extern "C" fn my_method(
///     _self: ObjectPtr,
///     _cmd: oxidec::runtime::Selector,
///     _args: *const *mut u8,
///     _ret: *mut u8,
/// ) {
///     // Method implementation
/// }
/// ```
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ObjectPtr(*mut RawObject);

unsafe impl Send for ObjectPtr {}
unsafe impl Sync for ObjectPtr {}

impl ObjectPtr {
    /// Creates an `ObjectPtr` from a raw pointer.
    ///
    /// # Safety
    ///
    /// Caller must ensure `ptr` points to a valid `RawObject`.
    #[must_use]
    pub(crate) unsafe fn from_raw(ptr: *mut RawObject) -> Self {
        ObjectPtr(ptr)
    }

    /// Returns the underlying raw pointer.
    #[must_use]
    #[allow(dead_code)]
    pub(crate) fn as_raw_ptr(self) -> *mut RawObject {
        self.0
    }
}

/// Raw object representation allocated on heap.
///
/// This struct is **not** allocated in the arena (unlike `Selector`/`Class`)
/// because objects have individual lifetimes controlled by reference counting.
#[repr(C)]
pub(crate) struct RawObject {
    /// Isa pointer: class that this object is an instance of
    /// Points to `ClassInner` in arena (never deallocated)
    /// Stored as opaque pointer to avoid circular dependency
    class_ptr: ClassInnerPtr,
    /// Object flags (reserved for future use: tagged pointers, etc.)
    flags: u32,
    /// Reference count (starts at 1, deallocated when reaches 0)
    /// Atomic for thread-safe retain/release
    refcount: AtomicU32,
    /// Payload data (flexible array member pattern)
    /// For Phase 1, this is empty. In Phase 2+, it will hold instance variables.
    payload: [u8; 0],
}

/// `Object` represents a runtime instance with dynamic dispatch.
///
/// `Object`s are reference-counted and support:
/// - Automatic memory management (retain/release)
/// - Dynamic dispatch via isa pointer
/// - Thread-safe reference counting
///
/// # Memory Layout
///
/// `Object`s use manual memory management for performance:
/// - `RawObject` allocated on heap (not arena, for per-instance lifecycle)
/// - Reference counted with atomic operations
/// - `Class` pointer (isa) for dynamic dispatch
///
/// # Thread Safety
///
/// `Object`s are `Send + Sync` when reference counting is atomic:
/// - Multiple threads can hold references to same object
/// - retain/release are thread-safe (atomic operations)
/// - Method dispatch requires external synchronization (Phase 2)
///
/// # Example
///
/// ```rust
/// use oxidec::{Class, Object};
///
/// let class = Class::new_root("MyClass").unwrap();
/// let obj1 = Object::new(&class).unwrap();
///
/// // Clone creates a new reference to the same object
/// let obj2 = obj1.clone();
///
/// // Both references point to the same object
/// assert_eq!(obj1.class().name(), obj2.class().name());
/// ```
pub struct Object {
    /// Pointer to object data on heap.
    /// Never null, valid while refcount > 0.
    ptr: NonNull<RawObject>,
}

impl Object {
    /// Creates a new object instance of the given class.
    ///
    /// # Arguments
    ///
    /// * `class` - The class to instantiate
    ///
    /// # Returns
    ///
    /// Returns `Ok(Object)` with refcount = 1, or `Err` on allocation failure.
    ///
    /// # Thread Safety
    ///
    /// Multiple threads can create objects concurrently. Each object is
    /// independently reference-counted.
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::{Class, Object};
    ///
    /// let class = Class::new_root("MyClass").unwrap();
    /// let obj = Object::new(&class).unwrap();
    ///
    /// assert_eq!(obj.class().name(), "MyClass");
    /// assert_eq!(obj.refcount(), 1);
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`Error::OutOfMemory`] if object allocation fails.
    pub fn new(class: &Class) -> Result<Self> {
        // Get class pointer for isa
        // Store as opaque pointer to avoid circular dependency
        let class_ptr = class.inner.as_ptr() as ClassInnerPtr;

        // Create RawObject with initial refcount = 1
        let raw_obj = RawObject {
            class_ptr,
            flags: 0,
            refcount: AtomicU32::new(1),
            payload: [],
        };

        // Allocate on heap (not arena) for individual lifecycle
        // Box allocation ensures proper alignment and validity
        let boxed = Box::new(raw_obj);

        // Convert to raw pointer (ownership transferred to Object)
        let ptr = Box::into_raw(boxed);

        // SAFETY: ptr is not null (Box::new always succeeds)
        Ok(Object {
            ptr: unsafe { NonNull::new_unchecked(ptr) },
        })
    }

    /// Increments the reference count (retain).
    ///
    /// # Thread Safety
    ///
    /// Multiple threads can call retain concurrently. Atomic operations
    /// prevent data races.
    ///
    /// # Panics
    ///
    /// Panics if refcount overflows (`u32::MAX`).
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::{Class, Object};
    ///
    /// let class = Class::new_root("MyClass").unwrap();
    /// let obj = Object::new(&class).unwrap();
    ///
    /// assert_eq!(obj.refcount(), 1);
    ///
    /// obj.retain();
    /// assert_eq!(obj.refcount(), 2);
    /// ```
    pub fn retain(&self) {
        // SAFETY: self.ptr points to valid RawObject
        let obj = unsafe { &*self.ptr.as_ptr() };

        // Atomic increment with AcqRel ordering
        let old = obj.refcount.fetch_add(1, Ordering::AcqRel);

        // Check for overflow
        assert!(
            old != u32::MAX,
            "Reference count overflow in Object::retain"
        );
    }

    /// Decrements the reference count (release).
    ///
    /// Deallocates the object if refcount reaches 0.
    ///
    /// # Thread Safety
    ///
    /// Multiple threads can call release concurrently.
    ///
    /// # Note
    ///
    /// After calling release, the object may be deallocated if refcount
    /// reaches 0. Accessing the object after deallocation is unsafe.
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::{Class, Object};
    ///
    /// let class = Class::new_root("MyClass").unwrap();
    /// let obj = Object::new(&class).unwrap();
    ///
    /// obj.retain();
    /// assert_eq!(obj.refcount(), 2);
    ///
    /// obj.release();
    /// assert_eq!(obj.refcount(), 1);
    /// ```
    pub fn release(&self) {
        // SAFETY: self.ptr points to valid RawObject
        let obj = unsafe { &*self.ptr.as_ptr() };

        // Atomic decrement with AcqRel ordering
        let old = obj.refcount.fetch_sub(1, Ordering::AcqRel);

        if old == 1 {
            // Refcount reached 0, deallocate
            // SAFETY: ptr was created with Box::into_raw
            // Reclaim ownership with Box::from_raw and drop
            unsafe {
                drop(Box::from_raw(self.ptr.as_ptr()));
            }
        }
    }

    /// Returns the object's class (isa pointer).
    ///
    /// # Returns
    ///
    /// Reference to the object's class.
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::{Class, Object};
    ///
    /// let class = Class::new_root("MyClass").unwrap();
    /// let obj = Object::new(&class).unwrap();
    ///
    /// assert_eq!(obj.class().name(), "MyClass");
    /// ```
    #[must_use]
    pub fn class(&self) -> Class {
        // SAFETY: self.ptr points to valid RawObject
        let obj = unsafe { &*self.ptr.as_ptr() };

        // Create Class from class_ptr
        // SAFETY: class_ptr points to ClassInner in arena (never deallocated)
        unsafe {
            Class {
                inner: NonNull::new_unchecked(obj.class_ptr as *mut _),
            }
        }
    }

    /// Returns the current reference count (for testing/debugging).
    ///
    /// # Returns
    ///
    /// Current reference count value.
    ///
    /// # Note
    ///
    /// This is primarily useful for testing. The refcount can change
    /// asynchronously due to concurrent retain/release operations.
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::{Class, Object};
    ///
    /// let class = Class::new_root("MyClass").unwrap();
    /// let obj = Object::new(&class).unwrap();
    ///
    /// assert_eq!(obj.refcount(), 1);
    /// ```
    #[must_use]
    pub fn refcount(&self) -> u32 {
        // SAFETY: self.ptr points to valid Raw`Object`
        let obj = unsafe { &*self.ptr.as_ptr() };

        // Load with Acquire ordering to see all previous releases
        obj.refcount.load(Ordering::Acquire)
    }

    /// Returns the raw pointer to the object data.
    ///
    /// This is used internally by the message dispatch system to pass
    /// the object pointer to method implementations.
    ///
    /// # Returns
    ///
    /// Raw pointer to the Raw`Object` (never null while object is alive).
    ///
    /// # Safety
    ///
    /// The returned pointer is valid only while the `Object` reference exists.
    /// Do not store it beyond the lifetime of the `Object` reference.
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::{Class, Object};
    ///
    /// # let class = Class::new_root("MyClass").unwrap();
    /// let obj = Object::new(&class).unwrap();
    /// let raw_ptr = obj.as_raw();
    /// ```
    #[must_use]
    pub fn as_raw(&self) -> ObjectPtr {
        // SAFETY: self.ptr is a valid NonNull<Raw`Object`> (guaranteed by `Object` invariants)
        unsafe { ObjectPtr::from_raw(self.ptr.as_ptr()) }
    }

    /// Sends a message to this object with no arguments.
    ///
    /// This is the primary method for dynamic message passing in the `OxideC` runtime.
    /// It performs method lookup, argument marshalling, and function pointer invocation.
    ///
    /// # Arguments
    ///
    /// * `selector` - The method selector to invoke
    ///
    /// # Returns
    ///
    /// - `Ok(Some(retval))` - `Method` returned a value (encoded as usize)
    /// - `Ok(None)` - `Method` returned void
    /// - `Err(Error::`Selector`NotFound)` - `Method` not found
    /// - `Err(Error::ArgumentCountMismatch)` - Wrong number of arguments
    ///
    /// # Thread Safety
    ///
    /// This method is thread-safe. Multiple threads can send messages concurrently.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use oxidec::{Class, Object, Selector};
    /// use oxidec::runtime::MessageArgs;
    /// use std::str::FromStr;
    ///
    /// # let class = Class::new_root("MyClass").unwrap();
    /// let obj = Object::new(&class).unwrap();
    /// let sel = Selector::from_str("doSomething").unwrap();
    ///
    /// // No arguments
    /// match obj.send_message(&sel, &MessageArgs::None) {
    ///     Ok(retval) => println!("Return value: {:?}", retval),
    ///     Err(e) => println!("Error: {:?}", e),
    /// }
    ///
    /// // One argument
    /// match obj.send_message(&sel, &MessageArgs::one(42)) {
    ///     Ok(retval) => println!("Return value: {:?}", retval),
    ///     Err(e) => println!("Error: {:?}", e),
    /// }
    ///
    /// // Two arguments
    /// match obj.send_message(&sel, &MessageArgs::two(10, 20)) {
    ///     Ok(retval) => println!("Return value: {:?}", retval),
    ///     Err(e) => println!("Error: {:?}", e),
    /// }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`Error::SelectorNotFound`] if the selector is not found in the
    /// object's class hierarchy, or [`Error::ArgumentCountMismatch`] if the
    /// number of arguments doesn't match the method signature.
    pub fn send_message(
        &self,
        selector: &Selector,
        args: &MessageArgs,
    ) -> Result<Option<usize>> {
        // SAFETY: self is a valid reference (lifetime protected)
        // The object's refcount ensures it remains alive during the call
        unsafe { crate::runtime::dispatch::send_message(self, selector, args) }
    }

    /// Checks if this object responds to a given selector.
    ///
    /// This method walks the inheritance chain to determine if the object
    /// (or any of its superclasses) implements a method with the given selector.
    ///
    /// # Arguments
    ///
    /// * `selector` - The method selector to check
    ///
    /// # Returns
    ///
    /// `true` if the object responds to this selector, `false` otherwise.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use oxidec::{Class, Object, Selector};
    /// use std::str::FromStr;
    ///
    /// # let class = Class::new_root("MyClass").unwrap();
    /// let obj = Object::new(&class).unwrap();
    /// let sel = Selector::from_str("doSomething").unwrap();
    ///
    /// if obj.responds_to(&sel) {
    ///     println!("Object responds to doSomething");
    /// } else {
    ///     println!("Object does not respond to doSomething");
    /// }
    /// ```
    #[must_use]
    pub fn responds_to(&self, selector: &Selector) -> bool {
        // Get object's class and lookup method (searches inheritance chain)
        let class = self.class();
        class.lookup_method(selector).is_some()
    }

    /// Sets the global forwarding hook for unhandled messages.
    ///
    /// The hook is called when a selector is not found in an object's class
    /// hierarchy. If the hook returns Some(target), the message is retried on
    /// the target object.
    ///
    /// # Priority
    ///
    /// Per-object hooks > Per-class hooks > Global hooks (this function)
    ///
    /// # Thread Safety
    ///
    /// This function is thread-safe. The last hook set wins.
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::{Class, Object, Selector};
    /// use std::sync::Mutex;
    ///
    /// static FORWARDING_TARGET: Mutex<Option<Object>> = Mutex::new(None);
    ///
    /// // Set up forwarding hook
    /// Object::set_global_forwarding_hook(|_obj, _sel| {
    ///     FORWARDING_TARGET.lock().unwrap().clone()
    /// });
    ///
    /// // Later: clear the hook
    /// Object::clear_global_forwarding_hook();
    /// ```
    ///
    /// # Safety
    ///
    /// Forwarding hooks must NOT re-enter the dispatch system to avoid deadlocks.
    /// Hooks should return quickly and avoid blocking operations.
    pub fn set_global_forwarding_hook(hook: crate::runtime::forwarding::GlobalForwardingHook) {
        crate::runtime::forwarding::set_global_forwarding_hook(hook);
    }

    /// Clears the global forwarding hook.
    ///
    /// After calling this, no global forwarding will occur (per-object and
    /// per-class hooks are unaffected).
    pub fn clear_global_forwarding_hook() {
        crate::runtime::forwarding::clear_global_forwarding_hook();
    }

    /// Sets the forwarding event callback for diagnostics.
    ///
    /// The callback is invoked for all forwarding-related events, including:
    /// - Forwarding attempts
    /// - Forwarding success
    /// - `DoesNotRecognizeSelector` invocations
    /// - Forwarding loop detection
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::Object;
    /// use oxidec::runtime::forwarding::ForwardingEvent;
    ///
    /// Object::set_forwarding_event_callback(|event| {
    ///     match event {
    ///         ForwardingEvent::ForwardingAttempt { object, selector, depth } => {
    ///             eprintln!("Forwarding attempt: {} -> {}, depth {}",
    ///                      object.class().name(), selector.name(), depth);
    ///         }
    ///         _ => { /* ... */ }
    ///     }
    /// });
    /// ```
    pub fn set_forwarding_event_callback(
        callback: crate::runtime::forwarding::ForwardingEventCallback,
    ) {
        crate::runtime::forwarding::set_forwarding_event_callback(callback);
    }

    /// Clears the forwarding event callback.
    pub fn clear_forwarding_event_callback() {
        crate::runtime::forwarding::clear_forwarding_event_callback();
    }
}

// SAFETY: Object is Send because:
// - Raw`Object` is heap-allocated with Box
// - Atomic refcounting prevents data races
// - `Class` pointer points to arena (never moves)
unsafe impl Send for Object {}

// SAFETY: Object is Sync because:
// - All accesses are through immutable references (retain/release/isa)
// - Atomic refcount prevents data races
// - `Class` pointer is immutable (set at creation, never changed)
unsafe impl Sync for Object {}

impl Clone for Object {
    fn clone(&self) -> Self {
        self.retain();

        // SAFETY: ptr is still valid (we just incremented refcount)
        Object { ptr: self.ptr }
    }
}

impl Drop for Object {
    fn drop(&mut self) {
        // Decrement refcount, deallocate if reaches 0
        self.release();
    }
}

impl PartialEq for Object {
    fn eq(&self, other: &Self) -> bool {
        // Pointer equality: same heap allocation
        std::ptr::eq(self.ptr.as_ptr(), other.ptr.as_ptr())
    }
}

impl Eq for Object {}

impl fmt::Debug for Object {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("`Object`")
            .field("class", &self.class().name())
            .field("refcount", &self.refcount())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;
    use crate::runtime::RuntimeString;
    use crate::runtime::get_global_arena;
    use crate::runtime::selector::SelectorHandle;

    unsafe extern "C" fn test_impl(
        _self: ObjectPtr,
        _cmd: SelectorHandle,
        _args: *const *mut u8,
        _ret: *mut u8,
    ) {
    }

    fn create_test_class(name: &str) -> Class {
        Class::new_root(name).expect("Failed to create test class")
    }

    #[test]
    fn test_object_creation() {
        let class = create_test_class("ObjCreateTest");
        let obj = Object::new(&class).unwrap();

        assert_eq!(obj.class().name(), "ObjCreateTest");
        assert_eq!(obj.refcount(), 1);
    }

    #[test]
    fn test_retain_increments_refcount() {
        let class = create_test_class("ObjRetainTest");
        let obj = Object::new(&class).unwrap();

        assert_eq!(obj.refcount(), 1);

        obj.retain();
        assert_eq!(obj.refcount(), 2);

        obj.retain();
        assert_eq!(obj.refcount(), 3);
    }

    #[test]
    fn test_release_decrements_refcount() {
        let class = create_test_class("ObjReleaseTest");
        let obj = Object::new(&class).unwrap();

        obj.retain();
        assert_eq!(obj.refcount(), 2);

        obj.release();
        assert_eq!(obj.refcount(), 1);
    }

    #[test]
    fn test_clone_increments_refcount() {
        let class = create_test_class("ObjCloneTest");
        let obj1 = Object::new(&class).unwrap();

        let obj2 = obj1.clone();

        assert_eq!(obj1.refcount(), 2);
        assert_eq!(obj2.refcount(), 2);

        // Both point to same object
        assert!(std::ptr::eq(obj1.ptr.as_ptr(), obj2.ptr.as_ptr()));
    }

    #[test]
    fn test_drop_decrements_refcount() {
        let class = create_test_class("ObjDropTest");
        let obj1 = Object::new(&class).unwrap();
        let obj2 = obj1.clone();

        assert_eq!(obj1.refcount(), 2);

        // Drop obj2
        drop(obj2);

        // Refcount should decrease
        assert_eq!(obj1.refcount(), 1);
    }

    #[test]
    fn test_object_equality() {
        let class = create_test_class("ObjEqualityTest");
        let obj1 = Object::new(&class).unwrap();
        let obj2 = Object::new(&class).unwrap();

        // Different objects are not equal
        assert_ne!(obj1, obj2);

        // Clone is equal
        let obj3 = obj1.clone();
        assert_eq!(obj1, obj3);
    }

    #[test]
    fn test_object_debug() {
        let class = create_test_class("ObjDebugTest");
        let obj = Object::new(&class).unwrap();

        let debug_str = format!("{obj:?}");

        assert!(debug_str.contains("ObjDebugTest"));
        assert!(debug_str.contains("refcount"));
    }

    #[test]
    #[should_panic(expected = "Reference count overflow")]
    fn test_refcount_overflow() {
        let class = create_test_class("ObjOverflowTest");
        let obj = Object::new(&class).unwrap();

        // Set refcount to MAX
        // SAFETY: Direct manipulation for testing
        unsafe {
            let raw = &*obj.ptr.as_ptr();
            raw.refcount.store(u32::MAX, Ordering::Release);
        }

        // Should panic on overflow
        obj.retain();
    }

    #[test]
    fn test_send_message_basic() {
        let class = create_test_class("SendMsgTest");
        let sel = Selector::from_str("testMethod").unwrap();
        let arena = get_global_arena();

        let method = crate::runtime::class::Method {
            selector: sel.clone(),
            imp: test_impl,
            types: RuntimeString::new("v@:", arena),
        };
        class.add_method(method).unwrap();

        // Create object and send message
        let obj = Object::new(&class).unwrap();
        let result = obj.send_message(&sel, &MessageArgs::None);

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), None); // Void return
    }

    #[test]
    fn test_send_message_selector_not_found() {
        let class = create_test_class("SendNotFoundTest");
        let sel = Selector::from_str("nonExistentMethod").unwrap();
        let obj = Object::new(&class).unwrap();

        let result = obj.send_message(&sel, &MessageArgs::None);
        assert!(matches!(result, Err(crate::error::Error::SelectorNotFound)));
    }

    #[test]
    fn test_responds_to() {
        let class = create_test_class("RespondsToTest");
        let sel = Selector::from_str("existingMethod").unwrap();
        let arena = get_global_arena();

        let method = crate::runtime::class::Method {
            selector: sel.clone(),
            imp: test_impl,
            types: RuntimeString::new("v@:", arena),
        };
        class.add_method(method).unwrap();

        let obj = Object::new(&class).unwrap();

        // `Object` should respond to existing method
        assert!(obj.responds_to(&sel));

        // `Object` should not respond to non-existent method
        let non_existent = Selector::from_str("nonExistentMethod").unwrap();
        assert!(!obj.responds_to(&non_existent));
    }

    #[test]
    fn test_responds_to_inherited() {
        // Test that responds_to works with inherited methods
        let parent = create_test_class("RespondsToParent");
        let sel = Selector::from_str("inheritedMethod").unwrap();
        let arena = get_global_arena();

        let method = crate::runtime::class::Method {
            selector: sel.clone(),
            imp: test_impl,
            types: RuntimeString::new("v@:", arena),
        };
        parent.add_method(method).unwrap();

        // Create child class
        let child = Class::new("RespondsToChild", &parent).unwrap();
        let obj = Object::new(&child).unwrap();

        // Child object should respond to parent's method
        assert!(obj.responds_to(&sel));
    }

    #[test]
    fn test_send_message_1_basic() {
        let class = create_test_class("SendMsg1Test");
        let sel = Selector::from_str("methodWithArg:").unwrap();
        let arena = get_global_arena();

        let method = crate::runtime::class::Method {
            selector: sel.clone(),
            imp: test_impl,
            types: RuntimeString::new("v@:i", arena), // void return, int arg
        };
        class.add_method(method).unwrap();

        let obj = Object::new(&class).unwrap();
        let result = obj.send_message(&sel, &MessageArgs::one(42));

        assert!(result.is_ok());
    }

    #[test]
    fn test_send_message_2_basic() {
        let class = create_test_class("SendMsg2Test");
        let sel = Selector::from_str("method:withArg2:").unwrap();
        let arena = get_global_arena();

        let method = crate::runtime::class::Method {
            selector: sel.clone(),
            imp: test_impl,
            types: RuntimeString::new("v@:ii", arena), // void, two int args
        };
        class.add_method(method).unwrap();

        let obj = Object::new(&class).unwrap();
        let result = obj.send_message(&sel, &MessageArgs::two(10, 20));

        assert!(result.is_ok());
    }
}
