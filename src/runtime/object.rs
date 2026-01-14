//! Object allocation and lifecycle management for the `OxideC` runtime.
//!
//! This module implements the object system with:
//! - Reference counting with atomic operations
//! - Automatic memory management (retain/release)
//! - Thread-safe object lifecycle
//! - Class isa pointers for dynamic dispatch
//!
//! # Architecture
//!
//! Objects are heap-allocated with manual memory management:
//! - Each object has an atomic reference count
//! - Objects are deallocated when refcount reaches 0
//! - Thread-safe via atomic operations (AcqRel ordering)
//! - Clone is shallow (pointer duplication with refcount increment)
//!
//! # Thread Safety
//!
//! Objects are `Send + Sync` when reference counting is atomic:
//! - Multiple threads can hold references to same object
//! - retain/release are thread-safe (atomic operations)
//! - Object data access requires external synchronization (Phase 2)

use crate::error::Result;
use crate::runtime::Class;
use std::fmt;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicU32, Ordering};

// Raw pointer to ClassInner (defined in class.rs)
// We use raw pointer to avoid circular dependency
type ClassInnerPtr = *const ();

/// Raw object representation allocated on heap.
///
/// This struct is **not** allocated in the arena (unlike Selector/Class)
/// because objects have individual lifetimes controlled by reference counting.
#[repr(C)]
pub(crate) struct RawObject {
    /// Isa pointer: class that this object is an instance of
    /// Points to ClassInner in arena (never deallocated)
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

/// Object represents a runtime instance with dynamic dispatch.
///
/// Objects are reference-counted and support:
/// - Automatic memory management (retain/release)
/// - Dynamic dispatch via isa pointer
/// - Thread-safe reference counting
///
/// # Memory Layout
///
/// Objects use manual memory management for performance:
/// - RawObject allocated on heap (not arena, for per-instance lifecycle)
/// - Reference counted with atomic operations
/// - Class pointer (isa) for dynamic dispatch
///
/// # Thread Safety
///
/// Objects are `Send + Sync` when reference counting is atomic:
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
    /// Panics if refcount overflows (u32::MAX).
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
        if old == u32::MAX {
            panic!("Reference count overflow in Object::retain");
        }
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
        // SAFETY: self.ptr points to valid RawObject
        let obj = unsafe { &*self.ptr.as_ptr() };

        // Load with Acquire ordering to see all previous releases
        obj.refcount.load(Ordering::Acquire)
    }
}

// SAFETY: Object is Send because:
// - RawObject is heap-allocated with Box
// - Atomic refcounting prevents data races
// - Class pointer points to arena (never moves)
unsafe impl Send for Object {}

// SAFETY: Object is Sync because:
// - All accesses are through immutable references (retain/release/isa)
// - Atomic refcount prevents data races
// - Class pointer is immutable (set at creation, never changed)
unsafe impl Sync for Object {}

impl Clone for Object {
    fn clone(&self) -> Self {
        self.retain();

        // SAFETY: ptr is still valid (we just incremented refcount)
        Object {
            ptr: self.ptr,
        }
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
        f.debug_struct("Object")
            .field("class", &self.class().name())
            .field("refcount", &self.refcount())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

        let debug_str = format!("{:?}", obj);

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
}
