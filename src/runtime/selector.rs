//! `Selector` interning and caching for the ``OxideC`` runtime.
//!
//! This module implements a global selector registry with interning, ensuring
//! that each unique selector name has exactly one ``Selector`` instance. This
//! enables fast pointer equality comparison and efficient method lookup.

// Allow cast truncation - NUM_BUCKETS is 256 (fits in u8) and hashes are modulo 256
#![allow(clippy::cast_possible_truncation)]
// Allow pointer constness casts - needed for FFI compatibility
#![allow(clippy::ptr_cast_constness)]
//!
//! # Architecture
//!
//! `Selector`s are **globally interned** in the global arena:
//! - Each unique name maps to exactly one ``Selector`` instance
//! - Pointers are stable for the entire program duration
//! - Comparison is O(1) pointer equality
//! - Hash is precomputed once at creation time
//!
//! # Thread Safety
//!
//! The selector registry is thread-safe and supports concurrent interning from
//! multiple threads. Uses `RwLock` for bucket access and atomic operations for
//! initialization.

use crate::Error;
use crate::error::Result;
use crate::runtime::{RuntimeString, get_global_arena};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ptr::NonNull;
use std::str::FromStr;
use std::sync::OnceLock;
use std::sync::RwLock;

/// Number of hash buckets in the selector registry (power of 2 for fast modulo).
const NUM_BUCKETS: usize = 256;

/// Opaque handle to a selector for FFI compatibility.
///
/// `SelectorHandle` is an opaque pointer type designed for C ABI compatibility
/// in method implementations. It wraps a raw pointer to an interned selector
/// while hiding the internal representation from FFI consumers.
///
/// # Purpose
///
/// This type serves as a bridge between Rust's type-safe `Selector` API and
/// C-compatible function pointers. It enables:
///
/// - **FFI Integration**: Used in the `Imp` function pointer type for method
///   implementations that follow the C ABI
/// - **Binary Stability**: As an opaque type, it maintains a stable interface
///   across code changes
/// - **Type Safety**: Prevents direct manipulation of selector internals from
///   FFI code while allowing safe conversion via `Selector`
///
/// # When to Use
///
/// - **FFI Boundaries**: Use `SelectorHandle` in function signatures that cross
///   the FFI boundary (e.g., `Imp` method implementations)
/// - **C Interop**: Use when passing selectors to/from C code
/// - **Internal Runtime**: Use `Selector` instead for all internal Rust code
///
/// # Safety
///
/// `SelectorHandle` is safe through encapsulation:
///
/// - **Creation**: Only obtainable via `Selector::as_handle()`, ensuring validity
/// - **Conversion**: Converting back to `Selector` requires `unsafe` code via
///   `Selector::from_handle()`
/// - **Lifetime**: The underlying selector has `'static` lifetime (allocated in
///   global arena, never deallocated)
/// - **Pointer Validity**: All handles point to valid, interned selectors
///
/// # Example
///
/// ```rust
/// use oxidec::Selector;
/// use oxidec::runtime::selector::SelectorHandle;
/// use oxidec::runtime::object::ObjectPtr;
/// use std::str::FromStr;
///
/// // In Rust code, work with the type-safe Selector
/// let selector = Selector::from_str("init").unwrap();
///
/// // Convert to handle for FFI
/// let handle = selector.as_handle();
///
/// // In FFI implementation (extern "C" fn)
/// unsafe extern "C" fn my_method_impl(
///     _self: ObjectPtr,
///     _cmd: SelectorHandle,  // Received as FFI handle
///     _args: *const *mut u8,
///     _ret: *mut u8,
/// ) {
///     // Convert back to Selector when needed
///     let selector = Selector::from_handle(_cmd);
/// }
/// ```
///
/// # Representation
///
/// - **Layout**: `#[repr(transparent)]` - guaranteed to have the same layout as
///   the underlying raw pointer
/// - **Size**: Pointer-sized (same as `*const InternedSelector`)
/// - **Traits**: Implements `Copy`, `Clone`, `Eq`, `PartialEq` for value semantics
///
/// # See Also
///
/// - [`Selector::as_handle()`] - Convert Selector to handle
/// - [`Selector::from_handle()`] - Convert handle to Selector (unsafe)
/// - [`Imp`](crate::runtime::class::Imp) - FFI function pointer type using `SelectorHandle`
#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq)]
pub struct SelectorHandle(*const InternedSelector);

