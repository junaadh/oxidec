//! Factory for creating thread-local arenas.
//!
//! `ArenaFactory` provides a cheap way to create arena instances without
//! pooling or reuse. It simply creates new arenas on demand with a configured
//! chunk size.
//!
//! # Design
//!
//! - **No pooling**: Arenas are created fresh each time
//! - **No reuse**: Arenas are dropped when done (automatic cleanup)
//! - **Cheap creation**: Just a struct with a size field
//!
//! # Use Cases
//!
//! - Thread-local arena creation in runtime code
//! - Per-request arenas in server contexts
//! - Phase-scoped arenas in compiler frontend
//!
//! # Examples
//!
//! ```
//! use oxidex_mem::factory::ArenaFactory;
//! use oxidex_mem::arena::LocalArena;
//!
//! // Create a factory
//! let factory = ArenaFactory::new(64 * 1024); // 64 KiB chunks
//!
//! // Create an arena from the factory
//! let mut arena = factory.create_arena();
//!
//! // Use the arena
//! let value: *mut u32 = arena.alloc(42);
//! unsafe {
//!     assert_eq!(*value, 42);
//! }
//!
//! // Arena is dropped here, memory is reclaimed
//! ```
//!
//! # Thread-local Usage
//!
//! ```ignore
//! use oxidex_mem::factory::ArenaFactory;
//! use oxidex_mem::arena::LocalArena;
//! use std::thread_local;
//!
//! thread_local! {
//!     static TEMP_ARENA: ArenaFactory = ArenaFactory::new(64 * 1024);
//! }
//!
//! fn with_temp_arena<F, R>(f: F) -> R
//! where
//!     F: FnOnce(&LocalArena) -> R,
//! {
//!     TEMP_ARENA.with(|factory| {
//!         let mut arena = factory.create_arena();
//!         f(&arena)
//!         // arena dropped here, automatic cleanup
//!     })
//! }
//! ```

use crate::arena::LocalArena;

/// Factory for creating thread-local arenas.
///
/// `ArenaFactory` is a simple, cheap way to create `LocalArena` instances
/// with a configured chunk size. It does NOT pool or reuse arenas - each
/// call to `create_arena()` creates a fresh arena.
///
/// # Rationale
///
/// Pooling and reuse were intentionally excluded because:
/// - Reuse complicates lifetime semantics
/// - Pooling defeats the benefits of clear phase boundaries
/// - Cheap creation makes pooling unnecessary
/// - RAII drop provides automatic cleanup
///
/// # Performance
///
/// Creating an arena from a factory is extremely cheap:
/// - Just a struct with a `usize` field
/// - Arena creation allocates the first chunk
/// - No allocation overhead for the factory itself
#[derive(Debug, Clone, Copy)]
pub struct ArenaFactory {
    /// Size of chunks in arenas created by this factory.
    chunk_size: usize,
}

impl ArenaFactory {
    /// Creates a new arena factory with the specified chunk size.
    ///
    /// # Arguments
    ///
    /// * `chunk_size` - The size of chunks for arenas created by this factory.
    ///   Must be at least 8 KiB and will be rounded up to a power of two.
    ///
    /// # Examples
    ///
    /// ```
    /// use oxidex_mem::factory::ArenaFactory;
    ///
    /// let factory = ArenaFactory::new(64 * 1024); // 64 KiB chunks
    /// ```
    #[must_use]
    pub const fn new(chunk_size: usize) -> Self {
        Self { chunk_size }
    }

    /// Creates a new arena with the factory's configured chunk size.
    ///
    /// # Returns
    ///
    /// A new `LocalArena` instance. The arena is freshly created and not
    /// reused from any pool.
    ///
    /// # Examples
    ///
    /// ```
    /// use oxidex_mem::factory::ArenaFactory;
    ///
    /// let factory = ArenaFactory::new(8192);
    /// let arena = factory.create_arena();
    ///
    /// // Use the arena...
    /// // When arena is dropped, all memory is reclaimed
    /// ```
    #[must_use]
    pub fn create_arena(&self) -> LocalArena {
        LocalArena::new(self.chunk_size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_factory_creation() {
        let factory = ArenaFactory::new(8192);
        assert_eq!(factory.chunk_size, 8192);
    }

    #[test]
    fn test_factory_create_arena() {
        let factory = ArenaFactory::new(8192);
        let mut arena = factory.create_arena();

        // Allocate something
        let value: *mut u32 = arena.alloc(42);
        unsafe {
            assert_eq!(*value, 42);
        }
    }

    #[test]
    fn test_factory_multiple_arenas() {
        let factory = ArenaFactory::new(8192);

        // Create multiple arenas
        let mut arena1 = factory.create_arena();
        let mut arena2 = factory.create_arena();

        // Each arena is independent
        let val1: *mut u32 = arena1.alloc(1);
        let val2: *mut u32 = arena2.alloc(2);

        unsafe {
            assert_eq!(*val1, 1);
            assert_eq!(*val2, 2);
        }

        // Pointers should be different (different arenas)
        assert_ne!(val1, val2);
    }

    #[test]
    fn test_factory_copy() {
        let factory1 = ArenaFactory::new(8192);
        let factory2 = factory1; // Copy

        // Both should work
        let mut arena1 = factory1.create_arena();
        let mut arena2 = factory2.create_arena();

        let val1: *mut u32 = arena1.alloc(42);
        let val2: *mut u32 = arena2.alloc(100);

        unsafe {
            assert_eq!(*val1, 42);
            assert_eq!(*val2, 100);
        }
    }
}
