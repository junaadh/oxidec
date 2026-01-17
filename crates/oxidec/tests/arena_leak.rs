// Arena allocation tests for oxidex-mem integration
//
// These tests validate the GlobalArena functionality from oxidex-mem
// as used by the oxidec runtime.

#[cfg(test)]
mod global_arena_tests {
    use oxidex_mem::GlobalArena;
    use std::sync::Arc;
    use std::thread;

    /// Test basic allocation in GlobalArena
    #[test]
    fn test_global_arena_basic_allocation() {
        let arena = GlobalArena::new(4096);

        let data1: &mut u32 = arena.alloc(42);
        let data2: &mut u64 = arena.alloc(100);

        assert_eq!(*data1, 42);
        assert_eq!(*data2, 100);
    }

    /// Test that GlobalArena is thread-safe
    #[test]
    fn test_global_arena_thread_safety() {
        let arena = Arc::new(GlobalArena::new(4096));
        let arena_clone1 = Arc::clone(&arena);
        let arena_clone2 = Arc::clone(&arena);

        let handle1 = thread::spawn(move || {
            for i in 0..100 {
                let _data: &mut u32 = arena_clone1.alloc(i);
            }
        });

        let handle2 = thread::spawn(move || {
            for i in 0..100 {
                let _data: &mut u32 = arena_clone2.alloc(i);
            }
        });

        handle1.join().unwrap();
        handle2.join().unwrap();

        // Should have allocated successfully without data races
        let stats = arena.stats();
        assert!(stats.total_allocated > 0);
    }

    /// Test GlobalArena stats
    #[test]
    fn test_global_arena_stats() {
        let arena = GlobalArena::new(4096);

        let _data1: &mut u32 = arena.alloc(42);
        let _data2: &mut u64 = arena.alloc(100);

        let stats = arena.stats();
        assert!(stats.total_allocated > 0);
        assert_eq!(stats.chunk_count, 1);
    }

    /// Test GlobalArena with large allocations
    #[test]
    fn test_global_arena_large_allocation() {
        let arena = GlobalArena::new(4096);

        struct LargeData {
            _a: [u64; 100],
            _b: [u32; 100],
        }

        let _data = arena.alloc(LargeData {
            _a: [0; 100],
            _b: [0; 100],
        });

        let stats = arena.stats();
        // Verify that some allocation occurred
        assert!(stats.total_allocated > 0);
        assert!(stats.total_allocated >= std::mem::size_of::<LargeData>());
    }

    /// Test GlobalArena with multiple types
    #[test]
    fn test_global_arena_multiple_types() {
        let arena = GlobalArena::new(4096);

        let _int: &mut i32 = arena.alloc(-42);
        let _uint: &mut u32 = arena.alloc(42);
        let _float: &mut f64 = arena.alloc(3.148);
        let _bool: &mut bool = arena.alloc(true);
        let _char: &mut char = arena.alloc('a');

        let stats = arena.stats();
        assert!(stats.total_allocated > 0);
    }

    /// Test that GlobalArena can be shared via Arc
    #[test]
    fn test_global_arena_arc_sharing() {
        let arena = Arc::new(GlobalArena::new(4096));

        // Allocate from shared arena
        let arena_clone = Arc::clone(&arena);
        let data = arena_clone.alloc(42u32);

        // data is valid as long as arena exists
        assert_eq!(*data, 42);

        // Keep data alive while arena exists
        assert_eq!(*data, 42);

        // Both arena_clone and data dropped here
        drop(arena_clone);

        // Original arena still has the allocation
        assert!(arena.stats().total_allocated > 0);
    }

    /// Test GlobalArena with many allocations
    #[test]
    fn test_global_arena_many_allocations() {
        let arena = GlobalArena::new(4096);

        // Allocate many items to force chunk growth
        for i in 0..1000 {
            let _data: &mut u32 = arena.alloc(i);
        }

        let stats = arena.stats();
        assert!(stats.chunk_count >= 1);
        assert!(stats.total_allocated > 0);
    }

    /// Test GlobalArena stats are accurate
    #[test]
    fn test_global_arena_stats_accuracy() {
        let arena = GlobalArena::new(1024); // Small chunk size

        let _data1: &mut u32 = arena.alloc(42);
        let _data2: &mut u64 = arena.alloc(100);
        let _data3: &mut u32 = arena.alloc(200);

        let stats = arena.stats();
        assert!(
            stats.total_allocated
                >= std::mem::size_of::<u32>()
                    + std::mem::size_of::<u64>()
                    + std::mem::size_of::<u32>()
        );
        assert!(stats.total_capacity >= stats.total_allocated);
    }
}

#[cfg(test)]
mod global_arena_singleton_tests {
    use oxidex_mem::global_arena;
    use std::thread;

    /// Test global_arena() returns the same instance
    #[test]
    fn test_global_arena_singleton() {
        let arena1 = global_arena();
        let arena2 = global_arena();

        // Should be the same instance
        assert!(std::ptr::eq(arena1, arena2));
    }

    /// Test global_arena() can be used from multiple threads
    #[test]
    fn test_global_arena_singleton_thread_safe() {
        let handles: Vec<_> = (0..10)
            .map(|_| {
                thread::spawn(|| {
                    let arena = global_arena();
                    let _data: &mut u32 = arena.alloc(42);
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        // All threads should have successfully allocated
        let arena = global_arena();
        let stats = arena.stats();
        assert!(stats.total_allocated > 0);
    }

    /// Test global_arena() persists across function calls
    #[test]
    fn test_global_arena_persistence() {
        fn allocate_in_function() -> &'static mut u32 {
            let arena = global_arena();
            arena.alloc(42)
        }

        let data1 = allocate_in_function();
        let data2 = allocate_in_function();

        // Both allocations should be valid
        assert_eq!(*data1, 42);
        assert_eq!(*data2, 42);

        // They should be different allocations
        assert!(!std::ptr::eq(data1, data2));
    }
}
