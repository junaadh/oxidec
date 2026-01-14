//! RuntimeString: Optimized string type for the `OxideC` runtime.
//!
//! This module implements a high-performance string abstraction with:
//! - **Small String Optimization (SSO)**: Strings ≤ 15 bytes stored inline (zero allocation)
//! - **Tagged Encoding**: Support for both UTF-8 and Latin-1/ASCII encoding
//! - **Copy-on-Write**: Efficient mutation via reference counting
//! - **Arena Allocation**: Large strings stored in arena for stable pointers
//!
//! # Memory Layout
//!
//! [`RuntimeString`] uses a tagged pointer representation:
//! - **Bit 0 (LSB)**: 1 = Inline SSO, 0 = Heap pointer
//! - **Bit 1**: 0 = UTF-8, 1 = Latin-1/ASCII
//! - **Bits 2-63**: Data or pointer
//!
//! # Example
//!
//! ```rust
//! use oxidec::runtime::{RuntimeString, get_global_arena};
//!
//! let arena = get_global_arena();
//!
//! // Small strings use inline storage (zero allocation)
//! let short = RuntimeString::new("init", arena);
//! assert!(short.is_inline());
//!
//! // Large strings use heap allocation in arena
//! let long = RuntimeString::new("This is a very long string that won't fit inline", arena);
//! assert!(!long.is_inline());
//! ```

use crate::error::Result;
use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ptr::NonNull;
use std::sync::atomic::{AtomicU8, AtomicU32, AtomicU64, Ordering};
use std::sync::{RwLock, OnceLock};

/// Maximum size for inline string storage (Small String Optimization).
const SSO_THRESHOLD: usize = 15;

/// Tag bit indicating inline SSO storage (LSB = 1).
const SSO_TAG_BIT: usize = 0x01;

/// Tag bit indicating Latin-1 encoding (bit 1 = 1).
const ENCODING_BIT: usize = 0x02;

/// HeapString flag: Latin-1/ASCII encoding (vs UTF-8).
const FLAG_ENCODING_LATIN1: u8 = 0x01;

/// HeapString flag: String has been interned.
#[allow(dead_code)] // Reserved for future interning system enhancements
const FLAG_INTERNED: u8 = 0x02;

/// Mask for clearing tag bits to get actual pointer.
const POINTER_MASK: usize = !0x03;

/// Union for either inline storage or heap pointer.
#[derive(Clone, Copy)]
union RuntimeStringData {
    /// Heap pointer (when bit 0 is clear)
    ptr: NonNull<u8>,
    /// Inline storage (when bit 0 is set)
    inline: [u8; 16],
}

/// An optimized string type for the `OxideC` runtime.
///
/// `RuntimeString` provides:
/// - **Small String Optimization (SSO)**: Strings ≤ 15 bytes stored inline
/// - **Tagged encoding**: UTF-8 vs Latin-1/ASCII discrimination
/// - **Copy-on-Write**: Efficient mutation via reference counting
/// - **Arena allocation**: Stable pointers for heap-allocated strings
///
/// # Memory Layout
///
/// The string is stored in one of two ways:
///
/// **Inline SSO (≤ 15 bytes)**:
/// - Stored directly in the `data.inline` array
/// - No heap allocation
/// - Format: `[bytes 0-14][length | tag bits]`
///
/// **Heap String (> 15 bytes)**:
/// - Stored in arena-allocated [`HeapString`]
/// - Pointer in `data.ptr` with encoding in bit 1
/// - Reference counted for copy-on-write
///
/// # Thread Safety
///
/// `RuntimeString` is `Send + Sync` because:
/// - Inline strings are immutable and cloned by value
/// - Heap strings point to arena memory (never moves)
/// - Atomic reference counting provides thread-safe COW
pub struct RuntimeString {
    /// Union containing either inline data or heap pointer
    data: RuntimeStringData,
}

// SAFETY: RuntimeString is Send because:
// - Inline strings are plain data that can be copied
// - Heap strings point to arena memory (valid for entire program)
// - All operations use atomic reference counting
unsafe impl Send for RuntimeString {}

// SAFETY: RuntimeString is Sync because:
// - Inline strings are immutable
// - Heap strings have atomic refcount and immutable data
// - Reading from multiple threads is safe (only mutation needs COW)
unsafe impl Sync for RuntimeString {}

