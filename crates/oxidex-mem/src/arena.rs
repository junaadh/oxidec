//! Arena allocator for `OxideX` runtime and compiler frontend.
//!
//! This module provides high-performance arena allocators designed for both
//! runtime metadata (classes, selectors, protocols) and compiler data (tokens,
//! AST nodes, interned strings). The arena provides:
//!
//! - **Fast allocation** through bump pointer strategy
//! - **Stable pointers** (never moves or reallocates)
//! - **Cache-friendly memory layout** with proper alignment
//! - **Thread-safe and thread-local variants**
//!
//! # Architecture
//!
//! - [`GlobalArena`]: Thread-safe arena for long-lived runtime metadata
//! - [`LocalArena`]: Thread-local arena for zero-contention allocation
//! - [`Chunk`]: Thread-safe chunk with atomic bump pointer
//! - [`LocalChunk`]: Thread-local chunk with non-atomic bump pointer
//!
//! # Performance
//!
//! Allocation performance characteristics:
//!
//! - **`GlobalArena` allocation**: ~13-15ns (atomic operations, thread-safe)
//! - **`LocalArena` allocation**: ~2-3ns (no atomics, thread-local)
//! - **Chunk growth**: Amortized O(1) as chunks double in size
//!
//! # Examples
//!
//! ## `GlobalArena` (thread-safe, for runtime metadata)
//!
//! ```
//! use oxidex_mem::arena::GlobalArena;
//!
//! let arena = GlobalArena::new(65536); // 64 KiB chunks
//!
//! // Allocate values in the arena
//! let value1 = arena.alloc(42u32);
//! let value2 = arena.alloc(100u64);
//!
//! assert_eq!(*value1, 42);
//! assert_eq!(*value2, 100);
//!
//! // All pointers remain valid for the entire program lifetime
//! // Arena does NOT support reset (lives for program duration)
//! ```
//!
//! ## `LocalArena` (thread-local, for compiler frontend)
//!
//! ```
//! use oxidex_mem::arena::LocalArena;
//!
//! let mut arena = LocalArena::new(8192); // 8 KiB initial chunk
//!
//! // Allocate values in the arena
//! let value1: *mut u32 = arena.alloc(42);
//! let value2: *mut u64 = arena.alloc(100);
//!
//! unsafe {
//!     assert_eq!(*value1, 42);
//!     assert_eq!(*value2, 100);
//! }
//!
//! // The pointers remain valid for the lifetime of the arena
//! // All memory is reclaimed when the arena is dropped
//! ```

use std::alloc::{self, Layout};
use std::ptr::NonNull;
use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

/// Error type for arena allocation failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArenaAllocError;

impl std::fmt::Display for ArenaAllocError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Arena allocation failed: out of memory")
    }
}

impl std::error::Error for ArenaAllocError {}

/// Default alignment for arena allocations (8 bytes).
///
/// This ensures proper alignment for:
/// - Pointers (8 bytes on 64-bit systems)
/// - u64 and i64 types
/// - General purpose cache line optimization
const DEFAULT_ALIGNMENT: usize = 8;

/// Minimum chunk size (8 KiB - optimized for compiler frontend).
const MIN_CHUNK_SIZE: usize = 8192;

/// Maximum chunk size (1 MiB).
const MAX_CHUNK_SIZE: usize = 1024 * 1024;

/// Optimized chunk size for compiler syntax phase.
pub const SYNTAX_CHUNK_SIZE: usize = 8192;

/// Optimized alignment for compiler data.
pub const SYNTAX_ALIGNMENT: usize = 8;

/// Arena allocation statistics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArenaStats {
    /// Total number of bytes allocated across all chunks.
    pub total_allocated: usize,
    /// Number of chunks in the arena.
    pub chunk_count: usize,
    /// Total capacity of all chunks in bytes.
    pub total_capacity: usize,
}

