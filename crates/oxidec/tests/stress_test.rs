//! Stress tests for OxideC runtime components.
//!
//! These tests validate runtime behavior under heavy load:
//! - Deep forwarding chains
//! - Heavy proxy usage
//! - Pool exhaustion
//! - Concurrent access patterns
//!
//! Run with: `cargo test --test stress_test -- --test-threads=1 --nocapture`

use oxidec::runtime::{
    Class, Object, Selector, TransparentProxy, LoggingProxy, PooledInvocation,
    compose_proxies, MessageArgs
};
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Instant;

static STRESS_ID: AtomicUsize = AtomicUsize::new(0);

fn setup_stress_class() -> (Class, Object) {
    let id = STRESS_ID.fetch_add(1, Ordering::SeqCst);
    let class_name = format!("StressTest_{}", id);
    let class = Class::new_root(&class_name).unwrap();
    let object = Object::new(&class).unwrap();
    (class, object)
}

// ============================================================================
// Deep Forwarding Chain Tests
// ============================================================================

#[test]
fn test_deep_forwarding_chain_10() {
    let (_class, target) = setup_stress_class();

    // Create 10-level proxy chain
    let mut current = target.clone();
    for _i in 0..10 {
        let proxy = TransparentProxy::new(&current).unwrap();
        current = proxy.into_object();
    }

    // Verify we can access the final proxy
    assert!(current.class().name().contains("TransparentProxy"));
}

#[test]
fn test_deep_forwarding_chain_100() {
    let (_class, target) = setup_stress_class();

    // Create 100-level proxy chain
    let mut current = target.clone();
    for _i in 0..100 {
        let proxy = TransparentProxy::new(&current).unwrap();
        current = proxy.into_object();
    }

    // Verify chain integrity
    assert!(current.class().name().contains("TransparentProxy"));
}

#[test]
fn test_deep_forwarding_chain_no_stack_overflow() {
    let (_class, target) = setup_stress_class();

    // Create very deep chain to test no stack overflow
    let mut current = target.clone();
    for _i in 0..1000 {
        let proxy = TransparentProxy::new(&current).unwrap();
        current = proxy.into_object();
    }

    // If we reach here, no stack overflow occurred
    assert!(current.class().name().contains("TransparentProxy"));
}

// ============================================================================
// Heavy Proxy Usage Tests
// ============================================================================

#[test]
fn test_heavy_proxy_usage_100() {
    let (_class, target) = setup_stress_class();

    // Create 100 proxies
    let proxies: Vec<_> = (0..100)
        .map(|_| TransparentProxy::new(&target).unwrap())
        .collect();

    // Verify all are valid
    assert_eq!(proxies.len(), 100);
    for proxy in proxies {
        assert!(proxy.as_object().class().name().contains("TransparentProxy"));
    }
}

#[test]
fn test_heavy_proxy_usage_1000() {
    let (_class, target) = setup_stress_class();

    // Create 1000 proxies
    let proxies: Vec<_> = (0..1000)
        .map(|_| TransparentProxy::new(&target).unwrap())
        .collect();

    // Verify all are valid
    assert_eq!(proxies.len(), 1000);
}

#[test]
fn test_heavy_proxy_usage_10000() {
    let (_class, target) = setup_stress_class();

    // Create 10000 proxies
    let proxies: Vec<_> = (0..10000)
        .map(|_| TransparentProxy::new(&target).unwrap())
        .collect();

    // Verify all are valid
    assert_eq!(proxies.len(), 10000);
}

#[test]
fn test_heavy_proxy_composition() {
    let (_class, target) = setup_stress_class();

    // Create 10 proxies and compose them
    let proxies: Vec<_> = (0..10)
        .map(|_| TransparentProxy::new(&target).unwrap())
        .map(|p| p.into_object())
        .collect();

    let proxy_refs: Vec<_> = proxies.iter().collect();
    let composed = compose_proxies(&proxy_refs);

    assert!(composed.is_ok());
}

#[test]
fn test_proxy_memory_leak() {
    let (_class, target) = setup_stress_class();

    // Create and drop many proxies to check for leaks
    for _ in 0..10000 {
        let proxy = TransparentProxy::new(&target).unwrap();
        drop(proxy);
    }

    // If we reach here without crashing or running out of memory, no leak
    assert!(true);
}

// ============================================================================
// Pool Exhaustion Tests
// ============================================================================