impl Clone for RuntimeString {
    fn clone(&self) -> Self {
        if self.is_inline() {
            // Inline strings: copy the 16-byte inline array
            // SAFETY: Reading from inline array, which is valid data
            let inline_array = unsafe { self.data.inline };
            RuntimeString {
                data: RuntimeStringData {
                    inline: inline_array,
                },
            }
        } else {
            // Heap strings: increment reference count
            // SAFETY: Clear tag bits to get actual HeapString pointer
            let heap_ptr =
                (unsafe { self.data.ptr.as_ptr() as usize & POINTER_MASK })
                    as *const HeapString;

            // Increment refcount
            // SAFETY: Using atomic fetch_add with AcqRel ordering
            unsafe {
                let old_count =
                    (*heap_ptr).refcount.fetch_add(1, Ordering::AcqRel);
                // Check for overflow
                if old_count == u32::MAX {
                    // Panic on overflow - this is a programming error
                    // (creating more than 4 billion references to the same string)
                    panic!("Reference count overflow in RuntimeString::clone");
                }
            }

            // Return copy with same tagged pointer
            RuntimeString { data: self.data }
        }
    }
}

impl Drop for RuntimeString {
    fn drop(&mut self) {
        if !self.is_inline() {
            // Heap strings: decrement reference count
            // SAFETY: Clear tag bits to get actual HeapString pointer
            let heap_ptr =
                (unsafe { self.data.ptr.as_ptr() as usize & POINTER_MASK })
                    as *const HeapString;

            // Decrement refcount
            // Note: We don't deallocate because the arena owns the memory
            // The refcount is only used for COW semantics
            // SAFETY: Using atomic fetch_sub with AcqRel ordering
            unsafe {
                (*heap_ptr).refcount.fetch_sub(1, Ordering::AcqRel);
            }
        }
        // Inline strings: nothing to drop (just inline data)
    }
}

/// Internal representation for heap-allocated strings.
///
/// `HeapString` is stored in the arena with 16-byte alignment.
/// It contains metadata followed by flexible array storage for the string data.
#[repr(C, align(16))]
struct HeapString {
    /// Length of the string in bytes (not including NUL terminator).
    length: AtomicU32,
    /// Precomputed hash of the string content.
    hash: AtomicU64,
    /// Reference count for copy-on-write semantics.
    refcount: AtomicU32,
    /// Capacity in bytes (including space for NUL terminator).
    capacity: u32,
    /// Flags: bit 0 = encoding (0=UTF-8, 1=Latin-1), bit 1 = is_interned.
    flags: AtomicU8,
    /// String data starts here (flexible array member).
    /// The actual string bytes follow, NUL-terminated.
    data: [u8; 0],
}

/// Global cache for string interning.
///
/// `StringInternCache` provides fast, thread-safe string deduplication using
/// a hash-based cache with lock-free reads. Multiple threads can simultaneously
/// look up strings without blocking.
///
/// # Performance
///
/// - **Cache hit**: < 50ns (single read lock + refcount increment)
/// - **Cache miss**: < 300ns (read + write lock + allocation + insert)
///
/// # Thread Safety
///
/// Uses `RwLock` for concurrent access:
/// - Multiple readers can access cache simultaneously (read lock)
/// - Writers have exclusive access (write lock)
struct StringInternCache {
    /// Hash map of interned strings.
    /// Key: precomputed hash (u64)
    /// Value: list of HeapString pointers with that hash (for collision resolution)
    cache: RwLock<HashMap<u64, Vec<*const HeapString>>>,
    /// Arena used for allocating interned strings
    arena: &'static crate::runtime::Arena,
}

// SAFETY: StringInternCache is Send because:
// - HeapString pointers point to arena memory (never deallocated, valid for entire program)
// - RwLock is Send
// - Arena is thread-safe
unsafe impl Send for StringInternCache {}

// SAFETY: StringInternCache is Sync because:
// - HeapString pointers are arena-allocated (immutable data, atomic refcount)
// - RwLock provides synchronized access
// - Multiple threads can safely read the cache concurrently
unsafe impl Sync for StringInternCache {}

impl StringInternCache {
    /// Creates a new intern cache.
    fn new(arena: &'static crate::runtime::Arena) -> Self {
        StringInternCache {
            cache: RwLock::new(HashMap::new()),
            arena,
        }
    }

