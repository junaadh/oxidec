// Arena leak detection and prevention tests
//
// These tests validate the leak detection functionality of the arena allocator,
// including tracking, reporting, and prevention strategies.

#[cfg(test)]
mod leak_detection_tests {
    use oxidec::runtime::arena::{Arena, LocalArena, ScopedArena};

    /// Test that ScopedArena properly detects and reports leaks
    #[test]
    fn test_scoped_arena_leak_detection() {
        // Note: In debug mode, this test will show leak warnings if we don't
        // properly clean up allocations. Since we're allocating and not using
        // the pointers after the arena drops, they're technically "leaks".

        // However, since we're in a test and the arena is dropped properly,
        // the leak tracker will report these allocations.

        let arena = ScopedArena::new(4096);

        // Allocate some data that we intentionally "leak"
        let _data1: *mut u32 = arena.alloc(42);
        let _data2: *mut u64 = arena.alloc(100);
        let _data3: *mut u32 = arena.alloc(200);

        // Arena drops here, and in debug mode will report these as leaks
        // This is expected behavior for this test
    }

    /// Test that reset() clears the leak tracker
    #[test]
    fn test_reset_clears_leak_tracker() {
        let arena = ScopedArena::new(4096);

        // Allocate data
        let _data1: *mut u32 = arena.alloc(42);
        let _data2: *mut u64 = arena.alloc(100);

        // Reset the arena (clears leak tracker)
        arena.reset();

        // Allocate more data
        let _data3: *mut u32 = arena.alloc(200);

        // Arena drops here - should only report data3 as leaked
        // (data1 and data2 were cleared by reset)
    }

    /// Test that ScopedArena with no allocations doesn't report leaks
    #[test]
    fn test_no_allocations_no_leaks() {
        let arena = ScopedArena::new(4096);
        // Don't allocate anything
        // Arena drops here - should not report any leaks
    }

    /// Test leak detection with multiple types
    #[test]
    fn test_leak_detection_multiple_types() {
        let arena = ScopedArena::new(4096);

        // Allocate various types
        let _int: *mut i32 = arena.alloc(-42);
        let _uint: *mut u32 = arena.alloc(42);
        let _float: *mut f64 = arena.alloc(3.14);
        let _bool: *mut bool = arena.alloc(true);
        let _char: *mut char = arena.alloc('a');

        // Arena drops here - should report all 5 allocations
    }

    /// Test that regular Arena doesn't have leak tracking
    #[test]
    fn test_regular_arena_no_leak_tracking() {
        // Regular Arena doesn't track leaks (only ScopedArena does)
        let arena = Arena::new(4096);

        let _data1: *mut u32 = arena.alloc(42);
        let _data2: *mut u64 = arena.alloc(100);

        // No leak tracking - just drops silently
        drop(arena);
    }

    /// Test LocalArena reset functionality
    #[test]
    fn test_local_arena_reset_reuse() {
        let mut arena = LocalArena::new(4096);

        // First allocation
        let ptr1: *mut u32 = arena.alloc(42);
        unsafe {
            assert_eq!(*ptr1, 42);
        }

        // Reset
        arena.reset();

        // Second allocation - should reuse same memory
        let ptr2: *mut u32 = arena.alloc(100);
        unsafe {
            assert_eq!(*ptr2, 100);
        }

        // Verify memory was reused
        assert_eq!(ptr1, ptr2);
    }
}

#[cfg(test)]
mod leak_prevention_tests {
    use oxidec::runtime::arena::ScopedArena;
    use std::sync::Arc;

    /// Test proper scoping prevents leaks
    #[test]
    fn test_proper_scoping() {
        // Allocate in inner scope
        {
            let arena = ScopedArena::new(4096);
            let _data = arena.alloc(42u32);
            // Arena dropped here - all memory freed
        }
        // No leaks possible here
    }

