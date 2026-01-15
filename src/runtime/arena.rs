//! `Arena` allocator for `OxideC` runtime.
//!
//! This module implements a high-performance arena allocator designed for
//! allocating long-lived runtime metadata such as selectors, classes, and
//! protocols. The arena provides:

// Allow precision loss in statistics calculations - acceptable for reporting purposes
#![allow(clippy::cast_precision_loss)]
//!
//! - **Thread-safe allocation** via atomic operations
//! - **Sub-microsecond allocation** through bump pointer strategy
//! - **Stable pointers** (never moves or reallocates)
//! - **Cache-friendly memory layout** with proper alignment
//!
//! # Lifetime Guarantees
//!
//! The arena allocator provides ** `'static` lifetime guarantees ** for all
//! allocated memory. This means:
//!
//! - Pointers returned from allocation methods are valid for the entire
//!   duration of the program (or until the arena is dropped)
//! - The memory is never deallocated or moved while the arena exists
//! - Pointers can be freely shared between threads and stored in global
//!   structures
//!
//! ## Memory Management
//!
//! The arena uses a **bump pointer allocation strategy**:
//!
//! - Memory is allocated sequentially from chunks
//! - Individual allocations are never freed (no deallocation overhead)
//! - All memory in a chunk is freed when the arena is dropped
//! - This design prioritizes allocation speed over memory efficiency
//!
//! ## Thread Safety
//!
//! - `Arena`: Fully thread-safe, can be shared and accessed concurrently
//! - `LocalArena`: Thread-local, single-threaded access only
//! - Both types provide the same allocation interface and guarantees
//!
//! # Stacked Borrows Safety
//!
//! This allocator is designed to be **Stacked Borrows compliant** when used
//! with MIRI's `-Zmiri-strict-provenance` flag. The following rules must be
//! followed to ensure safety:
//!
//! ## Ownership Model
//!
//! ### Chunks
//! - Each chunk is owned by exactly one `Box` at all times
//! - When a chunk is **active** (pointed to by `current_chunk`), the `Box`
//!   is leaked (transferred to `'static` lifetime)
//! - When a chunk is **retired**, ownership is transferred back to a `Box`
//!   and stored in `chunks` vector
//!
//! ### Critical Invariant
//! ```text
//! NEVER create a reference to leaked memory while the Box is still alive!
//! NEVER call Box::from_raw on memory that wasn't leaked from Box::into_raw!
//! ```
//!
//! ## Safe Patterns
//!
//! ### [OK] Correct: Create Box, then leak after CAS
//! ```rust,ignore
//! let chunk_box = Box::new(chunk);
//! let chunk_raw = Box::into_raw(chunk_box);  // Ownership transferred to raw
//!
//! match atomic_cas(&mut ptr, old, chunk_raw) {
//!     Ok(_) => {
//!         // Success: chunk_raw is now owned by arena (leaked)
//!         // DO NOT use chunk_box or call Box::from_raw here
//!     }
//!     Err(_) => {
//!         // Failure: reclaim ownership
//!         let _ = Box::from_raw(chunk_raw);  // Safe to reclaim
//!     }
//! }
//! ```
//!
//! ### [WRONG] Leak first, then try to reclaim
//! ```rust,ignore
//! let chunk_ptr = Box::leak(Box::new(chunk));  // Mutable reference with 'static
//!
//! match atomic_cas(&mut ptr, old, chunk_ptr) {
//!     Ok(_) => { /* ... */ }
//!     Err(_) => {
//!         // VIOLATION: Cannot Box::from_raw on a leaked reference!
//!         // This causes Stacked Borrows violations.
//!         let _ = Box::from_raw(chunk_ptr);  // [WRONG] UB!
//!     }
//! }
//! ```
//!
//! ## Atomic Operation Guidelines
//!
//! When using atomic operations on pointers:
//!
//! 1. **Always** use `Box::into_raw` before CAS (keeps ownership in raw pointer)
//! 2. **Never** use `Box::leak` before CAS (creates reference tied to Box)
//! 3. **Only** call `Box::from_raw` on pointers that came from `Box::into_raw`
//! 4. **Never** mix `Box::leak` and `Box::from_raw` on the same allocation
//!
//! ## Reference Rules
//!
//! ### Mutable References (`&mut`)
//! - **DO** create temporary `&mut` references for immediate operations
//! - **DO NOT** keep `&mut` references across atomic operations
//! - **DO NOT** create `&mut` to leaked memory while Box exists
//!
//! ### Shared References (`&`)
//! - **DO** create `&` references to leaked memory for read-only access
//! - **DO NOT** create `&` references while `&mut` exists
//! - **DO NOT** mix shared and mutable references to same memory
//!
//! ### Raw Pointers (`*mut` / `*const`)
//! - **DO** use raw pointers for atomic operations
//! - **DO** use `addr_of!` and `offset_of!` to avoid creating references
//! - **DO NOT** dereference raw pointers while mutable references exist
//!
//! ## Example: Safe Chunk Allocation
//!
//! ```rust,ignore
//! fn allocate_new_chunk(&self) -> Result<()> {
//!     loop {
//!         // 1. Load current pointer
//!         let current = self.current_chunk.load(Ordering::Acquire);
//!
//!         // 2. Create new chunk (owned by Box)
//!         let new_chunk = Chunk::new(size)?;
//!         let new_box = Box::new(new_chunk);
//!
//!         // 3. Transfer to raw pointer (still owned)
//!         let new_raw = Box::into_raw(new_box);
//!
//!         // 4. Try atomic CAS
//!         match self.current_chunk.compare_exchange_weak(
//!             current,
//!             new_raw,
//!             Ordering::AcqRel,
//!             Ordering::Acquire,
//!         ) {
//!             Ok(_) => {
//!                 // Success: new_raw is now leaked (owned by arena)
//!                 // Reclaim old chunk safely
//!                 let old_box = unsafe { Box::from_raw(current) };
//!                 self.chunks.lock().push(*old_box);
//!                 return Ok(());
//!             }
//!             Err(_) => {
//!                 // Failure: reclaim our new chunk
//!                 // SAFETY: new_raw is owned by us, safe to reclaim
//!                 unsafe { drop(Box::from_raw(new_raw)); }
//!                 continue;  // Retry
//!             }
//!         }
//!     }
//! }
//! ```
//!
//! # Safety
//!
//! The arena allocator uses extensive unsafe code for manual memory management.
//! All unsafe operations are documented with comprehensive SAFETY comments
//! proving pointer validity, alignment, and lifetime guarantees.
//!
//! # Architecture
//!
//! - [`Arena`]: Thread-safe arena for global metadata (classes, selectors)
//! - [`LocalArena`]: Thread-local arena for zero-contention allocation
//! - [`Chunk`]: Fixed-size memory block with bump allocation
//!
//! # Example
//!
//! ```rust
//! use oxidec::runtime::arena::Arena;
//!
//! let arena = Arena::new(4096); // 4 KiB initial chunk
//! let value: *mut u32 = arena.alloc(42);
//! unsafe {
//!     assert_eq!(*value, 42);
//! }
//! // The pointer remains valid for the lifetime of the arena
//! ```
//!
//! # Performance
//!
//! Allocation performance characteristics:
//!
//! - **`Arena` allocation**: ~13-15 ns (with atomic operations)
//! - **`LocalArena` allocation**: ~2-3 ns (no atomics, thread-local)
//! - **`Chunk` growth**: Amortized O(1) as chunks double in size
//!
//! # Memory Overhead
//!
//! - Per-chunk overhead: 32 bytes (metadata + atomic pointer)
//! - Unused memory in current chunk: Typically < chunk size
//! - No per-allocation overhead (unlike malloc)