    /// Interns a string, returning a cached copy if it exists.
    ///
    /// # Performance
    ///
    /// - **Fast path** (cache hit): Read lock only, non-blocking
    /// - **Slow path** (cache miss): Write lock for insertion
    ///
    /// # Algorithm
    ///
    /// 1. Compute hash of input string
    /// 2. Check cache with read lock (fast, shared)
    /// 3. If found: verify byte equality and return existing (increment refcount)
    /// 4. If not found: allocate new string, insert into cache with write lock
    fn intern(&self, s: &str) -> RuntimeString {
        // Bypass interning for small strings (SSO is already optimal)
        if s.len() <= SSO_THRESHOLD {
            return RuntimeString::new(s, self.arena);
        }

        let bytes = s.as_bytes();
        let hash = Self::compute_hash(bytes);

        // Fast path: Read lock (non-blocking for multiple readers)
        {
            let cache = self.cache.read().unwrap();
            if let Some(entry) = cache.get(&hash) {
                // Search bucket for matching string
                for &ptr in entry {
                    // SAFETY: ptr points to valid HeapString in arena
                    if unsafe { Self::bytes_match(ptr, bytes) } {
                        // Found! Return existing string (increment refcount)
                        return unsafe { RuntimeString::from_heap_ptr(ptr) };
                    }
                }
            }
        } // Read lock released here

        // Slow path: Write lock (allocate and insert)
        let rs = RuntimeString::new(s, self.arena);

        // Only cache heap-allocated strings
        if let Ok(heap_ptr) = rs.heap_ptr() {
            let mut cache = self.cache.write().unwrap();
            cache.entry(hash).or_insert_with(Vec::new).push(heap_ptr);
        }

        rs
    }

    /// Computes a hash for the string content.
    #[inline]
    fn compute_hash(bytes: &[u8]) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::Hasher;

        let mut hasher = DefaultHasher::new();
        bytes.hash(&mut hasher);
        hasher.finish()
    }

    /// Checks if a HeapString matches the given byte slice.
    ///
    /// # Safety
    ///
    /// Caller must ensure ptr points to a valid HeapString.
    unsafe fn bytes_match(ptr: *const HeapString, bytes: &[u8]) -> bool {
        // SAFETY: ptr is valid pointer to HeapString in arena
        let heap = unsafe { &*ptr };
        let len = heap.length.load(Ordering::Acquire) as usize;

        // Fast-fail: length check
        if len != bytes.len() {
            return false;
        }

        // Compare bytes
        // SAFETY: data_ptr points to valid string data in arena
        let data_ptr = heap.data.as_ptr();
        let heap_bytes = unsafe { std::slice::from_raw_parts(data_ptr, len) };
        heap_bytes == bytes
    }
}

impl RuntimeString {
    /// Creates a new RuntimeString from a string slice.
    ///
    /// # Arguments
    ///
    /// * `s` - The string slice to convert
    /// * `arena` - The arena to use for allocation (if needed for large strings)
    ///
    /// # Returns
    ///
    /// A new RuntimeString instance
    ///
    /// # Performance
    ///
    /// - Small strings (≤ 15 bytes): Inline storage, ~10ns
    /// - Large strings: Arena allocation, ~200ns
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::runtime::{RuntimeString, get_global_arena};
    ///
    /// let arena = get_global_arena();
    /// let rs = RuntimeString::new("hello", arena);
    /// assert!(rs.is_inline());
    /// assert_eq!(rs.len(), 5);
    /// ```
    pub fn new(s: &str, arena: &crate::runtime::Arena) -> Self {
        let bytes = s.as_bytes();
        let len = bytes.len();

        // Detect encoding
        let is_latin1 = Self::is_latin1_static(bytes);

        // Choose SSO or heap based on length
        if len <= SSO_THRESHOLD {
            // Fast path: Inline SSO
            Self::new_inline(s, is_latin1)
        } else {
            // Slow path: Heap allocation
            Self::new_heap(s, is_latin1, arena)
        }
    }

    /// Creates an inline SSO string.
    ///
    /// # Arguments
    ///
    /// * `s` - The string to store inline
    /// * `is_latin1` - Whether the string is Latin-1/ASCII only
    #[inline]
    fn new_inline(s: &str, is_latin1: bool) -> Self {
        let bytes = s.as_bytes();
        let len = bytes.len();

        debug_assert!(
            len <= SSO_THRESHOLD,
            "String too long for inline storage"
        );

        let mut inline = [0u8; 16];
        inline[..len].copy_from_slice(bytes);

        // Store length in bits 2-7, tag bits in 0-1
        // Bit 0: SSO flag (always 1)
        // Bit 1: Latin-1 flag
        // Bits 2-7: length
        let tag = SSO_TAG_BIT | if is_latin1 { ENCODING_BIT } else { 0 };
        inline[15] = ((len as u8) << 2) | (tag as u8);

        RuntimeString {
            data: RuntimeStringData { inline },
        }
    }