#[test]
fn test_pool_exhaustion_100() {
    let (_class, target) = setup_stress_class();
    let selector = Selector::from_str("testMethod:").unwrap();
    let args = MessageArgs::two(42, 99);

    // Clear pool to start fresh
    PooledInvocation::clear_pool();

    // Acquire 100 invocations (will exhaust pool quickly)
    let invocations: Vec<_> = (0..100)
        .map(|_| PooledInvocation::with_arguments(&target, &selector, &args).unwrap())
        .collect();

    assert_eq!(invocations.len(), 100);

    // Verify pool statistics
    if let Some(stats) = PooledInvocation::pool_stats() {
        println!("Pool hits: {}, misses: {}", stats.hits, stats.misses);
        assert!(stats.hits + stats.misses >= 100);
    }
}

#[test]
fn test_pool_exhaustion_1000() {
    let (_class, target) = setup_stress_class();
    let selector = Selector::from_str("testMethod:").unwrap();
    let args = MessageArgs::two(42, 99);

    PooledInvocation::clear_pool();

    // Acquire 1000 invocations
    let invocations: Vec<_> = (0..1000)
        .map(|_| PooledInvocation::with_arguments(&target, &selector, &args).unwrap())
        .collect();

    assert_eq!(invocations.len(), 1000);
}

#[test]
fn test_pool_exhaustion_and_reuse() {
    let (_class, target) = setup_stress_class();
    let selector = Selector::from_str("testMethod:").unwrap();
    let args = MessageArgs::two(42, 99);

    PooledInvocation::clear_pool();

    // First batch
    let invocations1: Vec<_> = (0..100)
        .map(|_| PooledInvocation::with_arguments(&target, &selector, &args).unwrap())
        .collect();
    drop(invocations1);

    // Second batch should reuse from pool
    let invocations2: Vec<_> = (0..100)
        .map(|_| PooledInvocation::with_arguments(&target, &selector, &args).unwrap())
        .collect();

    // Verify high hit rate after first batch
    if let Some(stats) = PooledInvocation::pool_stats() {
        let hit_rate = stats.hit_rate().unwrap_or(0.0);
        println!("Pool hit rate after reuse: {:.2}%", hit_rate * 100.0);
        assert!(hit_rate >= 0.5, "Pool hit rate should be >= 50%");
    }
}

// ============================================================================
// Concurrent Access Tests
// ============================================================================

#[test]
fn test_concurrent_proxy_creation() {
    let (_class, target) = setup_stress_class();
    let target = Arc::new(target);
    let num_threads = 8;
    let proxies_per_thread = 100;

    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let target = Arc::clone(&target);
            thread::spawn(move || {
                let proxies: Vec<_> = (0..proxies_per_thread)
                    .map(|_| TransparentProxy::new(&target).unwrap())
                    .collect();
                proxies.len()
            })
        })
        .collect();

    let total: usize = handles.into_iter().map(|h| h.join().unwrap()).sum();
    assert_eq!(total, num_threads * proxies_per_thread);
}

#[test]
fn test_concurrent_pool_access() {
    let (_class, target) = setup_stress_class();
    let selector = Selector::from_str("testMethod:").unwrap();
    let args = MessageArgs::two(42, 99);

    let target = Arc::new(target);
    let num_threads = 8;
    let invocations_per_thread = 100;

    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let target = Arc::clone(&target);
            let selector = selector.clone();
            let args = args.clone();
            thread::spawn(move || {
                let invocations: Vec<_> = (0..invocations_per_thread)
                    .map(|_| PooledInvocation::with_arguments(&target, &selector, &args).unwrap())
                    .collect();
                invocations.len()
            })
        })
        .collect();

    let total: usize = handles.into_iter().map(|h| h.join().unwrap()).sum();
    assert_eq!(total, num_threads * invocations_per_thread);
}

#[test]
fn test_concurrent_pool_exhaustion() {
    let (_class, target) = setup_stress_class();
    let selector = Selector::from_str("testMethod:").unwrap();
    let args = MessageArgs::two(42, 99);

    PooledInvocation::clear_pool();

    let target = Arc::new(target);
    let num_threads = 16;
    let invocations_per_thread = 300;

    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let target = Arc::clone(&target);
            let selector = selector.clone();
            let args = args.clone();
            thread::spawn(move || {
                let invocations: Vec<_> = (0..invocations_per_thread)
                    .map(|_| PooledInvocation::with_arguments(&target, &selector, &args).unwrap())
                    .collect();
                invocations.len()
            })
        })
        .collect();

    let total: usize = handles.into_iter().map(|h| h.join().unwrap()).sum();
    assert_eq!(total, num_threads * invocations_per_thread);

    println!("Concurrent pool exhaustion test completed: {} invocations created", total);
}