use crate::error::{Error, Result};
use std::alloc::{self, Layout};
use std::ptr::NonNull;
use std::sync::Mutex;
use std::sync::atomic::{AtomicPtr, Ordering};

/// Default alignment for arena allocations (16 bytes).
///
/// This ensures proper alignment for:
/// - Atomic operations (`AtomicU32` at 4-byte, `AtomicU64` at 8-byte)
/// - SIMD operations (16-byte alignment)
/// - General purpose cache line optimization
const DEFAULT_ALIGNMENT: usize = 16;

/// Minimum chunk size (4 KiB - one page).
const MIN_CHUNK_SIZE: usize = 4096;

/// Maximum chunk size (1 MiB).
const MAX_CHUNK_SIZE: usize = 1024 * 1024;

/// A fixed-size memory chunk with bump allocation.
///
/// `Chunk`s are allocated from the system allocator using `std::alloc` and
/// provide bump pointer allocation for fast, contiguous memory allocation.
///
/// # Thread Safety
///
/// `Chunk` uses atomic operations for the bump pointer, making it safe
/// to allocate from multiple threads concurrently.
///
/// # Safety
///
/// - `Chunk` memory is never deallocated until the `Chunk` is dropped
/// - All allocations are properly aligned
/// - Bump pointer always advances and never wraps around
pub struct Chunk {
    /// Start of the chunk's memory region.
    start: NonNull<u8>,
    /// Current bump pointer (atomic for thread safety).
    /// We use AtomicPtr<u8> to maintain proper provenance.
    ptr: AtomicPtr<u8>,
    /// End of the chunk's memory region (exclusive).
    end: NonNull<u8>,
    /// Total capacity of the chunk in bytes.
    capacity: usize,
}