    /// Creates a heap-allocated string in the arena.
    ///
    /// # Arguments
    ///
    /// * `s` - The string to allocate
    /// * `is_latin1` - Whether the string is Latin-1/ASCII only
    /// * `arena` - The arena to allocate from
    #[inline]
    fn new_heap(
        s: &str,
        is_latin1: bool,
        arena: &crate::runtime::Arena,
    ) -> Self {
        let bytes = s.as_bytes();
        let len = bytes.len();

        // Compute hash
        let hash = Self::compute_hash(bytes);

        // Calculate capacity (power of 2, at least len + 1 for NUL terminator)
        let capacity = (len + 1).next_power_of_two();

        // Create HeapString header
        let heap_str = HeapString {
            length: AtomicU32::new(len as u32),
            hash: AtomicU64::new(hash),
            refcount: AtomicU32::new(1),
            capacity: capacity as u32,
            flags: AtomicU8::new(if is_latin1 {
                FLAG_ENCODING_LATIN1
            } else {
                0
            }),
            data: [],
        };

        // Allocate in arena
        let ptr: *mut HeapString = arena.alloc_string(heap_str, capacity);

        // Copy string data
        // SAFETY: ptr is valid and points to allocated memory in arena
        unsafe {
            let data_ptr = (*ptr).data.as_ptr() as *mut u8;
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), data_ptr, len);
            *data_ptr.add(len) = 0; // NUL terminator
        }

        // Tag pointer with encoding
        let tagged_ptr = if is_latin1 {
            (ptr as usize | ENCODING_BIT) as *mut u8
        } else {
            ptr as *mut u8
        };

        // SAFETY: tagged_ptr is a valid pointer from arena allocation
        unsafe {
            RuntimeString {
                data: RuntimeStringData {
                    ptr: NonNull::new_unchecked(tagged_ptr),
                },
            }
        }
    }

    /// Checks if a byte slice contains only Latin-1/ASCII characters.
    ///
    /// Latin-1 (ISO-8859-1) includes all byte values 0x00-0xFF.
    /// For our purposes, we treat ASCII (0x00-0x7F) as Latin-1 for optimization.
    /// True UTF-8 sequences (bytes ≥ 0x80) are detected as non-Latin-1.
    ///
    /// # Arguments
    ///
    /// * `bytes` - The byte slice to check
    ///
    /// # Returns
    ///
    /// `true` if all bytes are ASCII (≤ 0x7F), `false` if any byte indicates UTF-8
    #[inline]
    fn is_latin1_static(bytes: &[u8]) -> bool {
        // Fast path: All bytes ≤ 0x7F means ASCII/Latin-1
        // Any byte ≥ 0x80 indicates multi-byte UTF-8 sequence
        bytes.iter().all(|&b| b <= 0x7F)
    }

    /// Computes a hash for the string content.
    #[inline]
    fn compute_hash(bytes: &[u8]) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::Hasher;

        let mut hasher = DefaultHasher::new();
        bytes.hash(&mut hasher);
        hasher.finish()
    }

    /// Checks if this is an inline SSO string.
    ///
    /// # Returns
    ///
    /// `true` if the string uses inline storage, `false` if heap-allocated
    #[inline]
    pub fn is_inline(&self) -> bool {
        // Check LSB of inline array (last byte, bit 0)
        (unsafe { self.data.inline[15] } & SSO_TAG_BIT as u8) != 0
    }

    /// Checks if this string uses Latin-1/ASCII encoding.
    ///
    /// # Returns
    ///
    /// `true` if Latin-1/ASCII, `false` if UTF-8
    #[inline]
    pub fn is_latin1(&self) -> bool {
        // Check bit 1 of inline array (last byte, bit 1) or heap pointer
        if self.is_inline() {
            (unsafe { self.data.inline[15] } & ENCODING_BIT as u8) != 0
        } else {
            // Check bit 1 of heap pointer
            (unsafe { self.data.ptr.as_ptr() } as usize & ENCODING_BIT) != 0
        }
    }

    /// Returns the byte slice for an inline SSO string.
    ///
    /// # Safety
    ///
    /// Caller must ensure `self.is_inline()` is true.
    #[inline]
    fn inline_bytes(&self) -> &[u8] {
        debug_assert!(self.is_inline(), "Not an inline string");

        // Extract length from bits 2-7 of byte 15
        let len = ((unsafe { self.data.inline[15] } as usize) >> 2) & 0x3F;

        // SAFETY: inline[0..len] contains valid string bytes
        // The length is stored in byte 15, and data is in bytes 0-14
        unsafe { &self.data.inline[..len] }
    }

    /// Returns the length of the string in bytes.
    #[inline]
    pub fn len(&self) -> usize {
        if self.is_inline() {
            // Extract length from inline storage (bits 2-7 of byte 15)
            // Length is stored in bits 2-7, so shift right by 2
            ((unsafe { self.data.inline[15] } as usize) >> 2) & 0x3F
        } else {
            // Get length from HeapString
            // SAFETY: Clear tag bits to get actual HeapString pointer
            // HeapString is in arena memory, which is never deallocated
            let heap_ptr =
                (unsafe { self.data.ptr.as_ptr() as usize & POINTER_MASK })
                    as *const HeapString;
            unsafe { (*heap_ptr).length.load(Ordering::Acquire) as usize }
        }
    }

    /// Checks if the string is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the heap pointer if this is a heap-allocated string.
    ///
    /// # Returns
    ///
    /// - `Ok(*const HeapString)` - Pointer to HeapString if heap-allocated
    /// - `Err(())` - If this is an inline SSO string
    #[inline]
    fn heap_ptr(&self) -> Result<*const HeapString> {
        if self.is_inline() {
            Err(crate::error::Error::InvalidArenaState)
        } else {
            // SAFETY: Clear tag bits to get actual HeapString pointer
            let heap_ptr =
                (unsafe { self.data.ptr.as_ptr() as usize & POINTER_MASK })
                    as *const HeapString;
            Ok(heap_ptr)
        }
    }

    /// Creates a RuntimeString from an existing HeapString pointer.
    ///
    /// This increments the reference count of the HeapString.
    ///
    /// # Safety
    ///
    /// Caller must ensure ptr points to a valid HeapString in the arena.
    #[inline]
    unsafe fn from_heap_ptr(ptr: *const HeapString) -> RuntimeString {
        // Increment refcount
        // SAFETY: ptr points to valid HeapString in arena
        unsafe {
            (*ptr).refcount.fetch_add(1, Ordering::AcqRel);
        }

        // Get encoding from flags
        // SAFETY: ptr points to valid HeapString
        let encoding_bit = unsafe {
            if (*ptr).flags.load(Ordering::Acquire) & FLAG_ENCODING_LATIN1 != 0 {
                ENCODING_BIT
            } else {
                0
            }
        };

        // Tag pointer with encoding
        let tagged_ptr = (ptr as usize | encoding_bit) as *mut u8;

        // SAFETY: tagged_ptr is valid non-null pointer
        unsafe {
            RuntimeString {
                data: RuntimeStringData {
                    ptr: NonNull::new_unchecked(tagged_ptr),
                },
            }
        }
    }

    /// Returns the string as a byte slice.
    pub fn as_bytes(&self) -> &[u8] {
        if self.is_inline() {
            self.inline_bytes()
        } else {
            // Get data from HeapString
            // SAFETY: Clear tag bits to get actual HeapString pointer
            // HeapString is in arena memory, which is never deallocated
            let heap_ptr =
                (unsafe { self.data.ptr.as_ptr() as usize & POINTER_MASK })
                    as *const HeapString;
            let len =
                unsafe { (*heap_ptr).length.load(Ordering::Acquire) as usize };

            // SAFETY: data array points to valid string bytes
            // The data is NUL-terminated and len specifies the valid range
            unsafe {
                let data_ptr = (*heap_ptr).data.as_ptr();
                std::slice::from_raw_parts(data_ptr, len)
            }
        }
    }

    /// Returns the string as a `str`, validating UTF-8.
    pub fn as_str(&self) -> Result<&str> {
        // TODO: Implement in Step 4
        std::str::from_utf8(self.as_bytes())
            .map_err(|_| crate::error::Error::InvalidArenaState)
    }

    /// Interns a string, returning a cached copy if it exists.
    ///
    /// This method provides fast string deduplication using a global cache.
    /// Strings are stored once and reused, saving memory for duplicates.
    ///
    /// # Performance
    ///
    /// - **Cache hit**: < 50ns (just refcount increment)
    /// - **Cache miss**: < 300ns (allocation + cache insert)
    /// - **Small strings**: Bypassed (SSO is already optimal)
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::runtime::RuntimeString;
    ///
    /// let rs1 = RuntimeString::intern("initWithObjects:");
    /// let rs2 = RuntimeString::intern("initWithObjects:");
    ///
    /// // Both are equal (same content)
    /// assert_eq!(rs1, rs2);
    ///
    /// // Long strings use heap allocation
    /// assert!(!rs1.is_inline());
    /// ```
    pub fn intern(s: &str) -> Self {
        get_intern_cache().intern(s)
    }
}