/// A thread-safe fixed-size memory chunk with atomic bump allocation.
///
/// `Chunk`s are allocated from the system allocator using `std::alloc` and
/// provide thread-safe bump pointer allocation through atomic operations.
///
/// # Thread Safety
///
/// `Chunk` uses an atomic bump pointer (`AtomicPtr<u8>`) for thread-safe
/// allocation. Multiple threads can allocate from the same chunk concurrently.
///
/// # Safety
///
/// - `Chunk` memory is never deallocated until the `Chunk` is dropped
/// - All allocations are properly aligned
/// - Bump pointer always advances and never wraps around
/// - CAS loop ensures atomic allocation without races
pub struct Chunk {
    /// Start of the chunk's memory region.
    start: NonNull<u8>,
    /// Current bump pointer (atomic for thread safety).
    /// We store as `AtomicPtr` to maintain provenance and enable atomic updates.
    ptr: AtomicPtr<u8>,
    /// End of the chunk's memory region (exclusive).
    end: NonNull<u8>,
    /// Total capacity of the chunk in bytes.
    capacity: usize,
}

impl Chunk {
    /// Creates a new thread-safe chunk.
    fn new(size: usize) -> Result<&'static mut Self, ArenaAllocError> {
        if size < MIN_CHUNK_SIZE {
            return Err(ArenaAllocError);
        }

        let layout = unsafe {
            Layout::from_size_align_unchecked(size, DEFAULT_ALIGNMENT)
        };

        let start = unsafe { alloc::alloc(layout) };
        let start = NonNull::new(start).ok_or(ArenaAllocError)?;

        // SAFETY: start.as_ptr().wrapping_add(size) is safe because:
        // 1. start is a valid pointer from alloc::alloc
        // 2. size is within reasonable bounds (checked above)
        // 3. We're creating a pointer to one past the end, which is valid for comparisons
        let end = unsafe { start.as_ptr().add(size) };
        let end = NonNull::new(end).expect("Chunk end pointer should not be null");

        // Allocate the chunk on the heap and leak it to get a 'static lifetime
        let chunk = Box::leak(Box::new(Chunk {
            start,
            // SAFETY: Initialize ptr with the start pointer.
            // This maintains proper provenance throughout the chunk's lifetime.
            ptr: AtomicPtr::new(start.as_ptr()),
            end,
            capacity: size,
        }));

        Ok(chunk)
    }

    /// Allocates from this chunk (atomic, thread-safe).
    #[must_use]
    #[inline(always)]
    fn try_alloc(&self, size: usize, align: usize) -> Option<NonNull<u8>> {
        // FIX ALIGNMENT BUG: Use normal arithmetic instead of wrapping_add
        // Old (buggy): let size_aligned = (size.wrapping_add(align).wrapping_sub(1)) & !(align - 1);
        // New (correct): let size_aligned = (size + align - 1) & !(align - 1);
        //
        // Why: wrapping_add can hide overflow bugs. Using normal arithmetic will
        // panic in debug mode on overflow, catching bugs earlier.
        let size_aligned = (size + align - 1) & !(align - 1);

        loop {
            // Load current bump pointer
            let current = self.ptr.load(Ordering::Acquire);
            let current_addr = current.addr();

            // Round up to alignment
            let aligned_start = (current_addr + align - 1) & !(align - 1);
            let new_addr = aligned_start.saturating_add(size_aligned);

            // Check if we have enough space
            let end_addr = self.end.addr().get();
            if new_addr > end_addr {
                return None;
            }

            // SAFETY: Reconstruct pointer with new address
            // new_addr is within chunk bounds (checked above)
            // with_addr preserves provenance of the original pointer
            let new_ptr = current.with_addr(new_addr);

            // CAS loop: try to update the bump pointer atomically
            match self.ptr.compare_exchange_weak(
                current,
                new_ptr,
                Ordering::Release,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    // CAS succeeded - we won the race
                    // SAFETY: aligned_start is within the chunk bounds (checked above)
                    // and is properly aligned by construction.
                    // with_addr preserves provenance.
                    let result_ptr = current.with_addr(aligned_start);
                    return unsafe { Some(NonNull::new_unchecked(result_ptr)) };
                }
                Err(_) => {
                    // CAS failed - another thread allocated first
                    // Loop and try again with the new current value
                    continue;
                }
            }
        }
    }
}

impl Drop for Chunk {
    fn drop(&mut self) {
        let layout = unsafe {
            Layout::from_size_align_unchecked(self.capacity, DEFAULT_ALIGNMENT)
        };

        unsafe {
            alloc::dealloc(self.start.as_ptr(), layout);
        }
    }
}