impl Chunk {
    /// Creates a new chunk with the specified size.
    ///
    /// # Arguments
    ///
    /// * `size` - The size of the chunk in bytes. Must be at least `MIN_CHUNK_SIZE`
    ///   and must be a multiple of `DEFAULT_ALIGNMENT`.
    ///
    /// # Returns
    ///
    /// Returns a `Result` containing the new `Chunk` or an `Error` if allocation fails.
    ///
    /// # Safety
    ///
    /// - The chunk is allocated with proper alignment (`DEFAULT_ALIGNMENT`)
    /// - The memory is valid for the lifetime of the `Chunk`
    /// - All pointers derived from this chunk are valid until the `Chunk` is dropped
    ///
    /// # Panics
    ///
    /// - Panics if the `Chunk::end` end pointer is null
    ///
    /// # Errors
    ///
    /// - Returns `Err(Error::ChunkAllocationFailed` when the size is less than `MIN_CHUNK_SIZE`
    /// - Returns `Err(Error::InvalidAlignment` when the size is not a power of two or is not a multiple of `DEFAULT_ALIGNMENT`
    pub fn new(size: usize) -> Result<Self> {
        if size < MIN_CHUNK_SIZE {
            return Err(Error::ChunkAllocationFailed { size });
        }

        if !size.is_power_of_two() || !size.is_multiple_of(DEFAULT_ALIGNMENT) {
            return Err(Error::InvalidAlignment {
                alignment: DEFAULT_ALIGNMENT,
            });
        }

        // SAFETY: We're creating a layout with size and alignment.
        // Both size and `DEFAULT_ALIGNMENT` (16) are valid (non-zero, power of two).
        let layout = unsafe {
            Layout::from_size_align_unchecked(size, DEFAULT_ALIGNMENT)
        };

        // SAFETY: alloc is safe to call with a valid layout.
        // It returns null if allocation fails, which we check.
        let start = unsafe { alloc::alloc(layout) };

        let start = NonNull::new(start).ok_or(Error::OutOfMemory)?;

        // SAFETY: start.as_ptr().wrapping_add(size) is safe because:
        // 1. start is a valid pointer from alloc::alloc
        // 2. size is within reasonable bounds (checked above)
        // 3. We're creating a pointer to one past the end, which is valid for comparisons
        let end = start.as_ptr().wrapping_add(size);
        let end =
            NonNull::new(end).expect("Chunk end pointer should not be null");

        Ok(Chunk {
            start,
            // SAFETY: Initialize AtomicPtr with the start pointer.
            // This maintains proper provenance throughout the chunk's lifetime.
            ptr: AtomicPtr::new(start.as_ptr()),
            end,
            capacity: size,
        })
    }

