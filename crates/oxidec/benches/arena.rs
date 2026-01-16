// Arena allocator benchmarks for OxideC runtime
//
// These benchmarks measure the performance of the arena allocator,
// including sequential allocations, mixed workloads, chunk growth,
// and thread-local vs global arena comparisons.

use criterion::{
    BenchmarkId, Criterion, black_box, criterion_group, criterion_main,
};
use oxidec::runtime::{Arena, arena::LocalArena};

/// Benchmark sequential allocations of different sizes.
///
/// Tests how fast the arena can allocate memory when called repeatedly
/// with the same allocation size. This measures the pure allocation
/// overhead without chunk management complexity.
fn bench_sequential_allocations(c: &mut Criterion) {
    let mut group = c.benchmark_group("sequential_alloc");
    group.sample_size(1000);

    for size in &[4, 16, 64, 256, 1024, 4096] {
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            size,
            |b, &size| {
                let arena = Arena::new(4096);
                b.iter(|| {
                    arena.alloc(black_box(size));
                });
            },
        );
    }

    group.finish();
}

/// Benchmark mixed-size allocations simulating real workloads.
///
/// Tests allocation performance with a variety of sizes in a single benchmark,
/// more closely matching real usage patterns where allocation sizes vary.
fn bench_mixed_allocations(c: &mut Criterion) {
    let mut group = c.benchmark_group("mixed_alloc");
    group.sample_size(1000);

    group.bench_function("variable_sizes", |b| {
        let arena = Arena::new(4096);
        let sizes = [4, 16, 64, 256, 1024];
        let mut i = 0;

        b.iter(|| {
            arena.alloc(black_box(sizes[i % sizes.len()]));
            i += 1;
        });
    });

    group.finish();
}

/// Benchmark chunk growth behavior.
///
/// Tests how the arena performs when it needs to allocate new chunks
/// as the current one fills up. This measures the overhead of
/// chunk expansion.
fn bench_chunk_growth(c: &mut Criterion) {
    let mut group = c.benchmark_group("chunk_growth");
    group.sample_size(100);

    group.bench_function("large_allocations", |b| {
        b.iter(|| {
            let arena = Arena::with_config(1024, 16);
            // Allocate enough to force chunk growth
            for _ in 0..100 {
                arena.alloc(black_box(64));
            }
        });
    });

    group.finish();
}

/// Benchmark thread-local arena vs global arena.
///
/// Compares `LocalArena` (no atomic operations) to Arena (atomic bump pointer)
/// to measure the overhead of thread safety.
fn bench_thread_local_vs_global(c: &mut Criterion) {
    let mut group = c.benchmark_group("arena_comparison");
    group.sample_size(10000);

    // Global arena with atomic operations
    group.bench_function("global_arena", |b| {
        let arena = Arena::new(4096);
        b.iter(|| {
            arena.alloc(black_box(64));
        });
    });

    // Thread-local arena without atomic operations
    group.bench_function("local_arena", |b| {
        let mut arena = LocalArena::new(4096);
        b.iter(|| {
            arena.alloc(black_box(64));
        });
    });

    group.finish();
}

/// Benchmark arena statistics access.
///
/// Tests how expensive it is to query arena statistics like
/// allocated bytes and chunk count.
fn bench_arena_stats(c: &mut Criterion) {
    let mut group = c.benchmark_group("arena_stats");
    group.sample_size(10000);

    group.bench_function("stats", |b| {
        let arena = Arena::new(4096);
        // Pre-populate arena
        for _ in 0..10 {
            arena.alloc(256);
        }

        b.iter(|| {
            black_box(arena.stats());
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_sequential_allocations,
    bench_mixed_allocations,
    bench_chunk_growth,
    bench_thread_local_vs_global,
    bench_arena_stats,
);
criterion_main!(benches);