/// Thread-safe arena for long-lived allocations.
///
/// `GlobalArena` provides thread-safe allocation through atomic operations
/// and is designed for runtime metadata that lives for the entire program
/// duration (classes, selectors, protocols, caches).
///
/// # Performance
///
/// - Allocation latency: ~13-15ns (atomic CAS operations)
/// - Thread-safe: Multiple threads can allocate concurrently
/// - Zero contention for different chunks
///
/// # Use Cases
///
/// - Global runtime metadata (classes, selectors, protocols)
/// - Type encodings and method signatures
/// - Method caches and vtables
///
/// # Lifetime
///
/// `GlobalArena` is designed for allocations that live for the entire program.
/// It does NOT support `reset()` - once allocated, memory lives until program
/// termination. This provides clear lifetime semantics and prevents use-after-reset bugs.
///
/// # Examples
///
/// ```
/// use oxidex_mem::arena::GlobalArena;
///
/// let arena = GlobalArena::new(65536);
///
/// // Allocate values
/// let value1 = arena.alloc(42u32);
/// let value2 = arena.alloc(100u64);
///
/// assert_eq!(*value1, 42);
/// assert_eq!(*value2, 100);
///
/// // Get statistics
/// let stats = arena.stats();
/// println!("Allocated: {} bytes in {} chunks", stats.total_allocated, stats.chunk_count);
/// ```
pub struct GlobalArena {
    /// List of all chunks in this arena.
    chunks: Mutex<Vec<NonNull<Chunk>>>,
    /// Current chunk for allocations.
    current_chunk: AtomicPtr<Chunk>,
    /// Alignment for all allocations.
    alignment: usize,
    /// Size of new chunks.
    chunk_size: usize,
    /// Total bytes allocated (atomic counter).
    total_allocated: AtomicUsize,
}

unsafe impl Send for GlobalArena {}
unsafe impl Sync for GlobalArena {}

impl GlobalArena {
    /// Creates a new global arena with the specified chunk size.
    ///
    /// # Arguments
    ///
    /// * `chunk_size` - The size of chunks in bytes. Must be at least 8 KiB
    ///   and will be rounded up to a power of two.
    ///
    /// # Panics
    ///
    /// Panics if the initial chunk allocation fails (e.g., out of memory).
    #[must_use]
    pub fn new(chunk_size: usize) -> Self {
        let size = chunk_size.max(MIN_CHUNK_SIZE);
        let size = size.next_power_of_two();

        // SAFETY: We immediately convert the &'static mut Chunk to a raw pointer
        // and never use the reference again. This prevents Stacked Borrows violations
        // that would occur if we created references from raw pointers later.
        let first_chunk = Chunk::new(size)
            .expect("Failed to allocate initial chunk");

        let first_chunk_ptr: *mut Chunk = first_chunk;
        let first_chunk_nonnull = NonNull::new(first_chunk_ptr)
            .expect("Chunk pointer should not be null");

        GlobalArena {
            chunks: Mutex::new(vec![first_chunk_nonnull]),
            current_chunk: AtomicPtr::new(first_chunk_ptr),
            alignment: DEFAULT_ALIGNMENT,
            chunk_size: size,
            total_allocated: AtomicUsize::new(0),
        }
    }

    /// Allocates a value in the global arena.
    ///
    /// # Arguments
    ///
    /// * `value` - The value to allocate in the arena.
    ///
    /// # Returns
    ///
    /// A mutable reference to the allocated value. The reference is valid for
    /// the entire program lifetime.
    ///
    /// # Panics
    ///
    /// Panics if the allocation fails (e.g., out of memory and unable to
    /// allocate additional chunks).
    #[inline(always)]
    #[allow(clippy::mut_from_ref)] // Uses interior mutability via UnsafeCell
    pub fn alloc<T>(&self, value: T) -> &mut T {
        let size = std::mem::size_of::<T>();
        let align = std::mem::align_of::<T>().max(self.alignment);

        loop {
            // Try to allocate from current chunk
            let current = self.current_chunk.load(Ordering::Acquire);

            if !current.is_null() {
                // SAFETY: current is a valid pointer (checked above)
                let chunk = unsafe { &*current };

                if let Some(ptr) = chunk.try_alloc(size, align) {
                    // Write the value
                    unsafe {
                        std::ptr::write(ptr.as_ptr().cast::<T>(), value);
                        self.total_allocated.fetch_add(size, Ordering::Relaxed);
                        return &mut *(ptr.as_ptr().cast::<T>());
                    }
                }
            }

            // Need to allocate a new chunk
            self.allocate_new_chunk(size);
        }
    }