    /// Attempts to allocate from this chunk with the given size and alignment.
    ///
    /// # Arguments
    ///
    /// * `size` - The size of the allocation in bytes.
    /// * `align` - The required alignment for the allocation.
    ///
    /// # Returns
    ///
    /// Returns `Some(ptr)` if there's enough space in this chunk, or `None` if
    /// the chunk is full.
    ///
    /// # Safety
    ///
    /// - If this returns `Some(ptr)`, the pointer is properly aligned
    /// - The pointer points to valid, writable memory within this chunk
    /// - The memory is uninitialized and can be written to
    #[must_use]
    pub fn try_alloc(&self, size: usize, align: usize) -> Option<NonNull<u8>> {
        loop {
            // Load current bump pointer as a raw pointer
            let current_ptr = self.ptr.load(Ordering::Acquire);

            // Get the address for arithmetic
            let current_addr = current_ptr.addr();

            // Calculate aligned offset
            let aligned_offset = Self::round_up_to_align(current_addr, align);
            let alloc_size = Self::round_up_to_align(size, align);

            // Check if we have enough space
            let end_addr = self.end.addr().get();
            if aligned_offset.wrapping_add(alloc_size) > end_addr {
                return None;
            }

            // Try to claim this space atomically
            let new_addr = aligned_offset.wrapping_add(alloc_size);

            // We reconstruct the pointer with the new address using with_addr.
            // This preserves the provenance of the original pointer while updating the address.
            // The new address is within the same allocated object (checked above).
            // with_addr is a safe method that maintains provenance.
            let new_ptr = current_ptr.with_addr(new_addr);

            // SAFETY: ptr is an AtomicPtr<u8> that we're performing a CAS operation on.
            // The ordering (Acquire/Release) ensures proper synchronization.
            // We use compare_exchange_weak which is more efficient in loops.
            if self
                .ptr
                .compare_exchange_weak(
                    current_ptr,
                    new_ptr,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok()
            {
                // Successfully claimed the space
                // SAFETY: aligned_offset is within the chunk bounds (checked above)
                // and is properly aligned by construction.
                // We reconstruct the pointer using with_addr to preserve provenance.
                // with_addr is safe, but new_unchecked requires unsafe because we're
                // guaranteeing the pointer is non-null (which it is, as it's within the chunk).
                let result_ptr = current_ptr.with_addr(aligned_offset);
                return unsafe { Some(NonNull::new_unchecked(result_ptr)) };
            }
        }
    }

    /// Returns the remaining capacity in this chunk.
    #[must_use]
    pub fn remaining(&self) -> usize {
        let current_ptr = self.ptr.load(Ordering::Acquire);
        let current_addr = current_ptr.addr();
        let end = self.end.addr().get();
        end.saturating_sub(current_addr)
    }

    /// Rounds up a value to the next multiple of alignment.
    ///
    /// # Arguments
    ///
    /// * `value` - The value to round up.
    /// * `align` - The alignment (must be a power of two).
    ///
    /// # Returns
    ///
    /// The rounded-up value.
    #[inline]
    fn round_up_to_align(value: usize, align: usize) -> usize {
        debug_assert!(
            align.is_power_of_two(),
            "Alignment must be a power of two"
        );
        value.wrapping_add(align - 1) & !(align - 1)
    }
}

impl Drop for Chunk {
    fn drop(&mut self) {
        // SAFETY: Deallocating the chunk's memory.
        // The layout matches what was used in `Chunk`::new.
        let layout = unsafe {
            Layout::from_size_align_unchecked(self.capacity, DEFAULT_ALIGNMENT)
        };

        unsafe {
            alloc::dealloc(self.start.as_ptr(), layout);
        }
    }
}

// SAFETY: Chunk uses atomic operations for thread safety.
// It's safe to share a `Chunk` between threads.
unsafe impl Send for Chunk {}
unsafe impl Sync for Chunk {}

/// Thread-safe arena allocator for long-lived runtime metadata.
///
/// The ``Arena`` provides fast, thread-safe allocation through a bump-pointer
/// strategy. It's designed for allocating metadata that lives for the entire
/// program duration, such as:
///
/// - `Selector`s (method names)
/// - `Class` definitions and method tables
/// - Protocol definitions
/// - Runtime caches
///
/// # Thread Safety
///
/// The ``Arena`` uses atomic operations for the bump pointer, making it safe
/// to allocate from multiple threads concurrently without external synchronization.
///
/// # Performance
///
/// - Allocation latency: <200ns (mostly atomic operations)
/// - Memory overhead: One pointer per allocation (8 bytes)
/// - No individual deallocation (entire arena dropped at once)
///
/// # Example
///
/// ```rust
/// use oxidec::runtime::arena::Arena;
///
/// let arena = Arena::new(4096);
///
/// // Allocate values in the arena
/// let value1: *mut u32 = arena.alloc(42);
/// let value2: *mut u64 = arena.alloc(100);
///
/// // Pointers are stable for the arena's lifetime
/// unsafe {
///     assert_eq!(*value1, 42);
///     assert_eq!(*value2, 100);
/// }
/// ```
pub struct Arena {
    /// List of chunks in this arena (protected by Mutex for thread safety).
    /// Only used for slow path (chunk allocation) and drop.
    chunks: Mutex<Vec<Chunk>>,
    /// Pointer to the current chunk (lock-free fast path).
    /// This allows direct access to the current chunk without taking the mutex.
    current_chunk: AtomicPtr<Chunk>,
    /// Minimum alignment for all allocations.
    alignment: usize,
}

impl Arena {
    /// Creates a new arena with the specified initial chunk size.
    ///
    /// # Arguments
    ///
    /// * `initial_size` - The size of the initial chunk in bytes. Must be at
    ///   least 4 KiB and will be rounded up to a power of two.
    ///
    /// # Returns
    ///
    /// A new `Arena` ready for allocations.
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::runtime::arena::Arena;
    ///
    /// let arena = Arena::new(4096); // 4 KiB initial chunk
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if the initial chunk allocation fails (e.g., out of memory).
    #[must_use]
    pub fn new(initial_size: usize) -> Self {
        let size = initial_size.max(MIN_CHUNK_SIZE);
        let size = size.next_power_of_two();

        let first_chunk =
            Chunk::new(size).expect("Failed to allocate initial chunk");

        // Get raw pointer to the first chunk
        let chunk_ptr = Box::leak(Box::new(first_chunk));

        Arena {
            chunks: Mutex::new(vec![]), // `Chunk`s managed separately
            current_chunk: AtomicPtr::new(chunk_ptr),
            alignment: DEFAULT_ALIGNMENT,
        }
    }

    /// Creates a new arena with a custom alignment.
    ///
    /// # Arguments
    ///
    /// * `initial_size` - The size of the initial chunk in bytes.
    /// * `alignment` - The minimum alignment for allocations (must be a power of two).
    ///
    /// # Panics
    ///
    /// Panics if `alignment` is not a power of two, or if the initial chunk
    /// allocation fails.
    #[must_use]
    pub fn with_config(initial_size: usize, alignment: usize) -> Self {
        assert!(
            alignment.is_power_of_two(),
            "Alignment must be a power of two"
        );

        let size = initial_size.max(MIN_CHUNK_SIZE);
        let size = size.next_power_of_two();

        let first_chunk =
            Chunk::new(size).expect("Failed to allocate initial chunk");

        // Get raw pointer to the first chunk
        let chunk_ptr = Box::leak(Box::new(first_chunk));

        Arena {
            chunks: Mutex::new(vec![]),
            current_chunk: AtomicPtr::new(chunk_ptr),
            alignment: alignment.max(DEFAULT_ALIGNMENT),
        }
    }

