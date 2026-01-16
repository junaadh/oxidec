// Comprehensive selector interning benchmarks for Phase 3b optimization
//
// This benchmark suite measures:
// - Cache hit/miss performance
// - Hash function comparison (DefaultHasher, FxHash, AHash)
// - Lock contention under concurrency
// - Collision handling
// - Integration with dispatch

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use oxidec::runtime::Selector;
use std::hash::{Hash, Hasher};
use std::str::FromStr;
use std::thread;

/// Benchmark cache hit performance - repeatedly intern the same selector
fn bench_cache_hit(c: &mut Criterion) {
    // Pre-intern the selector to ensure cache hit
    let _selector = Selector::from_str("initWithObject:").unwrap();

    c.bench_function("selector_cache_hit", |b| {
        b.iter(|| {
            black_box(Selector::from_str("initWithObject:").unwrap())
        })
    });
}

/// Benchmark cache miss performance - intern unique selectors
fn bench_cache_miss(c: &mut Criterion) {
    let mut counter = 0u64;
    c.bench_function("selector_cache_miss", |b| {
        b.iter(|| {
            counter = counter.wrapping_add(1);
            let selector = format!("uniqueSelector{}:", counter);
            black_box(Selector::from_str(&selector).unwrap())
        })
    });
}

/// Benchmark hash computation for different string lengths
fn bench_hash_computation(c: &mut Criterion) {
    let mut group = c.benchmark_group("hash_computation");

    let test_cases = vec![
        ("init", 4),
        ("initWithObject:", 15),
        ("performSelector:withObject:afterDelay:", 35),
        ("dictionaryWithObjectsAndKeys:count:", 36),
    ];

    for (name, length) in test_cases {
        // DefaultHasher
        group.bench_with_input(BenchmarkId::new("DefaultHasher", length), name, |b, s| {
            b.iter(|| {
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                s.hash(&mut hasher);
                black_box(hasher.finish())
            })
        });

        // FxHash
        group.bench_with_input(BenchmarkId::new("FxHash", length), name, |b, s| {
            b.iter(|| {
                let mut hasher = fxhash::FxHasher::default();
                s.hash(&mut hasher);
                black_box(hasher.finish())
            })
        });

        // AHash
        group.bench_with_input(BenchmarkId::new("AHash", length), name, |b, s| {
            b.iter(|| {
                let mut hasher = ahash::AHasher::default();
                s.hash(&mut hasher);
                black_box(hasher.finish())
            })
        });
    }

    group.finish();
}

/// Benchmark lock contention with multiple threads
fn bench_lock_contention(c: &mut Criterion) {
    let mut group = c.benchmark_group("lock_contention");

    for thread_count in [1, 2, 4, 8, 16].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(thread_count), thread_count, |b, &num_threads| {
            b.iter(|| {
                let handles: Vec<_> = (0..num_threads)
                    .map(|_| {
                        thread::spawn(|| {
                            // Each thread interns the same selector (contention on cache hit)
                            for _ in 0..1000 {
                                black_box(Selector::from_str("initWithObject:").unwrap());
                            }
                        })
                    })
                    .collect();

                for handle in handles {
                    handle.join().unwrap();
                }
            })
        });
    }

    group.finish();
}

/// Benchmark collision handling - create selectors that hash to same bucket
fn bench_collision_handling(c: &mut Criterion) {
    // Create selectors that will cause collisions
    // Note: This is approximate since we can't easily force specific hash collisions
    let selectors: Vec<String> = (0..100)
        .map(|i| format!("method{}:", i))
        .collect();

    c.bench_function("collision_handling", |b| {
        b.iter(|| {
            // Intern multiple selectors (some will collide in the same bucket)
            for selector in &selectors {
                black_box(Selector::from_str(selector).unwrap());
            }
        })
    });
}

/// Benchmark dispatch integration - selector creation + hash lookup
fn bench_dispatch_integration(c: &mut Criterion) {
    // Pre-intern the selector
    let selector = Selector::from_str("doSomething:").unwrap();

    c.bench_function("dispatch_with_cached_selector", |b| {
        b.iter(|| {
            // This measures selector hash lookup (used in dispatch)
            black_box(selector.hash());
        })
    });
}

/// Benchmark throughput - selectors interned per second
fn bench_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("throughput");

    for size in [10, 100, 1000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &n| {
            let selectors: Vec<String> = (0..n)
                .map(|i| format!("selector{}:", i))
                .collect();

            b.iter(|| {
                for selector in &selectors {
                    black_box(Selector::from_str(selector).unwrap());
                }
            })
        });
    }

    group.finish();
}

/// Benchmark comparison - cache hit vs cache miss
fn bench_hit_vs_miss(c: &mut Criterion) {
    // Cache hit - same selector
    c.bench_function("hit_vs_miss_hit", |b| {
        b.iter(|| {
            black_box(Selector::from_str("cachedSelector:").unwrap())
        })
    });

    // Cache miss - different selectors
    let mut counter = 0;
    c.bench_function("hit_vs_miss_miss", |b| {
        b.iter(|| {
            counter = (counter + 1) % 1000;
            let selector = format!("uniqueSelector{}:", counter);
            black_box(Selector::from_str(&selector).unwrap())
        })
    });
}

criterion_group!(
    benches,
    bench_cache_hit,
    bench_cache_miss,
    bench_hash_computation,
    bench_lock_contention,
    bench_collision_handling,
    bench_dispatch_integration,
    bench_throughput,
    bench_hit_vs_miss
);

criterion_main!(benches);