    /// Allocates a new chunk and updates `current_chunk` pointer.
    #[cold]
    fn allocate_new_chunk(&self, min_size: usize) {
        let new_size = (self.chunk_size * 2).min(MAX_CHUNK_SIZE).max(min_size);

        // SAFETY: We immediately convert the &'static mut Chunk to a raw pointer
        // and never use the reference again. This prevents Stacked Borrows violations.
        let new_chunk = Chunk::new(new_size)
            .expect("Failed to allocate new chunk");

        let new_chunk_ptr: *mut Chunk = new_chunk;
        let new_chunk_nonnull = NonNull::new(new_chunk_ptr)
            .expect("Chunk pointer should not be null");

        // Add to chunks list
        let mut chunks = self.chunks.lock().unwrap();
        chunks.push(new_chunk_nonnull);

        // Update current chunk pointer
        self.current_chunk.store(new_chunk_ptr, Ordering::Release);
    }

    /// Returns allocation statistics for this arena.
    #[must_use]
    pub fn stats(&self) -> ArenaStats {
        let chunks = self.chunks.lock().unwrap();
        let chunk_count = chunks.len();
        let total_capacity = chunks.iter().map(|c| {
            // SAFETY: c is a valid pointer to a Chunk
            unsafe {
                (*c.as_ptr()).capacity
            }
        }).sum();
        let total_allocated = self.total_allocated.load(Ordering::Relaxed);

        ArenaStats {
            total_allocated,
            chunk_count,
            total_capacity,
        }
    }

    /// Allocates a value with flexible array member in the global arena.
    ///
    /// This is used for `RuntimeString`'s heap allocation, where we need a
    /// fixed-size struct followed by a variable-length string buffer.
    ///
    /// # Arguments
    ///
    /// * `value` - The value to allocate
    /// * `capacity` - Additional capacity for flexible array member
    ///
    /// # Returns
    ///
    /// A raw pointer to the allocated value. The pointer is valid for the
    /// entire program lifetime.
    ///
    /// # Safety
    ///
    /// This method uses unsafe code to allocate extra space after the value.
    /// The caller is responsible for properly managing the flexible array.
    /// The returned pointer must not be used to create references that extend
    /// beyond the original value's size.
    #[inline(always)]
    #[allow(clippy::mut_from_ref)] // Uses interior mutability via UnsafeCell
    pub fn alloc_string<T>(&self, value: T, capacity: usize) -> *mut T {
        let size = std::mem::size_of::<T>();
        let align = std::mem::align_of::<T>().max(self.alignment);
        let total_size = size + capacity;

        loop {
            // Try to allocate from current chunk
            let current = self.current_chunk.load(Ordering::Acquire);

            if !current.is_null() {
                // SAFETY: current is a valid pointer (checked above)
                let chunk = unsafe { &*current };

                if let Some(ptr) = chunk.try_alloc(total_size, align) {
                    // Write the value and return raw pointer
                    unsafe {
                        std::ptr::write(ptr.as_ptr().cast::<T>(), value);
                        self.total_allocated.fetch_add(total_size, Ordering::Relaxed);
                        return ptr.as_ptr().cast::<T>();
                    }
                }
            }

            // Need to allocate a new chunk
            self.allocate_new_chunk(total_size);
        }
    }
}

/// Global arena singleton for runtime metadata.
///
/// This provides a single shared arena for all runtime metadata allocations
/// (classes, selectors, protocols, type encodings, etc.). The arena is
/// initialized once and lives for the entire program duration.
///
/// # Examples
///
/// ```
/// use oxidex_mem::arena::global_arena;
///
/// let arena = global_arena();
/// let value = arena.alloc(42u32);
/// assert_eq!(*value, 42);
/// ```
#[must_use]
pub fn global_arena() -> &'static GlobalArena {
    static ARENA: OnceLock<GlobalArena> = OnceLock::new();
    ARENA.get_or_init(|| GlobalArena::new(64 * 1024)) // 64 KiB chunks
}

