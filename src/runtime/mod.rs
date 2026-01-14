//! `OxideC` runtime module.
//!
//! This module provides the core runtime infrastructure for `OxideC`, including:
//!
//! - Arena allocation for long-lived metadata
//! - Thread-safe and thread-local allocators
//! - Runtime initialization and global state
//!
//! # Architecture
//!
//! The runtime is organized into several modules:
//!
//! - [`arena`]: Arena allocators for high-performance memory allocation
//! - `object`: Object allocation and lifecycle management (TODO)
//! - `class`: Class creation and inheritance (TODO)
//! - `selector`: Selector interning and caching (TODO)
//! - `dispatch`: Message dispatch system (TODO)
//! - `cache`: Method caching (TODO)
//! - `protocol`: Protocol conformance checking (TODO)
//!
//! # Global Arena
//!
//! The runtime maintains a global arena for allocating long-lived metadata
//! such as selectors, classes, and protocols. This arena is initialized once
//! and lives for the entire program duration.
//!
//! # Example
//!
//! ```rust
//! use oxidec::runtime::get_global_arena;
//!
//! let arena = get_global_arena();
//! let value: *mut u32 = arena.alloc(42);
//! ```

pub mod arena;
pub mod string;

pub use arena::{Arena, LocalArena};
pub use string::RuntimeString;

use std::sync::OnceLock;

/// Global arena for allocating long-lived runtime metadata.
///
/// This arena is initialized once on first use and lives for the entire
/// program duration. It's thread-safe and can be accessed from any thread.
///
/// # Thread Safety
///
/// The global arena uses atomic operations for allocation, making it safe
/// to access from multiple threads concurrently.
static GLOBAL_ARENA: OnceLock<Arena> = OnceLock::new();

/// Returns a reference to the global arena.
///
/// This function initializes the global arena on first call with a 4 KiB
/// initial chunk size. Subsequent calls return the same arena instance.
///
/// # Returns
///
/// A static reference to the global arena, valid for the entire program duration.
///
/// # Example
///
/// ```rust
/// use oxidec::runtime::get_global_arena;
///
/// let arena = get_global_arena();
/// let ptr: *mut u32 = arena.alloc(42);
///
/// unsafe {
///     assert_eq!(*ptr, 42);
/// }
/// ```
#[must_use]
pub fn get_global_arena() -> &'static Arena {
    GLOBAL_ARENA.get_or_init(|| Arena::with_config(4096, 16))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_global_arena_initialization() {
        let arena1 = get_global_arena();
        let arena2 = get_global_arena();

        // Should return the same instance
        assert_eq!(arena1 as *const Arena as usize, arena2 as *const Arena as usize);
    }

    #[test]
    fn test_global_arena_allocation() {
        let arena = get_global_arena();

        let ptr: *mut u32 = arena.alloc(42);
        unsafe {
            assert_eq!(*ptr, 42);
        }
    }

    #[test]
    fn test_global_arena_multiple_allocations() {
        let arena = get_global_arena();

        let ptr1: *mut u32 = arena.alloc(1);
        let ptr2: *mut u64 = arena.alloc(2);
        let ptr3: *mut u32 = arena.alloc(3);

        unsafe {
            assert_eq!(*ptr1, 1);
            assert_eq!(*ptr2, 2);
            assert_eq!(*ptr3, 3);
        }
    }
}
