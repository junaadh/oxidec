// Proxy benchmarks for OxideC runtime
//
// These benchmarks measure the performance of proxy infrastructure,
// including proxy overhead, composition, and bypass optimization.

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use oxidec::runtime::{Class, Object, Selector, TransparentProxy, compose_proxies};
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};

static BENCH_ID: AtomicUsize = AtomicUsize::new(0);

fn setup_test() -> (Class, Object, Selector) {
    let id = BENCH_ID.fetch_add(1, Ordering::SeqCst);
    let class_name = format!("BenchProxy_{id}");
    let class = Class::new_root(&class_name).unwrap();
    let object = Object::new(&class).unwrap();
    let selector = Selector::from_str("benchMethod:").unwrap();
    (class, object, selector)
}

/// Benchmark transparent proxy creation.
///
/// Measures the time to create a transparent proxy, which should be
/// < 500ns for practical use.
fn bench_transparent_proxy_creation(c: &mut Criterion) {
    let (_class, target, _selector) = setup_test();

    c.bench_function("transparent_proxy_creation", |b| {
        b.iter(|| {
            let proxy = TransparentProxy::new(black_box(&target)).unwrap();
            black_box(proxy);
        });
    });
}

/// Benchmark proxy forwarding overhead vs direct call.
///
/// This is the key metric - proxy forwarding should be < 2x direct call.
/// In a full implementation with forwarding hooks, this would measure
/// the actual message send overhead. For now, we measure proxy creation.
fn bench_proxy_overhead(c: &mut Criterion) {
    let (_class, target, _selector) = setup_test();

    let mut group = c.benchmark_group("proxy_overhead");
    group.sample_size(1000);

    // Measure direct object access
    group.bench_function("direct_access", |b| {
        b.iter(|| {
            let obj = black_box(&target);
            black_box(obj.class());
        });
    });

    // Measure proxy access
    group.bench_function("proxy_access", |b| {
        b.iter(|| {
            let proxy = TransparentProxy::new(black_box(&target)).unwrap();
            black_box(proxy.as_object().class());
        });
    });

    group.finish();
}

/// Benchmark proxy composition.
///
/// Measures the overhead of composing multiple proxies in a chain.
/// Each additional proxy should add minimal overhead.
fn bench_proxy_composition(c: &mut Criterion) {
    let (_class, target, _selector) = setup_test();

    let mut group = c.benchmark_group("proxy_composition");
    group.sample_size(1000);

    for proxy_count in &[1, 2, 3, 5] {
        group.bench_with_input(
            BenchmarkId::new("compose_proxies", proxy_count),
            proxy_count,
            |b, &proxy_count| {
                b.iter(|| {
                    let proxies: Vec<_> = (0..proxy_count)
                        .map(|_| TransparentProxy::new(&target).unwrap())
                        .map(|p| p.into_object())
                        .collect();

                    let proxy_refs: Vec<_> = proxies.iter().collect();
                    let composed = compose_proxies(&proxy_refs).unwrap();
                    black_box(composed);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark proxy memory allocation overhead.
///
/// Measures the memory allocation cost of creating proxies vs direct objects.
fn bench_proxy_allocation(c: &mut Criterion) {
    let (class, _target, _selector) = setup_test();

    let mut group = c.benchmark_group("proxy_allocation");
    group.sample_size(1000);

    // Baseline: direct object allocation
    group.bench_function("direct_object", |b| {
        b.iter(|| {
            let obj = Object::new(black_box(&class)).unwrap();
            black_box(obj);
        });
    });

    // Proxy object allocation
    group.bench_function("proxy_object", |b| {
        b.iter(|| {
            let id = BENCH_ID.fetch_add(1, Ordering::SeqCst);
            let proxy_class = Class::new_root(&format!("BenchProxyClass_{id}")).unwrap();
            let obj = Object::new(black_box(&proxy_class)).unwrap();
            black_box(obj);
        });
    });

    group.finish();
}

/// Benchmark bypass proxy optimization.
///
/// In a full implementation, this would measure the performance improvement
/// from bypassing the forwarding pipeline for known fast-path methods.
fn bench_bypass_proxy(c: &mut Criterion) {
    let (_class, target, _selector) = setup_test();

    c.bench_function("bypass_proxy_creation", |b| {
        b.iter(|| {
            let fast_method = Selector::from_str("fastMethod").unwrap();
            let proxy = oxidec::runtime::bypass_proxy(
                black_box(&target),
                black_box(vec![fast_method]),
            ).unwrap();
            black_box(proxy);
        });
    });
}

/// Benchmark concurrent proxy access.
///
/// Measures proxy performance under multi-threaded contention.
/// Since proxies are independent objects, there should be no contention.
fn bench_concurrent_proxy_access(c: &mut Criterion) {
    let (_class, target, _selector) = setup_test();

    let mut group = c.benchmark_group("concurrent_proxy");
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
                            let b = barrier.clone();
                            std::thread::spawn(move || {
                                b.wait();
                                for _ in 0..100 {
                                    let _ = TransparentProxy::new(&t);
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

criterion_group!(
    benches,
    bench_transparent_proxy_creation,
    bench_proxy_overhead,
    bench_proxy_composition,
    bench_proxy_allocation,
    bench_bypass_proxy,
    bench_concurrent_proxy_access,
);

criterion_main!(benches);