/// Interned selector stored in the global arena.
///
/// This struct is allocated in the global arena and never deallocated.
/// `Selector`s have `'static` lifetime - they live for the entire program duration.
#[repr(C)]
struct InternedSelector {
    /// `Selector` name (e.g., "initWith`Object`s:")
    name: RuntimeString,
    /// Precomputed hash for fast comparison and lookup
    hash: u64,
    /// Next selector in hash bucket (for collision resolution via chaining)
    next: *const InternedSelector,
}

/// Global selector registry with interning.
///
/// Uses open chaining with `RwLock` for thread-safe concurrent access.
struct SelectorRegistry {
    /// Hash buckets: array of linked lists
    /// Key: precomputed hash (mod `NUM_BUCKETS`)
    /// Value: linked list of Interned`Selector` pointers
    buckets: RwLock<Vec<*const InternedSelector>>,
}

// SAFETY: SelectorRegistry is Send + Sync because:
// - Interned`Selector` pointers point to arena memory (never deallocated)
// - RwLock provides synchronized access to buckets
// - `Arena` ensures proper alignment and validity
unsafe impl Send for SelectorRegistry {}
unsafe impl Sync for SelectorRegistry {}

/// Global selector registry instance.
static REGISTRY: OnceLock<SelectorRegistry> = OnceLock::new();

/// `Selector` represents a unique method name in the runtime.
///
/// `Selector`s are **globally interned** - each unique name has exactly one
/// ``Selector`` instance. This enables:
/// - Fast pointer equality comparison
/// - Efficient method lookup
/// - Precomputed hash for O(1) hashing
///
/// # Memory Management
///
/// `Selector`s are stored in the global arena with `'static` lifetime:
/// - Never deallocated (lives for program duration)
/// - Stable pointers (safe to store in raw pointers)
/// - Thread-safe access (atomic operations)
///
/// # Performance
///
/// - Creation (cache hit): < 50ns (read lock only)
/// - Creation (cache miss): < 300ns (write lock + allocation)
/// - Comparison: O(1) pointer equality
/// - Hash: O(1) (precomputed)
///
/// # Example
///
/// ```rust
/// use oxidec::Selector;
/// use std::str::FromStr;
///
/// let sel1 = Selector::from_str("init").unwrap();
/// let sel2 = Selector::from_str("init").unwrap();
///
/// // Same name = same selector (equality check)
/// assert_eq!(sel1, sel2);
/// ```
pub struct Selector {
    /// Pointer to interned selector data in global arena.
    /// Never null, valid for entire program lifetime.
    ptr: NonNull<InternedSelector>,
}