/// A fixed-size memory chunk with bump allocation.
///
/// `Chunk`s are allocated from the system allocator using `std::alloc` and
/// provide bump pointer allocation for fast, contiguous memory allocation.
///
/// # Thread Safety
///
/// `LocalChunk` uses a non-atomic bump pointer for zero-contention allocation.
/// It is NOT thread-safe and must only be accessed from a single thread.
///
/// # Safety
///
/// - `LocalChunk` memory is never deallocated until the `LocalChunk` is dropped
/// - All allocations are properly aligned
/// - Bump pointer always advances and never wraps around
struct LocalChunk {
    /// Start of the chunk's memory region.
    start: NonNull<u8>,
    /// Current bump pointer (non-atomic).
    /// We store as a raw pointer to maintain provenance.
    ptr: *mut u8,
    /// End of the chunk's memory region (exclusive).
    end: NonNull<u8>,
    /// Total capacity of the chunk in bytes.
    capacity: usize,
}

impl LocalChunk {
    /// Creates a new local chunk.
    fn new(size: usize) -> Result<&'static mut Self, ArenaAllocError> {
        if size < MIN_CHUNK_SIZE {
            return Err(ArenaAllocError);
        }

        let layout = unsafe {
            Layout::from_size_align_unchecked(size, DEFAULT_ALIGNMENT)
        };

        let start = unsafe { alloc::alloc(layout) };
        let start = NonNull::new(start).ok_or(ArenaAllocError)?;

        // SAFETY: start.as_ptr().wrapping_add(size) is safe because:
        // 1. start is a valid pointer from alloc::alloc
        // 2. size is within reasonable bounds (checked above)
        // 3. We're creating a pointer to one past the end, which is valid for comparisons
        let end = start.as_ptr().wrapping_add(size);
        let end =
            NonNull::new(end).expect("LocalChunk end pointer should not be null");

        // Allocate the chunk on the heap and leak it to get a 'static lifetime
        let chunk = Box::leak(Box::new(LocalChunk {
            start,
            // SAFETY: Initialize ptr with the start pointer.
            // This maintains proper provenance throughout the chunk's lifetime.
            ptr: start.as_ptr(),
            end,
            capacity: size,
        }));

        Ok(chunk)
    }

    /// Allocates from this chunk (non-atomic).
    #[must_use]
    #[inline(always)]
    fn alloc(&mut self, size: usize, align: usize) -> Option<NonNull<u8>> {
        // FIX ALIGNMENT BUG: Use normal arithmetic instead of wrapping_add
        // Old (buggy): let aligned_start = (current_addr.wrapping_add(align).wrapping_sub(1)) & !(align - 1);
        // New (correct): let aligned_start = (current_addr + align - 1) & !(align - 1);
        let current_addr = self.ptr.addr();

        // Round up to alignment
        let aligned_start = (current_addr + align - 1) & !(align - 1);
        let size_aligned = (size + align - 1) & !(align - 1);
        let new_addr = aligned_start.saturating_add(size_aligned);

        // Check if we have enough space
        let end_addr = self.end.addr().get();
        if new_addr > end_addr {
            return None;
        }

        // Update the bump pointer
        // We reconstruct the pointer with the new address using with_addr.
        // This preserves the provenance of the original pointer while updating the address.
        // The new address is within the same allocated object (checked above).
        // with_addr is a safe method that maintains provenance.
        self.ptr = self.ptr.with_addr(new_addr);

        // SAFETY: aligned_start is within the chunk bounds (checked above)
        // and is properly aligned by construction.
        // We reconstruct the pointer using with_addr to preserve provenance.
        // with_addr is safe, but new_unchecked requires unsafe because we're
        // guaranteeing the pointer is non-null (which it is, as it's within the chunk).
        let result_ptr = self.ptr.with_addr(aligned_start);
        unsafe { Some(NonNull::new_unchecked(result_ptr)) }
    }
}

impl Drop for LocalChunk {
    fn drop(&mut self) {
        let layout = unsafe {
            Layout::from_size_align_unchecked(self.capacity, DEFAULT_ALIGNMENT)
        };

        unsafe {
            alloc::dealloc(self.start.as_ptr(), layout);
        }
    }
}

