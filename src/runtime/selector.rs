//! Selector interning and caching for the `OxideC` runtime.
//!
//! This module implements a global selector registry with interning, ensuring
//! that each unique selector name has exactly one `Selector` instance. This
//! enables fast pointer equality comparison and efficient method lookup.
//!
//! # Architecture
//!
//! Selectors are **globally interned** in the global arena:
//! - Each unique name maps to exactly one `Selector` instance
//! - Pointers are stable for the entire program duration
//! - Comparison is O(1) pointer equality
//! - Hash is precomputed once at creation time
//!
//! # Thread Safety
//!
//! The selector registry is thread-safe and supports concurrent interning from
//! multiple threads. Uses `RwLock` for bucket access and atomic operations for
//! initialization.

use crate::error::Result;
use crate::runtime::{get_global_arena, RuntimeString};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ptr::NonNull;
use std::sync::RwLock;
use std::sync::OnceLock;

/// Number of hash buckets in the selector registry (power of 2 for fast modulo).
const NUM_BUCKETS: usize = 256;

/// Interned selector stored in the global arena.
///
/// This struct is allocated in the global arena and never deallocated.
/// Selectors have `'static` lifetime - they live for the entire program duration.
#[repr(C)]
struct InternedSelector {
    /// Selector name (e.g., "initWithObjects:")
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
    /// Key: precomputed hash (mod NUM_BUCKETS)
    /// Value: linked list of InternedSelector pointers
    buckets: RwLock<Vec<*const InternedSelector>>,
}

// SAFETY: SelectorRegistry is Send + Sync because:
// - InternedSelector pointers point to arena memory (never deallocated)
// - RwLock provides synchronized access to buckets
// - Arena ensures proper alignment and validity
unsafe impl Send for SelectorRegistry {}
unsafe impl Sync for SelectorRegistry {}

/// Global selector registry instance.
static REGISTRY: OnceLock<SelectorRegistry> = OnceLock::new();

/// Selector represents a unique method name in the runtime.
///
/// Selectors are **globally interned** - each unique name has exactly one
/// `Selector` instance. This enables:
/// - Fast pointer equality comparison
/// - Efficient method lookup
/// - Precomputed hash for O(1) hashing
///
/// # Memory Management
///
/// Selectors are stored in the global arena with `'static` lifetime:
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

impl Selector {
    /// Returns the selector for a given name, interning if necessary.
    ///
    /// # Arguments
    ///
    /// * `name` - Selector name (e.g., "init", "initWithObjects:")
    ///
    /// # Returns
    ///
    /// Returns `Ok(Selector)` with the interned selector, or `Err` on allocation failure.
    ///
    /// # Performance
    ///
    /// - Cache hit: < 50ns (read lock only)
    /// - Cache miss: < 300ns (write lock + allocation + insert)
    ///
    /// # Thread Safety
    ///
    /// Multiple threads can call this concurrently. Returns the same
    /// `Selector` instance for the same name (pointer equality).
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::Selector;
    ///
    /// let sel = Selector::from_str("doSomething:withObject:").unwrap();
    /// assert_eq!(sel.name(), "doSomething:withObject:");
    /// ```
    pub fn from_str(name: &str) -> Result<Self> {
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
                // SAFETY: current points to valid InternedSelector in global arena
                // - Arena is never deallocated (static lifetime)
                // - Pointer is properly aligned (Arena ensures 16-byte alignment)
                // - No mutable references exist (RwLock ensures exclusive access)
                let interned = unsafe { &*current };

                if interned.hash == hash {
                    // Hash matches, verify name equality
                    // SAFETY: interned.name is valid RuntimeString
                    // unwrap() is safe because RuntimeString in arena is always valid
                    if interned.name.as_str().unwrap() == name {
                        // Found existing selector
                        // SAFETY: current is not null (checked above)
                        return Ok(Selector {
                            ptr: unsafe { NonNull::new_unchecked(current as *mut InternedSelector) },
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
            // unwrap() is safe because RuntimeString in arena is always valid
            if interned.hash == hash && interned.name.as_str().unwrap() == name {
                // Another thread inserted it, return existing
                return Ok(Selector {
                    ptr: unsafe { NonNull::new_unchecked(current as *mut InternedSelector) },
                });
            }
            current = interned.next;
        }

        // Allocate new InternedSelector in global arena
        let arena = get_global_arena();

        // Allocate RuntimeString for the name
        let name_str = RuntimeString::new(name, arena);

        // Create InternedSelector struct
        let interned = InternedSelector {
            name: name_str,
            hash,
            next: buckets[bucket_idx], // Insert at head of list
        };

        // Allocate InternedSelector struct in arena
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
    ///
    /// let sel = Selector::from_str("initWithObjects:").unwrap();
    /// assert_eq!(sel.name(), "initWithObjects:");
    /// ```
    #[must_use]
    pub fn name(&self) -> &str {
        // SAFETY: self.ptr points to valid InternedSelector in global arena
        // - Arena is never deallocated
        // - Pointer is properly aligned
        // - InternedSelector.name is valid RuntimeString
        // unwrap() is safe because RuntimeString in arena is always valid
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
        // SAFETY: self.ptr points to valid InternedSelector
        unsafe { *(&(*self.ptr.as_ptr()).hash) }
    }
}

// SAFETY: Selector is Send because:
// - InternedSelector is in arena (never moves, 'static lifetime)
// - Pointer is valid for entire program duration
// - No mutable state (all fields immutable after creation)
unsafe impl Send for Selector {}

// SAFETY: Selector is Sync because:
// - All methods only read immutable data
// - InternedSelector never changes after creation
// - Arena provides stable pointers
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
        f.debug_struct("Selector")
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
        let sel = Selector::from_str("initWithObjects:count:").unwrap();
        assert_eq!(sel.name(), "initWithObjects:count:");
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
        let sel = Selector::from_str("debugMethod:").unwrap();
        let debug_str = format!("{:?}", sel);

        assert!(debug_str.contains("debugMethod:"));
        assert!(debug_str.contains("Selector"));
    }

    #[test]
    fn test_selector_thread_safety() {
        let name = "concurrentSelector:";
        let handles: Vec<_> = (0..10)
            .map(|_| {
                let name = name.to_string();
                thread::spawn(move || Selector::from_str(&name).unwrap())
            })
            .collect();

        let selectors: Vec<_> = handles
            .into_iter()
            .map(|h| h.join().unwrap())
            .collect();

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