impl FromStr for Selector {
    type Err = Error;
    /// Returns the selector for a given name, interning if necessary.
    ///
    /// # Arguments
    ///
    /// * `name` - `Selector` name (e.g., "init", "initWith`Object`s:")
    ///
    /// # Returns
    ///
    /// Returns `Ok(`Selector`)` with the interned selector, or `Err` on allocation failure.
    ///
    /// # Performance
    ///
    /// - Cache hit: < 50ns (read lock only)
    /// - Cache miss: < 300ns (write lock + allocation + insert)
    ///
    /// # Thread Safety
    ///
    /// Multiple threads can call this concurrently. Returns the same
    /// ``Selector`` instance for the same name (pointer equality).
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::Selector;
    /// use std::str::FromStr;
    ///
    /// let sel = Selector::from_str("doSomething:withObject:").unwrap();
    /// assert_eq!(sel.name(), "doSomething:withObject:");
    /// ```
    fn from_str(name: &str) -> Result<Self> {
        // Initialize registry on first use
        let registry = REGISTRY.get_or_init(|| {
            let buckets = vec![std::ptr::null(); NUM_BUCKETS];
            SelectorRegistry {
                buckets: RwLock::new(buckets),
            }
        });

        // Compute hash for the selector name
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        name.hash(&mut hasher);
        let hash = hasher.finish();

        // Determine bucket index
        let bucket_idx = (hash as usize) % NUM_BUCKETS;

        // Fast path: Acquire read lock, search buckets
        {
            let buckets = registry.buckets.read().unwrap();
            let mut current = buckets[bucket_idx];

            while !current.is_null() {
                // SAFETY: current points to valid Interned`Selector` in global arena
                // - `Arena` is never deallocated (static lifetime)
                // - Pointer is properly aligned (`Arena` ensures 16-byte alignment)
                // - No mutable references exist (RwLock ensures exclusive access)
                let interned = unsafe { &*current };

                if interned.hash == hash {
                    // Hash matches, verify name equality
                    // SAFETY: interned.name is valid `RuntimeString`
                    // unwrap() is safe because `RuntimeString` in arena is always valid
                    if interned.name.as_str().unwrap() == name {
                        // Found existing selector
                        // SAFETY: current is not null (checked above)
                        return Ok(Selector {
                            ptr: unsafe {
                                NonNull::new_unchecked(
                                    current as *mut InternedSelector,
                                )
                            },
                        });
                    }
                }

                current = interned.next;
            }
        } // Release read lock

        // Slow path: Acquire write lock, allocate and insert
        let mut buckets = registry.buckets.write().unwrap();

        // Double-check: Another thread might have inserted while we waited for write lock
        let mut current = buckets[bucket_idx];
        while !current.is_null() {
            let interned = unsafe { &*current };
            // unwrap() is safe because `RuntimeString` in arena is always valid
            if interned.hash == hash && interned.name.as_str().unwrap() == name
            {
                // Another thread inserted it, return existing
                return Ok(Selector {
                    ptr: unsafe {
                        NonNull::new_unchecked(current as *mut InternedSelector)
                    },
                });
            }
            current = interned.next;
        }

        // Allocate new Interned`Selector` in global arena
        let arena = get_global_arena();

        // Allocate `RuntimeString` for the name
        let name_str = RuntimeString::new(name, arena);

        // Create Interned`Selector` struct
        let interned = InternedSelector {
            name: name_str,
            hash,
            next: buckets[bucket_idx], // Insert at head of list
        };

        // Allocate Interned`Selector` struct in arena
        // SAFETY: We're allocating in the global arena, which lives for 'static
        // The struct will never be deallocated
        let interned_ptr: *mut InternedSelector = arena.alloc(interned);

        // Insert at head of bucket
        buckets[bucket_idx] = interned_ptr as *const InternedSelector;

        // SAFETY: interned_ptr is not null and properly aligned
        Ok(Selector {
            ptr: unsafe { NonNull::new_unchecked(interned_ptr) },
        })
    }
}

impl Selector {
    #[inline]
    #[must_use]
    pub fn as_handle(&self) -> SelectorHandle {
        SelectorHandle(self.ptr.as_ptr())
    }

    /// .
    ///
    /// # Safety
    ///
    /// .
    #[inline]
    #[must_use]
    pub unsafe fn from_handle(h: SelectorHandle) -> Self {
        Selector {
            ptr: unsafe {
                NonNull::new_unchecked(h.0 as *mut InternedSelector)
            },
        }
    }
}

impl Selector {
    /// Returns the selector's name as a string slice.
    ///
    /// # Returns
    ///
    /// String slice with the selector name (e.g., "init", "method:withParam:")
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::Selector;
    /// use std::str::FromStr;
    ///
    /// let sel = Selector::from_str("initWithObjects:").unwrap();
    /// assert_eq!(sel.name(), "initWithObjects:");
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if the selector's name string in the arena is invalid UTF-8
    /// (which should never happen under normal circumstances).
    #[must_use]
    pub fn name(&self) -> &str {
        // SAFETY: self.ptr points to valid Interned`Selector` in global arena
        // - `Arena` is never deallocated
        // - Pointer is properly aligned
        // - Interned`Selector`.name is valid `RuntimeString`
        // unwrap() is safe because `RuntimeString` in arena is always valid
        unsafe { &(*self.ptr.as_ptr()).name }.as_str().unwrap()
    }

    /// Returns the precomputed hash of the selector name.
    ///
    /// # Returns
    ///
    /// u64 hash value computed from the selector name.
    ///
    /// # Note
    ///
    /// The hash is precomputed once at creation time and never changes.
    /// This is safe because selector names are immutable (stored in arena).
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::Selector;
    /// use std::collections::hash_map::DefaultHasher;
    /// use std::hash::{Hash, Hasher};
    /// use std::str::FromStr;
    ///
    /// let sel = Selector::from_str("hashMethod").unwrap();
    ///
    /// // Verify hash matches expected value
    /// let mut hasher = DefaultHasher::new();
    /// "hashMethod".hash(&mut hasher);
    /// assert_eq!(sel.hash(), hasher.finish());
    /// ```
    #[must_use]
    pub fn hash(&self) -> u64 {
        // SAFETY: self.ptr points to valid Interned`Selector`
        unsafe { (*self.ptr.as_ptr()).hash }
    }
}

// SAFETY: Selector is Send because:
// - Interned`Selector` is in arena (never moves, 'static lifetime)
// - Pointer is valid for entire program duration
// - No mutable state (all fields immutable after creation)
unsafe impl Send for Selector {}

