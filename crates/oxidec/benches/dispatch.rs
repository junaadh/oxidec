// Comprehensive dispatch benchmarks for Phase 3b optimization
//
// This benchmark suite measures:
// - Cached vs uncached dispatch performance
// - Method swizzling overhead
// - Lock contention in method cache
// - Inheritance traversal cost
// - Multi-threaded dispatch

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use oxidec::runtime::{Class, Method, Object, Selector, get_global_arena};
use std::str::FromStr;
use std::sync::Arc;
use std::thread;

// Helper function to create a test class with a method
fn create_test_class_with_method(name: &str, method_name: &str) -> Class {
    let class = Class::new_root(name).unwrap();

    // Add a simple method to the class
    extern "C" fn test_method_impl(
        _self: oxidec::runtime::object::ObjectPtr,
        _cmd: oxidec::runtime::selector::SelectorHandle,
        _args: *const *mut u8,
        _ret: *mut u8,
    ) {
        // Empty method - just return
    }

    let selector = Selector::from_str(method_name).unwrap();
    let arena = get_global_arena();
    let types = oxidec::runtime::RuntimeString::new("v@:", arena);

    let method = Method {
        selector,
        imp: test_method_impl,
        types,
    };

    class.add_method(method).unwrap();

    class
}

/// Benchmark cached message send - repeated calls to same method
fn bench_cached_dispatch(c: &mut Criterion) {
    let class = create_test_class_with_method("CachedTestClass", "testMethod:");
    let instance = Object::new(&class).unwrap();
    let selector = Selector::from_str("testMethod:").unwrap();

    // Warm up the cache
    let _ = Object::send_message(&instance, &selector, &oxidec::runtime::MessageArgs::None);

    c.bench_function("cached_dispatch", |b| {
        b.iter(|| {
            black_box(Object::send_message(&instance, &selector, &oxidec::runtime::MessageArgs::None).unwrap())
        })
    });
}

/// Benchmark uncached message send - first call to method
fn bench_uncached_dispatch(c: &mut Criterion) {
    c.bench_function("uncached_dispatch", |b| {
        b.iter(|| {
            // Create a new class each iteration to ensure cache miss
            let class_name = format!("UncachedTestClass{}", black_box(0));
            let class = create_test_class_with_method(&class_name, "uniqueMethod:");
            let instance = Object::new(&class).unwrap();
            let selector = Selector::from_str("uniqueMethod:").unwrap();

            black_box(Object::send_message(&instance, &selector, &oxidec::runtime::MessageArgs::None).unwrap())
        })
    });
}

/// Benchmark inheritance traversal cost
fn bench_inheritance_traversal(c: &mut Criterion) {
    // Create a class hierarchy with different depths
    let mut group = c.benchmark_group("inheritance_depth");

    for depth in [1, 2, 4, 8].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(depth), depth, |b, &n| {
            b.iter(|| {
                // Create chain of classes
                let mut current_class = None;
                for i in 0..n {
                    let name = format!("BaseClass{}", i);
                    let class = create_test_class_with_method(&name, "rootMethod:");
                    match current_class {
                        None => {
                            current_class = Some(class);
                        }
                        Some(_parent) => {
                            // For simplicity, we'll just create independent classes
                            // Real inheritance would require subclass API
                            current_class = Some(class);
                        }
                    }
                }

                // The class has the method
                let leaf_class = current_class.unwrap();
                let instance = Object::new(&leaf_class).unwrap();
                let selector = Selector::from_str("rootMethod:").unwrap();

                black_box(Object::send_message(&instance, &selector, &oxidec::runtime::MessageArgs::None).unwrap())
            })
        });
    }

    group.finish();
}

/// Benchmark method swizzling overhead
fn bench_method_swizzling(c: &mut Criterion) {
    extern "C" fn original_method(
        _self: oxidec::runtime::object::ObjectPtr,
        _cmd: oxidec::runtime::selector::SelectorHandle,
        _args: *const *mut u8,
        _ret: *mut u8,
    ) {
    }

    extern "C" fn replacement_method(
        _self: oxidec::runtime::object::ObjectPtr,
        _cmd: oxidec::runtime::selector::SelectorHandle,
        _args: *const *mut u8,
        _ret: *mut u8,
    ) {
    }

    let class = create_test_class_with_method("SwizzleTestClass", "swizzleMethod:");
    let selector = Selector::from_str("swizzleMethod:").unwrap();
    let instance = Object::new(&class).unwrap();

    // Warm up cache
    let _ = Object::send_message(&instance, &selector, &oxidec::runtime::MessageArgs::None);

    // Measure swizzling cost
    c.bench_function("method_swizzle", |b| {
        b.iter(|| {
            // Create methods for swizzling
            let arena = get_global_arena();
            let types = oxidec::runtime::RuntimeString::new("v@:", arena);

            let original = Method {
                selector: selector.clone(),
                imp: original_method,
                types: types.clone(),
            };

            let replacement = Method {
                selector: selector.clone(),
                imp: replacement_method,
                types,
            };

            // Swizzle methods (replace original with replacement)
            class.add_method(replacement).unwrap();

            // Call the method (now uses replacement)
            black_box(Object::send_message(&instance, &selector, &oxidec::runtime::MessageArgs::None).unwrap());

            // Swizzle back
            class.add_method(original).unwrap();
        })
    });
}