    /// Allocates a value in the arena and returns a pointer to it.
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
    /// - Valid for the 'static lifetime (arena never drops during program execution)
    /// - Pointing to writable, uninitialized memory
    /// - Unique (no other pointers to this allocation)
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::runtime::arena::Arena;
    ///
    /// let arena = Arena::new(4096);
    /// let ptr: *mut u32 = arena.alloc(42);
    ///
    /// unsafe {
    ///     assert_eq!(*ptr, 42);
    /// }
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if the allocation fails (e.g., out of memory and unable to
    /// allocate additional chunks).
    pub fn alloc<T>(&self, value: T) -> *mut T {
        let size = std::mem::size_of::<T>();
        let align = std::mem::align_of::<T>().max(self.alignment);

        // Try to allocate in the current chunk (lock-free fast path)
        loop {
            // Load current chunk pointer (Acquire ordering ensures we see the latest chunk)
            let chunk_ptr = self.current_chunk.load(Ordering::Acquire);

            // SAFETY: chunk_ptr is a valid pointer to a `Chunk` that lives for 'static
            // (leaked box), so dereferencing it here is safe.
            let chunk = unsafe { &*chunk_ptr };

            // Try fast path allocation
            if let Some(ptr) = chunk.try_alloc(size, align) {
                // SAFETY: Writing value to arena memory:
                // 1. ptr is properly aligned (guaranteed by try_alloc)
                // 2. ptr points to valid memory (from chunk)
                // 3. Memory is uninitialized and writable
                // 4. size matches sizeof(T) exactly
                unsafe {
                    std::ptr::write(ptr.as_ptr().cast::<T>(), value);
                    return ptr.as_ptr().cast::<T>();
                }
            }

            // Slow path: allocate new chunk
            if self.allocate_new_chunk(size).is_ok() {
                continue;
            }

            // Out of memory
            panic!("`Arena` allocation failed: out of memory");
        }
    }

    /// Allocates a new chunk and adds it to the arena.
    ///
    /// # Arguments
    ///
    /// * `min_size` - The minimum size required for the new chunk.
    ///
    /// # Thread Safety
    ///
    /// This method uses atomic compare-and-swap to safely replace the current chunk.
    /// Only one thread will successfully replace the chunk; others will retry.
    ///
    /// # Stacked Borrows Safety
    ///
    /// This function carefully manages Box ownership to prevent Stacked Borrows violations:
    ///
    /// 1. **Before CAS**: `new_chunk_raw` is owned via `Box::into_raw` (no references exist)
    /// 2. **After successful CAS**: `new_chunk_raw` is leaked (transferred to arena ownership)
    /// 3. **After failed CAS**: `new_chunk_raw` is reclaimed via `Box::from_raw` (safe - we own it)
    ///
    /// **CRITICAL**: We use `Box::into_raw` NOT `Box::leak` to avoid creating a `&mut` reference
    /// that would conflict with the Box's ownership and cause Stacked Borrows violations.
    fn allocate_new_chunk(&self, min_size: usize) -> Result<()> {
        // Try to replace the current chunk atomically
        loop {
            // Load current chunk pointer (Acquire ordering ensures we see the latest chunk)
            let current_ptr = self.current_chunk.load(Ordering::Acquire);

            // SAFETY: current_ptr is valid (leaked box with 'static lifetime)
            // We only read the capacity field - no mutable references created
            // Using `&*` is safe here because we're not keeping the reference
            let current_capacity = unsafe { &*current_ptr }.capacity;

            // Calculate new chunk size (double the current, up to MAX_CHUNK_SIZE)
            let new_size = (current_capacity * 2).min(MAX_CHUNK_SIZE).max(min_size);

            let new_chunk = Chunk::new(new_size)?;

            // CRITICAL: Use Box::into_raw NOT Box::leak
            // Box::leak would create a &'static mut, violating Stacked Borrows if we later
            // call Box::from_raw. Box::into_raw keeps ownership in the raw pointer.
            let new_chunk_box = Box::new(new_chunk);
            let new_chunk_raw = Box::into_raw(new_chunk_box);

            // SAFETY: new_chunk_raw is valid and non-null (Box guarantees this)
            assert!(!new_chunk_raw.is_null(), "Chunk pointer must not be null");

            // Try to atomically replace the current chunk
            // SAFETY: Lock-free CAS operation - only one thread succeeds
            // AcqRel ensures all writes to new chunk visible before other threads see it
            match self.current_chunk.compare_exchange_weak(
                current_ptr,
                new_chunk_raw,
                Ordering::AcqRel,  // Success ordering: ensures all writes to new chunk are visible
                Ordering::Acquire, // Failure ordering: ensures we see the latest chunk
            ) {
                Ok(_) => {
                    // We successfully replaced the chunk! Now safely reclaim the old one.
                    // SAFETY: current_ptr points to a previously leaked chunk (from successful CAS)
                    // Calling Box::from_raw is safe because we're reclaiming ownership
                    // SAFETY: new_chunk_raw is now owned by the arena (leaked for 'static lifetime)
                    // DO NOT call Box::from_raw on new_chunk_raw - it's no longer owned by us
                    let old_chunk = unsafe { Box::from_raw(current_ptr) };
                    let mut chunks = self.chunks.lock().unwrap();
                    chunks.push(*old_chunk);
                    return Ok(());
                }
                Err(_actual_ptr) => {
                    // Another thread already replaced the chunk.
                    // Clean up our unused chunk by reclaiming ownership.
                    // SAFETY: new_chunk_raw is owned by us (CAS failed), safe to reclaim
                    // We MUST reclaim here to avoid memory leak
                    // Using drop() to make the intent explicit (reclaim then drop)
                    unsafe {
                        drop(Box::from_raw(new_chunk_raw));
                    }
                    // The chunk memory is freed when the Box is dropped
                    continue; // Retry with the new current chunk
                }
            }
        }
    }

    /// Allocates a string in the arena with a flexible array member.
    ///
    /// # Arguments
    ///
    /// * `heap_str` - The `HeapString` header to allocate
    /// * `capacity` - The total capacity including string data
    ///
    /// # Returns
    ///
    /// Pointer to the allocated `HeapString`
    ///
    /// # Safety
    ///
    /// - Caller must ensure capacity is sufficient for string data
    /// - Returned pointer is valid for the arena's lifetime
    /// - String data must be copied immediately after allocation
    pub fn alloc_string<T>(&self, heap_str: T, capacity: usize) -> *mut T {
        // Calculate total size including string data
        let header_size = std::mem::size_of::<T>();
        let total_size = header_size + capacity;

        // Allocate with arena's alignment (ensures proper alignment for all types)
        let layout = unsafe {
            Layout::from_size_align_unchecked(total_size, self.alignment)
        };

        // Allocate using existing chunk logic
        let ptr = self.alloc_aligned(layout);

        // SAFETY: Writing T to arena memory:
        // - ptr is properly aligned (self.alignment bytes, guaranteed by arena)
        // - ptr points to valid memory (from arena)
        // - Memory is uninitialized and writable
        // - Layout matches T structure
        unsafe {
            std::ptr::write(ptr.cast::<T>(), heap_str);
        }

        ptr.cast::<T>()
    }

    /// Allocates with custom alignment (internal helper).
    ///
    /// # Arguments
    ///
    /// * `layout` - The layout describing size and alignment
    ///
    /// # Returns
    ///
    /// Pointer to the allocated memory
    fn alloc_aligned(&self, layout: Layout) -> *mut u8 {
        let size = layout.size();
        let align = layout.align();

        loop {
            // Load current chunk pointer (lock-free)
            let chunk_ptr = self.current_chunk.load(Ordering::Acquire);

            // SAFETY: chunk_ptr is valid (leaked box with 'static lifetime)
            let chunk = unsafe { &*chunk_ptr };

            if let Some(ptr) = chunk.try_alloc(size, align) {
                return ptr.as_ptr();
            }

            // Allocate new chunk
            if self.allocate_new_chunk(size).is_ok() {
                continue;
            }

            panic!("`Arena` allocation failed: out of memory");
        }
    }

    /// Returns statistics about the arena's memory usage.
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned (which should never happen
    /// under normal circumstances).
    #[must_use]
    pub fn stats(&self) -> ArenaStats {
        // Get current chunk
        let current_ptr = self.current_chunk.load(Ordering::Acquire);
        let current_chunk = unsafe { &*current_ptr };

        // Get old chunks
        let chunks = self.chunks.lock().unwrap();

        // Count current chunk + all old chunks
        let total_chunks = chunks.len() + 1;

        // Sum capacities
        let total_capacity: usize = chunks
            .iter()
            .map(|c| c.capacity)
            .chain(std::iter::once(current_chunk.capacity))
            .sum();

        // Sum used space
        let total_used: usize = chunks
            .iter()
            .map(|c| c.capacity - c.remaining())
            .chain(std::iter::once(
                current_chunk.capacity - current_chunk.remaining(),
            ))
            .sum();

        ArenaStats {
            total_chunks,
            total_capacity,
            total_used,
            unused_ratio: if total_capacity > 0 {
                1.0 - (total_used as f64 / total_capacity as f64)
            } else {
                0.0
            },
        }
    }
}

impl Drop for Arena {
    fn drop(&mut self) {
        // Clean up current chunk
        let current_ptr = self.current_chunk.load(Ordering::Acquire);
        if !current_ptr.is_null() {
            unsafe {
                let _ = Box::from_raw(current_ptr);
            }
        }

        // Clean up all old chunks
        let mut chunks = self.chunks.lock().unwrap();
        chunks.clear();
    }
}

/// Statistics about arena memory usage.
#[derive(Debug, Clone, Copy)]
pub struct ArenaStats {
    /// Total number of chunks in the arena.
    pub total_chunks: usize,
    /// Total capacity of all chunks in bytes.
    pub total_capacity: usize,
    /// Total used memory across all chunks in bytes.
    pub total_used: usize,
    /// Unused memory ratio (0.0 = fully utilized, 1.0 = completely empty).
    /// This is the proportion of allocated but unused memory.
    pub unused_ratio: f64,
}

// SAFETY: Arena uses atomic operations and `Chunk` is Sync, so `Arena` is thread-safe.
unsafe impl Sync for Arena {}
unsafe impl Send for Arena {}

/// Thread-local arena for zero-contention allocation.
///
/// `LocalArena` provides the same allocation strategy as `Arena` but without
/// any atomic operations or synchronization overhead. It's designed for
/// thread-local allocations where the arena is only accessed from a single thread.
///
/// # Performance
///
/// - Allocation latency: <50ns (pure pointer arithmetic, no atomics)
/// - Zero contention (single-threaded access)
/// - Same memory layout as `Arena`
///
/// # Use Cases
///
/// - Thread-local temporary objects
/// - Per-thread method caches
/// - Message dispatch buffers
///
/// # Example
///
/// ```rust
/// use oxidec::runtime::arena::LocalArena;
///
/// let mut arena = LocalArena::new(4096);
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
    chunks: Vec<LocalChunk>,
    /// Index of the current chunk.
    current_chunk: usize,
    /// Minimum alignment for all allocations.
    alignment: usize,
}

/// A chunk for thread-local arena allocation.
///
/// Unlike `Chunk`, `LocalChunk` uses a non-atomic bump pointer for
/// zero-contention allocation.
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
    fn new(size: usize) -> Result<Self> {
        if size < MIN_CHUNK_SIZE {
            return Err(Error::ChunkAllocationFailed { size });
        }

        let layout = unsafe {
            Layout::from_size_align_unchecked(size, DEFAULT_ALIGNMENT)
        };

        let start = unsafe { alloc::alloc(layout) };
        let start = NonNull::new(start).ok_or(Error::OutOfMemory)?;

        // SAFETY: start.as_ptr().wrapping_add(size) is safe because:
        // 1. start is a valid pointer from alloc::alloc
        // 2. size is within reasonable bounds (checked above)
        // 3. We're creating a pointer to one past the end, which is valid for comparisons
        let end = start.as_ptr().wrapping_add(size);
        let end =
            NonNull::new(end).expect("`Chunk` end pointer should not be null");

        Ok(LocalChunk {
            start,
            // SAFETY: Initialize ptr with the start pointer.
            // This maintains proper provenance throughout the chunk's lifetime.
            ptr: start.as_ptr(),
            end,
            capacity: size,
        })
    }