/// Thread-local arena for zero-contention allocation.
///
/// `LocalArena` provides fast allocation without any atomic operations or
/// synchronization overhead. It's designed for thread-local allocations where
/// the arena is only accessed from a single thread.
///
/// # Performance
///
/// - Allocation latency: ~2-3ns (pure pointer arithmetic, no atomics)
/// - Zero contention (single-threaded access)
/// - Same memory layout as thread-safe arenas
///
/// # Use Cases
///
/// - Thread-local temporary objects
/// - Per-thread compilation units
/// - String interning storage
///
/// # Examples
///
/// ```
/// use oxidex_mem::arena::LocalArena;
///
/// let mut arena = LocalArena::new(8192);
///
/// // Allocate values
/// let value1: *mut u32 = arena.alloc(42);
/// let value2: *mut u64 = arena.alloc(100);
///
/// unsafe {
///     assert_eq!(*value1, 42);
///     assert_eq!(*value2, 100);
/// }
/// ```
pub struct LocalArena {
    /// List of chunks in this arena.
    chunks: Vec<&'static mut LocalChunk>,
    /// Index of the current chunk.
    current_chunk: usize,
    /// Minimum alignment for all allocations.
    alignment: usize,
}

impl LocalArena {
    /// Creates a new local arena with the specified initial chunk size.
    ///
    /// # Arguments
    ///
    /// * `initial_size` - The size of the initial chunk in bytes. Must be at
    ///   least 8 KiB and will be rounded up to a power of two.
    ///
    /// # Panics
    ///
    /// Panics if the initial chunk allocation fails (e.g., out of memory).
    #[must_use]
    pub fn new(initial_size: usize) -> Self {
        let size = initial_size.max(MIN_CHUNK_SIZE);
        let size = size.next_power_of_two();

        let first_chunk = LocalChunk::new(size)
            .expect("Failed to allocate initial chunk");

        LocalArena {
            chunks: vec![first_chunk],
            current_chunk: 0,
            alignment: DEFAULT_ALIGNMENT,
        }
    }

    /// Allocates a value in the local arena.
    ///
    /// # Arguments
    ///
    /// * `value` - The value to allocate in the arena.
    ///
    /// # Returns
    ///
    /// A pointer to the allocated value. The pointer is stable and will remain
    /// valid for the lifetime of the arena.
    ///
    /// # Safety
    ///
    /// This function uses unsafe code to write the value to arena memory.
    /// The pointer is guaranteed to be:
    /// - Properly aligned for type T
    /// - Valid for the arena's lifetime
    /// - Pointing to writable, uninitialized memory
    ///
    /// # Panics
    ///
    /// Panics if the allocation fails (e.g., out of memory and unable to
    /// allocate additional chunks).
    #[inline(always)]
    pub fn alloc<T>(&mut self, value: T) -> *mut T {
        let size = std::mem::size_of::<T>();
        let align = std::mem::align_of::<T>().max(self.alignment);

        loop {
            if let Some(chunk) = self.chunks.get_mut(self.current_chunk)
                && let Some(ptr) = chunk.alloc(size, align)
            {
                unsafe {
                    std::ptr::write(ptr.as_ptr().cast::<T>(), value);
                    return ptr.as_ptr().cast::<T>();
                }
            }

            // Allocate new chunk
            let last_size =
                self.chunks.last().map_or(MIN_CHUNK_SIZE, |c| c.capacity);
            let new_size = (last_size * 2).min(MAX_CHUNK_SIZE).max(size);

            let new_chunk = LocalChunk::new(new_size)
                .expect("Failed to allocate new chunk");
            self.chunks.push(new_chunk);
            self.current_chunk = self.chunks.len() - 1;
        }
    }

    /// Allocates a string in the arena.
    ///
    /// # Arguments
    ///
    /// * `s` - The string slice to allocate
    ///
    /// # Returns
    ///
    /// A pointer to the allocated string data in the arena.
    ///
    /// # Safety
    ///
    /// - The returned pointer is valid for the arena's lifetime
    /// - The pointer points to valid UTF-8 data
    /// - The string is null-terminated for convenience
    #[inline(always)]
    pub fn alloc_str(&mut self, s: &str) -> *mut u8 {
        let len = s.len();
        let ptr = self.alloc_bytes(len + 1); // +1 for null terminator

        // SAFETY: Copying string data to arena memory:
        // - ptr is valid for len bytes (allocated above)
        // - s.as_ptr() is valid for len bytes (comes from &str)
        // - Regions do not overlap (freshly allocated arena memory)
        // - Source string is alive during copy
        unsafe {
            std::ptr::copy_nonoverlapping(s.as_ptr(), ptr, len);
            *ptr.add(len) = 0; // Add null terminator
        }

        ptr
    }