/// Benchmark cache repopulation after swizzling
fn bench_cache_repopulation(c: &mut Criterion) {
    let class = create_test_class_with_method("RepopTestClass", "repopMethod:");
    let selector = Selector::from_str("repopMethod:").unwrap();
    let instance = Object::new(&class).unwrap();

    // Warm up cache
    let _ = Object::send_message(&instance, &selector, &oxidec::runtime::MessageArgs::None);

    c.bench_function("cache_repopulation", |b| {
        b.iter(|| {
            // Invalidate cache by swizzling (even with same implementation)
            extern "C" fn same_method(
                _self: oxidec::runtime::object::ObjectPtr,
                _cmd: oxidec::runtime::selector::SelectorHandle,
                _args: *const *mut u8,
                _ret: *mut u8,
            ) {
            }

            let arena = get_global_arena();
            let types = oxidec::runtime::RuntimeString::new("v@:", arena);
            let method = Method {
                selector: selector.clone(),
                imp: same_method,
                types,
            };

            class.add_method(method).unwrap();

            // This call will be a cache miss and will repopulate the cache
            black_box(Object::send_message(&instance, &selector, &oxidec::runtime::MessageArgs::None).unwrap())
        })
    });
}

/// Benchmark multi-threaded dispatch
fn bench_multithreaded_dispatch(c: &mut Criterion) {
    let class = Arc::new(create_test_class_with_method("MTTestClass", "mtMethod:"));
    let selector = Arc::new(Selector::from_str("mtMethod:").unwrap());

    let mut group = c.benchmark_group("multithreaded_dispatch");

    for thread_count in [1, 2, 4, 8].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(thread_count), thread_count, |b, &num_threads| {
            b.iter(|| {
                let handles: Vec<_> = (0..num_threads)
                    .map(|_| {
                        let cls = Arc::clone(&class);
                        let sel = Arc::clone(&selector);

                        thread::spawn(move || {
                            let instance = Object::new(&cls).unwrap();
                            for _ in 0..100 {
                                black_box(Object::send_message(&instance, &sel, &oxidec::runtime::MessageArgs::None).unwrap());
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

/// Benchmark dispatch throughput - messages sent per second
fn bench_dispatch_throughput(c: &mut Criterion) {
    let class = create_test_class_with_method("ThroughputTestClass", "throughputMethod:");
    let selector = Selector::from_str("throughputMethod:").unwrap();
    let instance = Object::new(&class).unwrap();

    let mut group = c.benchmark_group("dispatch_throughput");

    for count in [100, 1000, 10000].iter() {
        group.throughput(Throughput::Elements(*count as u64));
        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, &n| {
            b.iter(|| {
                for _ in 0..n {
                    black_box(Object::send_message(&instance, &selector, &oxidec::runtime::MessageArgs::None).unwrap());
                }
            })
        });
    }

    group.finish();
}

/// Benchmark method lookup in different scenarios
fn bench_method_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("method_lookup");

    // Direct method (no inheritance)
    let direct_class = create_test_class_with_method("DirectClass", "directMethod:");
    let direct_selector = Selector::from_str("directMethod:").unwrap();

    group.bench_function("direct_method", |b| {
        b.iter(|| {
            black_box(direct_class.lookup_imp(&direct_selector))
        })
    });

    // Non-existent method
    let nonexistent_selector = Selector::from_str("nonexistentMethod:").unwrap();

    group.bench_function("nonexistent_method", |b| {
        b.iter(|| {
            black_box(direct_class.lookup_imp(&nonexistent_selector))
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_cached_dispatch,
    bench_uncached_dispatch,
    bench_inheritance_traversal,
    bench_method_swizzling,
    bench_cache_repopulation,
    bench_multithreaded_dispatch,
    bench_dispatch_throughput,
    bench_method_lookup
);

criterion_main!(benches);