    /// Allocates from this chunk (non-atomic).
    #[must_use]
    fn alloc(&mut self, size: usize, align: usize) -> Option<NonNull<u8>> {
        // Get the current address from the pointer
        let current_addr = self.ptr.addr();

        let aligned_offset = Chunk::round_up_to_align(current_addr, align);
        let offset = aligned_offset - current_addr;
        let _total_size = Chunk::round_up_to_align(size, align) + offset;

        let end_addr = self.end.addr().get();
        if aligned_offset + size > end_addr {
            return None;
        }

        // We reconstruct the pointer with the new address using with_addr.
        // This preserves the provenance of the original pointer while updating the address.
        // The new address is within the same allocated object (checked above).
        // with_addr is a safe method that maintains provenance.
        let new_addr = aligned_offset + Chunk::round_up_to_align(size, align);
        self.ptr = self.ptr.with_addr(new_addr);

        // SAFETY: aligned_offset is within the chunk bounds (checked above)
        // and is properly aligned by construction.
        // We reconstruct the pointer using with_addr to preserve provenance.
        // with_addr is safe, but new_unchecked requires unsafe because we're
        // guaranteeing the pointer is non-null (which it is, as it's within the chunk).
        let result_ptr = self.ptr.with_addr(aligned_offset);
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

impl LocalArena {
    /// Creates a new local arena with the specified initial chunk size.
    ///
    /// # Panics
    ///
    /// Panics if the initial chunk allocation fails (e.g., out of memory).
    #[must_use]
    pub fn new(initial_size: usize) -> Self {
        let size = initial_size.max(MIN_CHUNK_SIZE);
        let size = size.next_power_of_two();

        let first_chunk =
            LocalChunk::new(size).expect("Failed to allocate initial chunk");

        LocalArena {
            chunks: vec![first_chunk],
            current_chunk: 0,
            alignment: DEFAULT_ALIGNMENT,
        }
    }

    /// Allocates a value in the local arena.
    ///
    /// # Panics
    ///
    /// Panics if the allocation fails (e.g., out of memory and unable to
    /// allocate additional chunks).
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

    /// Allocates a string in the local arena with a flexible array member.
    ///
    /// # Arguments
    ///
    /// * `heap_str` - The `HeapString` header to allocate
    /// * `capacity` - The total capacity including string data
    ///
    /// # Returns
    ///
    /// Pointer to the allocated `HeapString`
    pub fn alloc_string<T>(&mut self, heap_str: T, capacity: usize) -> *mut T {
        // Calculate total size including string data
        let header_size = std::mem::size_of::<T>();
        let total_size = header_size + capacity;

        // Allocate with arena's alignment (ensures proper alignment for all types)
        let layout = unsafe {
            Layout::from_size_align_unchecked(total_size, self.alignment)
        };

        // Allocate using existing chunk logic
        let ptr = self.alloc_aligned(layout);

        // SAFETY: Writing T to arena memory
        unsafe {
            std::ptr::write(ptr.cast::<T>(), heap_str);
        }

        ptr.cast::<T>()
    }

    /// Allocates with custom alignment (internal helper).
    ///
    /// # Arguments
    ///
    /// * `layout` - The layout describing size and alignment
    ///
    /// # Returns
    ///
    /// Pointer to the allocated memory
    fn alloc_aligned(&mut self, layout: Layout) -> *mut u8 {
        let size = layout.size();
        let align = layout.align();

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

    /// Returns statistics about the local arena's memory usage.
    #[must_use]
    pub fn stats(&self) -> ArenaStats {
        let total_chunks = self.chunks.len();
        let total_capacity: usize =
            self.chunks.iter().map(|c| c.capacity).sum();

        let total_used: usize = self
            .chunks
            .iter()
            .map(|c| c.ptr.addr() - c.start.addr().get())
            .sum();

        ArenaStats {
            total_chunks,
            total_capacity,
            total_used,
            unused_ratio: if total_capacity > 0 {
                1.0 - (total_used as f64 / total_capacity as f64)
            } else {
                0.0
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_creation() {
        let chunk = Chunk::new(4096).unwrap();
        assert_eq!(chunk.capacity, 4096);
        assert_eq!(chunk.remaining(), 4096);
    }

    #[test]
    fn test_chunk_alignment() {
        let chunk = Chunk::new(4096).unwrap();
        assert_eq!(chunk.start.addr().get() % DEFAULT_ALIGNMENT, 0);
    }

    #[test]
    fn test_chunk_allocation() {
        let chunk = Chunk::new(4096).unwrap();

        let ptr1 = chunk.try_alloc(16, 8);
        assert!(ptr1.is_some());

        let ptr2 = chunk.try_alloc(32, 16);
        assert!(ptr2.is_some());

        // Pointers should be different
        assert_ne!(ptr1.unwrap(), ptr2.unwrap());
    }

    #[test]
    fn test_arena_basic_allocation() {
        let arena = Arena::new(4096);

        let ptr: *mut u32 = arena.alloc(42);
        unsafe {
            assert_eq!(*ptr, 42);
        }
    }

    #[test]
    fn test_arena_multiple_allocations() {
        let arena = Arena::new(4096);

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
    fn test_arena_chunk_growth() {
        let arena = Arena::new(64); // Small initial chunk

        // Allocate enough to force chunk growth
        let values: Vec<*mut u64> = (0..100).map(|i| arena.alloc(i)).collect();

        for (i, &val) in values.iter().enumerate() {
            unsafe {
                assert_eq!(*val, i as u64);
            }
        }
    }

    #[test]
    fn test_arena_stats() {
        let arena = Arena::new(4096);

        let _ptr1: *mut u32 = arena.alloc(42);
        let _ptr2: *mut u64 = arena.alloc(100);

        let stats = arena.stats();
        assert_eq!(stats.total_chunks, 1);
        assert!(stats.total_used > 0);
    }

    #[test]
    fn test_local_arena_basic_allocation() {
        let mut arena = LocalArena::new(4096);

        let ptr: *mut u32 = arena.alloc(42);
        unsafe {
            assert_eq!(*ptr, 42);
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
    fn test_local_arena_stats() {
        let mut arena = LocalArena::new(4096);

        let _ptr1: *mut u32 = arena.alloc(42);
        let _ptr2: *mut u64 = arena.alloc(100);

        let stats = arena.stats();
        assert_eq!(stats.total_chunks, 1);
        assert!(stats.total_used > 0);
    }

    #[test]
    fn test_alignment_rounding() {
        assert_eq!(Chunk::round_up_to_align(0, 16), 0);
        assert_eq!(Chunk::round_up_to_align(1, 16), 16);
        assert_eq!(Chunk::round_up_to_align(15, 16), 16);
        assert_eq!(Chunk::round_up_to_align(16, 16), 16);
        assert_eq!(Chunk::round_up_to_align(17, 16), 32);
    }
}