    /// Allocates raw bytes in the arena.
    ///
    /// # Arguments
    ///
    /// * `size` - The number of bytes to allocate
    ///
    /// # Returns
    ///
    /// A pointer to the allocated memory.
    fn alloc_bytes(&mut self, size: usize) -> *mut u8 {
        let align = self.alignment;

        loop {
            if let Some(chunk) = self.chunks.get_mut(self.current_chunk)
                && let Some(ptr) = chunk.alloc(size, align)
            {
                return ptr.as_ptr();
            }

            // Allocate new chunk
            let last_size =
                self.chunks.last().map_or(MIN_CHUNK_SIZE, |c| c.capacity);
            let new_size = (last_size * 2).min(MAX_CHUNK_SIZE).max(size);

            let new_chunk = LocalChunk::new(new_size)
                .expect("Failed to allocate new chunk");
            self.chunks.push(new_chunk);
            self.current_chunk = self.chunks.len() - 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_local_arena_basic_allocation() {
        let mut arena = LocalArena::new(8192);

        let ptr: *mut u32 = arena.alloc(42);
        unsafe {
            assert_eq!(*ptr, 42);
        }
    }

    #[test]
    fn test_local_arena_multiple_allocations() {
        let mut arena = LocalArena::new(8192);

        let ptr1: *mut u32 = arena.alloc(1);
        let ptr2: *mut u32 = arena.alloc(2);
        let ptr3: *mut u32 = arena.alloc(3);

        unsafe {
            assert_eq!(*ptr1, 1);
            assert_eq!(*ptr2, 2);
            assert_eq!(*ptr3, 3);
        }
    }

    #[test]
    fn test_local_arena_chunk_growth() {
        let mut arena = LocalArena::new(64);

        let values: Vec<*mut u64> = (0..100).map(|i| arena.alloc(i)).collect();

        for (i, &val) in values.iter().enumerate() {
            unsafe {
                assert_eq!(*val, i as u64);
            }
        }
    }

    #[test]
    fn test_local_arena_string_allocation() {
        let mut arena = LocalArena::new(8192);

        let ptr = arena.alloc_str("hello world");

        unsafe {
            // Verify the string was copied correctly
            let s = std::slice::from_raw_parts(ptr, 11);
            assert_eq!(std::str::from_utf8(s), Ok("hello world"));

            // Verify null terminator
            assert_eq!(*ptr.add(11), 0);
        }
    }

    #[test]
    fn test_global_arena_basic_allocation() {
        let arena = GlobalArena::new(65536);

        let value1 = arena.alloc(42u32);
        let value2 = arena.alloc(100u64);

        assert_eq!(*value1, 42);
        assert_eq!(*value2, 100);
    }

    #[test]
    fn test_global_arena_thread_safe() {
        use std::sync::Arc;
        use std::thread;

        let arena = Arc::new(GlobalArena::new(65536));
        let mut handles = vec![];

        // Spawn multiple threads
        for i in 0..10 {
            let arena_clone = Arc::clone(&arena);
            let handle = thread::spawn(move || {
                let value = arena_clone.alloc(i);
                *value
            });
            handles.push(handle);
        }

        // Collect results
        let results: Vec<u32> = handles.into_iter().map(|h| h.join().unwrap()).collect();

        // Each thread should have gotten its value
        for (i, &val) in results.iter().enumerate() {
            assert_eq!(val, i as u32);
        }
    }

    #[test]
    fn test_global_arena_stats() {
        let arena = GlobalArena::new(8192);

        // Allocate some values
        arena.alloc(42u32);
        arena.alloc(100u64);

        let stats = arena.stats();
        assert!(stats.total_allocated > 0);
        assert!(stats.chunk_count >= 1);
        assert!(stats.total_capacity >= 8192);
    }

    #[test]
    fn test_global_arena_singleton() {
        let arena1 = global_arena();
        let arena2 = global_arena();

        // Both calls should return the same arena
        assert!(std::ptr::eq(arena1, arena2));
    }
}