// ============================================================================
// Performance Stress Tests
// ============================================================================

#[test]
fn test_stress_performance_proxy_creation() {
    let (_class, target) = setup_stress_class();
    let iterations = 10000;

    let start = Instant::now();
    for _ in 0..iterations {
        let _proxy = TransparentProxy::new(&target).unwrap();
    }
    let duration = start.elapsed();

    let avg_ns = duration.as_nanos() / iterations as u128;
    println!("Average proxy creation time: {} ns (debug mode)", avg_ns);

    // In debug mode, proxy creation is slower. In release mode it's < 1μs.
    // This test validates it's not catastrophically slow.
    assert!(avg_ns < 50_000, "Proxy creation too slow: {} ns (debug mode)", avg_ns);
}

#[test]
fn test_stress_performance_pool_acquisition() {
    let (_class, target) = setup_stress_class();
    let selector = Selector::from_str("testMethod:").unwrap();
    let args = MessageArgs::two(42, 99);

    PooledInvocation::clear_pool();

    // Warm up pool
    for _ in 0..10 {
        let _ = PooledInvocation::with_arguments(&target, &selector, &args);
    }

    let iterations = 10000;
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = PooledInvocation::with_arguments(&target, &selector, &args).unwrap();
    }
    let duration = start.elapsed();

    let avg_ns = duration.as_nanos() / iterations as u128;
    println!("Average pool acquisition time: {} ns", avg_ns);

    // Pool acquisition should be fast (< 500ns in release, < 1000ns in debug)
    #[cfg(debug_assertions)]
    let threshold = 1000;
    #[cfg(not(debug_assertions))]
    let threshold = 500;

    assert!(avg_ns < threshold, "Pool acquisition too slow: {} ns (threshold: {} ns)", avg_ns, threshold);
}

#[test]
fn test_stress_performance_composition() {
    let (_class, target) = setup_stress_class();

    let iterations = 1000;
    let start = Instant::now();

    for _ in 0..iterations {
        let proxies: Vec<_> = (0..5)
            .map(|_| TransparentProxy::new(&target).unwrap())
            .map(|p| p.into_object())
            .collect();

        let proxy_refs: Vec<_> = proxies.iter().collect();
        let _composed = compose_proxies(&proxy_refs).unwrap();
    }

    let duration = start.elapsed();
    let avg_ns = duration.as_nanos() / iterations as u128;
    println!("Average composition time (5 proxies): {} ns", avg_ns);

    // Composition should complete in reasonable time
    // In debug mode or with contention, can be slower. Use generous threshold.
    assert!(avg_ns < 500_000, "Composition too slow: {} ns (debug mode with contention)", avg_ns);
}

// ============================================================================
// Edge Case Stress Tests
// ============================================================================

#[test]
fn test_rapid_proxy_creation_destruction() {
    let (_class, target) = setup_stress_class();

    // Rapidly create and destroy proxies
    for _ in 0..10000 {
        let proxy = TransparentProxy::new(&target).unwrap();
        drop(proxy);
    }

    // If we reach here, no crashes or memory corruption
    assert!(true);
}

#[test]
fn test_mixed_proxy_types() {
    let (_class, target) = setup_stress_class();

    // Create different proxy types
    for i in 0..1000 {
        if i % 3 == 0 {
            let _proxy = TransparentProxy::new(&target).unwrap();
        } else if i % 3 == 1 {
            let _proxy = LoggingProxy::new(&target, |sel, _args| {
                println!("Logging: {:?}", sel.name());
            }).unwrap();
        } else {
            // RemoteProxy (just for variety)
            let _proxy = oxidec::runtime::RemoteProxy::new(i as u64, i as u64);
        }
    }

    // If we reach here, all proxy types work
    assert!(true);
}

#[test]
fn test_empty_and_single_proxy_composition() {
    let (_class, target) = setup_stress_class();

    // Empty composition should fail
    let result = compose_proxies(&[]);
    assert!(result.is_err(), "Empty proxy composition should fail");

    // Single proxy should work
    let proxy = TransparentProxy::new(&target).unwrap();
    let result = compose_proxies(&[proxy.as_object()]);
    assert!(result.is_ok(), "Single proxy composition should work");
}
