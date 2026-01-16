// Invocation pool benchmarks for OxideC runtime
//
// These benchmarks measure the performance of the invocation object pool,
// including pool hit rate, pooled vs direct allocation, and concurrent access.

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use oxidec::runtime::{Class, MessageArgs, Object, Selector, PooledInvocation, Invocation};
use std::str::FromStr;

fn setup_test() -> (Class, Object, Selector, MessageArgs) {
    let class = Class::new_root("BenchPool").unwrap();
    let object = Object::new(&class).unwrap();
    let selector = Selector::from_str("benchMethod:").unwrap();
    let args = MessageArgs::two(42, 99);
    (class, object, selector, args)
}

/// Benchmark pooled invocation creation (after warmup for pool hits).
///
/// Measures the time to acquire an invocation from the thread-local pool,
/// which should be ~100ns (vs ~300ns for direct allocation).
fn bench_pooled_invocation_creation(c: &mut Criterion) {
    let (_class, target, selector, args) = setup_test();

    // Warm up the pool
    for _ in 0..10 {
        let _ = PooledInvocation::with_arguments(&target, &selector, &args);
    }

    c.bench_function("pooled_invocation_creation", |b| {
        b.iter(|| {
            let pooled =
                PooledInvocation::with_arguments(black_box(&target), black_box(&selector), black_box(&args)).unwrap();
            black_box(pooled);
        });
    });
}

/// Benchmark direct invocation creation (no pooling).
///
/// Measures the time to allocate a new invocation without pooling,
/// which should be ~300ns. This is the baseline for comparison.
fn bench_direct_invocation_creation(c: &mut Criterion) {
    let (_class, target, selector, args) = setup_test();

    c.bench_function("direct_invocation_creation", |b| {
        b.iter(|| {
            let invocation =
                Invocation::with_arguments(black_box(&target), black_box(&selector), black_box(&args)).unwrap();
            black_box(invocation);
        });
    });
}

/// Benchmark invocation pool hit rate.
///
/// Measures how effectively the pool reuses invocations across different
/// workload patterns (small, medium, large).
fn bench_pool_hit_rate(c: &mut Criterion) {
    let (_class, target, selector, args) = setup_test();

    let mut group = c.benchmark_group("pool_hit_rate");
    group.sample_size(1000);

    for iterations in &[10, 100, 1000] {
        group.bench_with_input(
            BenchmarkId::from_parameter(iterations),
            iterations,
            |b, &iterations| {
                b.iter(|| {
                    // Clear pool to start fresh
                    PooledInvocation::clear_pool();

                    for _ in 0..iterations {
                        let _ =
                            PooledInvocation::with_arguments(&target, &selector, &args).unwrap();
                    }

                    // Check hit rate
                    if let Some(stats) = PooledInvocation::pool_stats() {
                        black_box(stats.hit_rate());
                    }
                });
            },
        );
    }

    group.finish();
}

/// Benchmark pool exhaustion handling.
///
/// Measures performance when the pool is exhausted and needs to fall back
/// to direct allocation. This should still be fast due to early detection.
fn bench_pool_exhaustion(c: &mut Criterion) {
    let (_class, target, selector, args) = setup_test();

    c.bench_function("pool_exhaustion_fallback", |b| {
        b.iter(|| {
            // Clear pool to ensure we exhaust it
            PooledInvocation::clear_pool();

            // Acquire many invocations to exhaust the pool
            let _invocations: Vec<_> = (0..300)
                .map(|_| PooledInvocation::with_arguments(&target, &selector, &args).unwrap())
                .collect();

            black_box(_invocations);
        });
    });
}

/// Benchmark concurrent pool access.
///
/// Measures pool performance under multi-threaded contention. Since each
/// thread has its own pool, there should be no lock contention.
fn bench_concurrent_pool_access(c: &mut Criterion) {
    let (_class, target, selector, args) = setup_test();

    let mut group = c.benchmark_group("concurrent_pool");
    group.sample_size(100);

    for threads in &[1, 2, 4, 8] {
        group.bench_with_input(
            BenchmarkId::new("threads", threads),
            threads,
            |b, &threads| {
                b.iter(|| {
                    use std::sync::Barrier;

                    let barrier = std::sync::Arc::new(Barrier::new(threads));
                    let handles: Vec<_> = (0..threads)
                        .map(|_| {
                            let t = target.clone();
                            let s = selector.clone();
                            let a = args.clone();
                            let b = barrier.clone();
                            std::thread::spawn(move || {
                                b.wait();
                                for _ in 0..100 {
                                    let _ = PooledInvocation::with_arguments(&t, &s, &a).unwrap();
                                }
                            })
                        })
                        .collect();

                    for handle in handles {
                        handle.join().unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

/// Benchmark pooled vs direct allocation overhead.
///
/// Direct comparison showing the speedup from pooling (target: 2-3x faster).
fn bench_pooled_vs_direct(c: &mut Criterion) {
    let (_class, target, selector, args) = setup_test();

    let mut group = c.benchmark_group("allocation_comparison");
    group.sample_size(1000);

    // Warm up the pool
    for _ in 0..10 {
        let _ = PooledInvocation::with_arguments(&target, &selector, &args);
    }

    group.bench_function("pooled", |b| {
        b.iter(|| {
            let pooled =
                PooledInvocation::with_arguments(black_box(&target), black_box(&selector), black_box(&args)).unwrap();
            black_box(pooled);
        });
    });

    group.bench_function("direct", |b| {
        b.iter(|| {
            let invocation =
                Invocation::with_arguments(black_box(&target), black_box(&selector), black_box(&args)).unwrap();
            black_box(invocation);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_pooled_invocation_creation,
    bench_direct_invocation_creation,
    bench_pool_hit_rate,
    bench_pool_exhaustion,
    bench_concurrent_pool_access,
    bench_pooled_vs_direct,
);

criterion_main!(benches);