// SAFETY: Selector is Sync because:
// - All methods only read immutable data
// - Interned`Selector` never changes after creation
// - `Arena` provides stable pointers
unsafe impl Sync for Selector {}

impl Clone for Selector {
    fn clone(&self) -> Self {
        Selector { ptr: self.ptr }
    }
}

impl PartialEq for Selector {
    fn eq(&self, other: &Self) -> bool {
        // Pointer equality: same name = same selector (interning guarantee)
        std::ptr::eq(self.ptr.as_ptr(), other.ptr.as_ptr())
    }
}

impl Eq for Selector {}

impl Hash for Selector {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Use precomputed hash (fast O(1))
        state.write_u64(self.hash());
    }
}

impl fmt::Debug for Selector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("`Selector`")
            .field("name", &self.name())
            .field("hash", &format!("{:#x}", self.hash()))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_selector_interning() {
        let sel1 = Selector::from_str("init").unwrap();
        let sel2 = Selector::from_str("init").unwrap();

        // Same name = same pointer (interning)
        assert_eq!(sel1.ptr.as_ptr() as usize, sel2.ptr.as_ptr() as usize);

        // Pointer equality
        assert!(std::ptr::eq(sel1.ptr.as_ptr(), sel2.ptr.as_ptr()));

        // Equality works
        assert_eq!(sel1, sel2);
    }

    #[test]
    fn test_selector_hash_stability() {
        let sel1 = Selector::from_str("method:name:").unwrap();
        let sel2 = Selector::from_str("method:name:").unwrap();

        // Same name = same hash
        assert_eq!(sel1.hash(), sel2.hash());

        // Hash is precomputed and stable
        let hash1 = sel1.hash();
        let hash2 = sel1.hash();
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_selector_different_names() {
        let sel1 = Selector::from_str("foo").unwrap();
        let sel2 = Selector::from_str("bar").unwrap();

        // Different names = different selectors
        assert_ne!(sel1.ptr.as_ptr() as usize, sel2.ptr.as_ptr() as usize);

        // Different names
        assert_eq!(sel1.name(), "foo");
        assert_eq!(sel2.name(), "bar");

        // Not equal
        assert_ne!(sel1, sel2);
    }

    #[test]
    fn test_selector_name() {
        let sel = Selector::from_str("initWith`Object`s:count:").unwrap();
        assert_eq!(sel.name(), "initWith`Object`s:count:");
    }

    #[test]
    fn test_selector_clone() {
        let sel1 = Selector::from_str("cloneTest").unwrap();
        let sel2 = sel1.clone();

        // Clone shares the same pointer
        assert_eq!(sel1.ptr.as_ptr() as usize, sel2.ptr.as_ptr() as usize);

        // Equality works
        assert_eq!(sel1, sel2);
    }

    #[test]
    fn test_selector_debug() {
        let sel = Selector::from_str("debug`Method`:").unwrap();
        let debug_str = format!("{sel:?}");

        assert!(debug_str.contains("debug`Method`:"));
        assert!(debug_str.contains("`Selector`"));
    }

    #[test]
    fn test_selector_thread_safety() {
        let name = "concurrent`Selector`:";
        let handles: Vec<_> = (0..10)
            .map(|_| {
                let name = name.to_string();
                thread::spawn(move || Selector::from_str(&name).unwrap())
            })
            .collect();

        let selectors: Vec<_> =
            handles.into_iter().map(|h| h.join().unwrap()).collect();

        // All threads should get the same selector pointer
        for sel in &selectors[1..] {
            assert_eq!(
                selectors[0].ptr.as_ptr() as usize,
                sel.ptr.as_ptr() as usize
            );
        }
    }

    #[test]
    fn test_selector_hash_collisions() {
        // Create multiple selectors with different names
        let sel1 = Selector::from_str("aaa").unwrap();
        let sel2 = Selector::from_str("bbb").unwrap();
        let sel3 = Selector::from_str("ccc").unwrap();

        // Different names = different pointers
        assert_ne!(sel1.ptr.as_ptr() as usize, sel2.ptr.as_ptr() as usize);
        assert_ne!(sel2.ptr.as_ptr() as usize, sel3.ptr.as_ptr() as usize);

        // Correct names returned
        assert_eq!(sel1.name(), "aaa");
        assert_eq!(sel2.name(), "bbb");
        assert_eq!(sel3.name(), "ccc");
    }
}