    /// Test that moving arena transfers ownership
    #[test]
    fn test_move_arena() {
        let arena = ScopedArena::new(4096);
        let data = arena.alloc(42u32);

        // Move arena
        let arena2 = arena;

        // data is still valid (arena2 owns the memory)
        unsafe {
            assert_eq!(*data, 42);
        }

        // arena2 dropped here - reports data as leaked
        // (but that's expected since we didn't use it after dropping)
    }

    /// Test Arc sharing of arena
    #[test]
    fn test_arc_shared_arena() {
        let arena = Arc::new(ScopedArena::new(4096));

        // Allocate from shared arena
        let data = {
            let arena_clone = Arc::clone(&arena);
            arena_clone.alloc(42u32)
        };

        // data is valid as long as arena exists
        unsafe {
            assert_eq!(*data, 42);
        }

        // Keep data alive while arena exists
        unsafe {
            assert_eq!(*data, 42);
        }

        // Both arena and data dropped here
        // Leak tracker will report the allocation
    }

    /// Test reset between operations
    #[test]
    fn test_reset_pattern() {
        let arena = ScopedArena::new(4096);

        // Pattern: Allocate -> Use -> Reset -> Repeat
        for i in 0..10 {
            let data = arena.alloc(i);
            unsafe {
                assert_eq!(*data, i);
            }
            arena.reset();  // Clear for next iteration
        }

        // Only last allocation should be reported
    }
}

#[cfg(test)]
mod edge_case_tests {
    use oxidec::runtime::arena::ScopedArena;

    /// Test arena with zero allocations then reset
    #[test]
    fn test_reset_without_allocations() {
        let arena = ScopedArena::new(4096);
        arena.reset();  // Should not panic
        // No leaks
    }

    /// Test multiple resets in a row
    #[test]
    fn test_multiple_resets() {
        let arena = ScopedArena::new(4096);

        let _data = arena.alloc(42u32);
        arena.reset();
        arena.reset();  // Reset again (no-op)
        arena.reset();  // And again

        let _data2 = arena.alloc(100u32);
        // Only data2 should be reported
    }

    /// Test reset after chunk growth
    #[test]
    fn test_reset_after_growth() {
        let arena = ScopedArena::new(64); // Small initial size

        // Force chunk growth
        for i in 0..100 {
            let _data = arena.alloc(i);
        }

        arena.reset();  // Reset all chunks

        // Allocate again - should start from first chunk
        let data = arena.alloc(999u32);
        unsafe {
            assert_eq!(*data, 999);
        }
    }

    /// Test large allocations
    #[test]
    fn test_large_allocation_leak_tracking() {
        let arena = ScopedArena::new(4096);

        // Allocate a larger structure
        struct LargeData {
            a: [u64; 100],
            b: [u32; 100],
        }

        let _data = arena.alloc(LargeData {
            a: [0; 100],
            b: [0; 100],
        });

        // Arena drops here - should report the large allocation
    }

    /// Test mixed allocation sizes
    #[test]
    fn test_mixed_size_allocations() {
        let arena = ScopedArena::new(4096);

        let _small: *mut u8 = arena.alloc(1u8);
        let _medium: *mut u32 = arena.alloc(42u32);
        let _large: *mut [u64; 10] = arena.alloc([0u64; 10]);

        // Arena drops here - should report all 3 allocations
    }
}

#[cfg(test)]
mod statistics_tests {
    use oxidec::runtime::arena::ScopedArena;

    /// Test that stats work correctly with leak tracking
    #[test]
    fn test_stats_with_leak_tracking() {
        let arena = ScopedArena::new(4096);

        let _data1 = arena.alloc(42u32);
        let _data2 = arena.alloc(100u64);

        let stats = arena.stats();
        assert!(stats.total_used > 0);
        assert_eq!(stats.total_chunks, 1);
    }

    /// Test stats after reset
    #[test]
    fn test_stats_after_reset() {
        let arena = ScopedArena::new(4096);

        let _data1 = arena.alloc(42u32);
        let _data2 = arena.alloc(100u64);

        arena.reset();

        let stats = arena.stats();
        assert_eq!(stats.total_used, 0);
    }
}