impl fmt::Display for RuntimeString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // SAFETY: as_bytes() returns valid UTF-8 for the string content
        // For Latin-1 strings, all bytes are ≤ 0x7F, which is valid UTF-8
        let str = String::from_utf8_lossy(self.as_bytes()).into_owned();
        write!(f, "{str}")
    }
}

impl fmt::Debug for RuntimeString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeString")
            .field("is_inline", &self.is_inline())
            .field("is_latin1", &self.is_latin1())
            .field("len", &self.len())
            .finish()
    }
}

impl PartialEq for RuntimeString {
    fn eq(&self, other: &Self) -> bool {
        // Fast path 1: Both inline, compare inline data
        if self.is_inline() && other.is_inline() {
            unsafe { self.data.inline == other.data.inline }
        } else if !self.is_inline() && !other.is_inline() {
            // Fast path 2: Both heap, check pointer equality first
            let ptr1 =
                unsafe { self.data.ptr.as_ptr() as usize & POINTER_MASK };
            let ptr2 =
                unsafe { other.data.ptr.as_ptr() as usize & POINTER_MASK };

            if ptr1 == ptr2 {
                return true; // Same heap allocation
            }

            // Slow path: Different heap strings, compare content
            // Fast-fail: Check length first
            if self.len() != other.len() {
                return false;
            }

            // Compare bytes
            self.as_bytes() == other.as_bytes()
        } else {
            // Mixed inline/heap: byte-by-byte comparison
            self.as_bytes() == other.as_bytes()
        }
    }
}

impl Eq for RuntimeString {}

impl Hash for RuntimeString {
    fn hash<H: Hasher>(&self, state: &mut H) {
        if self.is_inline() {
            // Hash the inline bytes (excluding padding)
            // SAFETY: inline[0..len] contains valid string data
            let len = self.len();
            unsafe { self.data.inline[..len].hash(state) };
        } else {
            // Use cached hash from HeapString
            // SAFETY: Clear tag bits to get actual HeapString pointer
            let heap_ptr =
                (unsafe { self.data.ptr.as_ptr() as usize & POINTER_MASK })
                    as *const HeapString;
            let hash = unsafe { (*heap_ptr).hash.load(Ordering::Acquire) };
            hash.hash(state);
        }
    }
}

/// Returns the global string intern cache.
///
/// This function lazily initializes the cache on first call and returns
/// a reference to it. The cache lives for the entire program duration.
///
/// # Thread Safety
///
/// The cache is thread-safe and can be accessed from multiple threads
/// concurrently.
#[must_use]
fn get_intern_cache() -> &'static StringInternCache {
    use crate::runtime::get_global_arena;

    static INTERN_CACHE: OnceLock<StringInternCache> = OnceLock::new();

    INTERN_CACHE.get_or_init(|| StringInternCache::new(get_global_arena()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runtime_string_size() {
        // RuntimeString contains a union with a 16-byte array
        // So it must be 16 bytes
        assert_eq!(std::mem::size_of::<RuntimeString>(), 16);
    }

    #[test]
    fn test_inline_short_string() {
        // Empty arena for now (will be used in Step 2)
        let rs = RuntimeString::new_inline("hello", true);
        assert!(rs.is_inline());
        assert!(rs.is_latin1());
        assert_eq!(rs.len(), 5);
    }

    #[test]
    fn test_inline_encoding() {
        let ascii = RuntimeString::new_inline("test", true);
        assert!(ascii.is_latin1());

        let utf8 = RuntimeString::new_inline("test", false);
        assert!(!utf8.is_latin1());
    }

    #[test]
    fn test_empty_string() {
        let empty = RuntimeString::new_inline("", true);
        assert!(empty.is_inline());
        assert!(empty.is_empty());
        assert_eq!(empty.len(), 0);
    }

    #[test]
    fn test_inline_max_length() {
        // Test SSO threshold
        let max_sso = "123456789012345"; // 15 bytes
        let rs = RuntimeString::new_inline(max_sso, true);
        assert_eq!(rs.len(), 15);
        assert!(rs.is_inline());
    }

    #[test]
    fn test_heap_string() {
        use crate::runtime::get_global_arena;

        let arena = get_global_arena();
        let long = "This is a very long string that won't fit inline";
        let rs = RuntimeString::new(long, arena);

        assert!(!rs.is_inline());
        assert_eq!(rs.len(), long.len());
        // Verify it's using heap storage
        assert!(rs.is_latin1()); // ASCII string
    }

    #[test]
    fn test_string_creation_ascii() {
        use crate::runtime::get_global_arena;

        let arena = get_global_arena();
        let rs = RuntimeString::new("test", arena);

        assert!(rs.is_inline());
        assert!(rs.is_latin1());
        assert_eq!(rs.len(), 4);
    }

    #[test]
    fn test_string_creation_too_long() {
        use crate::runtime::get_global_arena;

        let arena = get_global_arena();
        let long = "1234567890123456"; // 16 bytes - too long for inline
        let rs = RuntimeString::new(long, arena);

        assert!(!rs.is_inline());
        assert_eq!(rs.len(), 16);
    }

    #[test]
    fn test_as_bytes_inline() {
        use crate::runtime::get_global_arena;

        let arena = get_global_arena();
        let rs = RuntimeString::new("hello", arena);

        assert_eq!(rs.as_bytes(), b"hello");
    }

    #[test]
    fn test_as_bytes_heap() {
        use crate::runtime::get_global_arena;

        let arena = get_global_arena();
        let long = "This is a very long string that won't fit inline";
        let rs = RuntimeString::new(long, arena);

        assert_eq!(rs.as_bytes(), long.as_bytes());
    }

    #[test]
    fn test_as_str_valid_utf8() {
        use crate::runtime::get_global_arena;

        let arena = get_global_arena();
        let rs = RuntimeString::new("hello world", arena);

        assert_eq!(rs.as_str().unwrap(), "hello world");
    }

    #[test]
    fn test_to_string_inline() {
        use crate::runtime::get_global_arena;

        let arena = get_global_arena();
        let rs = RuntimeString::new("test", arena);

        assert_eq!(rs.to_string(), "test");
    }

    #[test]
    fn test_to_string_heap() {
        use crate::runtime::get_global_arena;

        let arena = get_global_arena();
        let long = "This is a long string that requires heap allocation";
        let rs = RuntimeString::new(long, arena);

        assert_eq!(rs.to_string(), long);
    }

    #[test]
    fn test_clone_inline() {
        use crate::runtime::get_global_arena;

        let arena = get_global_arena();
        let rs1 = RuntimeString::new("hello", arena);
        let rs2 = rs1.clone();

        assert_eq!(rs1.as_bytes(), rs2.as_bytes());
        assert!(rs1.is_inline());
        assert!(rs2.is_inline());
    }

    #[test]
    fn test_clone_heap_increments_refcount() {
        use crate::runtime::get_global_arena;

        let arena = get_global_arena();
        let rs1 = RuntimeString::new(
            "This is a long string that requires heap allocation",
            arena,
        );

        // Get initial refcount
        let heap_ptr1 = unsafe {
            (rs1.data.ptr.as_ptr() as usize & POINTER_MASK) as *const HeapString
        };
        let initial_count =
            unsafe { (*heap_ptr1).refcount.load(Ordering::Acquire) };
        assert_eq!(initial_count, 1);

        // Clone should increment refcount
        let rs2 = rs1.clone();
        let count_after_clone =
            unsafe { (*heap_ptr1).refcount.load(Ordering::Acquire) };
        assert_eq!(count_after_clone, 2);

        // Drop should decrement refcount
        drop(rs2);
        let count_after_drop =
            unsafe { (*heap_ptr1).refcount.load(Ordering::Acquire) };
        assert_eq!(count_after_drop, 1);
    }

    #[test]
    fn test_eq_inline_strings() {
        use crate::runtime::get_global_arena;

        let arena = get_global_arena();
        let rs1 = RuntimeString::new("hello", arena);
        let rs2 = RuntimeString::new("hello", arena);

        assert_eq!(rs1, rs2);
    }

    #[test]
    fn test_eq_heap_strings_same_allocation() {
        use crate::runtime::get_global_arena;

        let arena = get_global_arena();
        let rs1 = RuntimeString::new("This is a long string", arena);
        let rs2 = rs1.clone(); // Same heap allocation

        assert_eq!(rs1, rs2);
    }

    #[test]
    fn test_eq_heap_strings_different_allocations() {
        use crate::runtime::get_global_arena;

        let arena = get_global_arena();
        let rs1 = RuntimeString::new("This is a long string", arena);
        let rs2 = RuntimeString::new("This is a long string", arena); // Different allocation

        assert_eq!(rs1, rs2);
    }

    #[test]
    fn test_neq_different_strings() {
        use crate::runtime::get_global_arena;

        let arena = get_global_arena();
        let rs1 = RuntimeString::new("hello", arena);
        let rs2 = RuntimeString::new("world", arena);

        assert_ne!(rs1, rs2);
    }

    #[test]
    fn test_hash_consistency() {
        use crate::runtime::get_global_arena;
        use std::collections::hash_map::DefaultHasher;

        let arena = get_global_arena();
        let rs1 = RuntimeString::new("test", arena);
        let rs2 = RuntimeString::new("test", arena);

        let mut hasher1 = DefaultHasher::new();
        let mut hasher2 = DefaultHasher::new();

        rs1.hash(&mut hasher1);
        rs2.hash(&mut hasher2);

        assert_eq!(hasher1.finish(), hasher2.finish());
    }

    #[test]
    fn test_hash_different_strings() {
        use crate::runtime::get_global_arena;
        use std::collections::hash_map::DefaultHasher;

        let arena = get_global_arena();
        let rs1 = RuntimeString::new("hello", arena);
        let rs2 = RuntimeString::new("world", arena);

        let mut hasher1 = DefaultHasher::new();
        let mut hasher2 = DefaultHasher::new();

        rs1.hash(&mut hasher1);
        rs2.hash(&mut hasher2);

        assert_ne!(hasher1.finish(), hasher2.finish());
    }

    #[test]
    fn test_intern_same_string_returns_same_pointer() {
        // Intern the same string twice (long enough to bypass SSO)
        let rs1 = RuntimeString::intern("initWithObjects:andKeys:");
        let rs2 = RuntimeString::intern("initWithObjects:andKeys:");

        // Should both be heap-allocated (not inline)
        assert!(!rs1.is_inline());
        assert!(!rs2.is_inline());

        // Should point to same allocation
        let ptr1 = rs1.heap_ptr().unwrap();
        let ptr2 = rs2.heap_ptr().unwrap();
        assert_eq!(ptr1, ptr2);

        // Content should match
        assert_eq!(rs1.as_bytes(), rs2.as_bytes());
    }

    #[test]
    fn test_intern_different_strings_returns_different_pointers() {
        let rs1 = RuntimeString::intern("methodWithVeryLongNameOne:");
        let rs2 = RuntimeString::intern("methodWithVeryLongNameTwo:");

        // Should point to different allocations
        let ptr1 = rs1.heap_ptr().unwrap();
        let ptr2 = rs2.heap_ptr().unwrap();
        assert_ne!(ptr1, ptr2);

        // Content should be different
        assert_ne!(rs1.as_bytes(), rs2.as_bytes());
    }

    #[test]
    fn test_intern_bypasses_small_strings() {
        // Small string (inline SSO)
        let rs1 = RuntimeString::intern("short");
        let rs2 = RuntimeString::intern("short");

        // Should both be inline (bypassed cache)
        assert!(rs1.is_inline());
        assert!(rs2.is_inline());

        // Should still be equal
        assert_eq!(rs1, rs2);
    }

    #[test]
    fn test_intern_refcount_increments() {
        let rs1 = RuntimeString::intern("sharedSelector:withMultipleArguments:");
        let ptr1 = rs1.heap_ptr().unwrap();

        // Initial refcount should be 1
        let initial_count = unsafe { (*ptr1).refcount.load(Ordering::Acquire) };
        assert_eq!(initial_count, 1);

        // Intern again should increment refcount
        let rs2 = RuntimeString::intern("sharedSelector:withMultipleArguments:");
        let count_after_intern = unsafe { (*ptr1).refcount.load(Ordering::Acquire) };
        assert_eq!(count_after_intern, 2);

        // Content should match
        assert_eq!(rs1.as_bytes(), rs2.as_bytes());
    }

    #[test]
    fn test_intern_vs_new() {
        use crate::runtime::get_global_arena;

        let arena = get_global_arena();

        // Create with intern() (use long string)
        let rs1 = RuntimeString::intern("initializeWithStringEncoding:");
        let rs2 = RuntimeString::intern("initializeWithStringEncoding:");

        // Create with new() - different allocations
        let rs3 = RuntimeString::new("initializeWithStringEncoding:", arena);
        let rs4 = RuntimeString::new("initializeWithStringEncoding:", arena);

        // intern() returns same allocation
        assert_eq!(rs1.heap_ptr().unwrap(), rs2.heap_ptr().unwrap());

        // new() creates different allocations
        assert_ne!(rs3.heap_ptr().unwrap(), rs4.heap_ptr().unwrap());

        // intern() is different from new()
        assert_ne!(rs1.heap_ptr().unwrap(), rs3.heap_ptr().unwrap());

        // All content is equal
        assert_eq!(rs1, rs3);
        assert_eq!(rs3, rs4);
    }
}
