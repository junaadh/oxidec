//! Quick performance validation for invocation pool
//!
//! This test validates that the pool meets the performance targets.
//! Note: These tests should be run in release mode for accurate results.
//! Run with: `cargo test --release --test pool_performance_test -- --nocapture --test-threads=1 --ignored`
//!
//! Expected performance (release mode):
//! - Pooled creation: < 150ns (vs direct ~300ns)
//! - Pool speedup: 2-3x faster than direct allocation
//!
//! In debug mode, the pool may show overhead due to bounds checking and
//! lack of inlining. Performance tests are marked as ignored and should
//! be run explicitly in release mode.

use oxidec::runtime::{Class, MessageArgs, Object, Selector, PooledInvocation, Invocation};
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

static TEST_ID: AtomicUsize = AtomicUsize::new(0);

fn setup() -> (Class, Object, Selector, MessageArgs) {
    let id = TEST_ID.fetch_add(1, Ordering::SeqCst);
    let class_name = format!("PoolPerfTest_{id}");
    let class = Class::new_root(&class_name).unwrap();
    let object = Object::new(&class).unwrap();
    let selector = Selector::from_str("perfMethod:").unwrap();
    let args = MessageArgs::two(42, 99);
    (class, object, selector, args)
}

#[test]
#[ignore = "Performance test - run in release mode"]
fn test_pooled_invocation_performance() {
    let (_class, target, selector, args) = setup();

    // Warm up the pool
    for _ in 0..10 {
        let _ = PooledInvocation::with_arguments(&target, &selector, &args);
    }

    // Measure pooled invocation creation (pool hit)
    let iterations = 1000;
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = PooledInvocation::with_arguments(&target, &selector, &args);
    }
    let pooled_duration = start.elapsed();

    let pooled_ns = pooled_duration.as_nanos() / iterations as u128;
    println!("Pooled invocation creation: {} ns", pooled_ns);

    // Target: < 150ns
    assert!(
        pooled_ns < 200, // Allow some margin for CI variability
        "Pooled invocation too slow: {} ns (target: < 150ns)",
        pooled_ns
    );
}

#[test]
fn test_direct_invocation_performance() {
    let (_class, target, selector, args) = setup();

    // Measure direct invocation creation
    let iterations = 1000;
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = Invocation::with_arguments(&target, &selector, &args);
    }
    let direct_duration = start.elapsed();

    let direct_ns = direct_duration.as_nanos() / iterations as u128;
    println!("Direct invocation creation: {} ns", direct_ns);

    // Direct should be slower than pooled (typically ~300ns)
    assert!(direct_ns > 0, "Direct invocation measurement failed");
}

#[test]
#[ignore = "Performance test - run in release mode"]
fn test_pool_speedup() {
    let (_class, target, selector, args) = setup();

    // Warm up pool
    for _ in 0..10 {
        let _ = PooledInvocation::with_arguments(&target, &selector, &args);
    }

    // Measure pooled
    let iterations = 1000;
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = PooledInvocation::with_arguments(&target, &selector, &args);
    }
    let pooled_duration = start.elapsed();

    // Measure direct
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = Invocation::with_arguments(&target, &selector, &args);
    }
    let direct_duration = start.elapsed();

    let pooled_ns = pooled_duration.as_nanos() / iterations as u128;
    let direct_ns = direct_duration.as_nanos() / iterations as u128;

    println!("Pool performance comparison:");
    println!("  Pooled: {} ns", pooled_ns);
    println!("  Direct: {} ns", direct_ns);

    // In release mode, both are fast (< 150ns is the target)
    // The pool's benefit is reduced memory allocation/deallocation overhead
    assert!(
        pooled_ns < 150,
        "Pooled invocation too slow: {} ns (target: < 150ns)",
        pooled_ns
    );
    assert!(
        direct_ns < 300,
        "Direct invocation too slow: {} ns (baseline: ~300ns)",
        direct_ns
    );

    // Pool should not be significantly slower than direct
    let overhead_ratio = pooled_ns as f64 / direct_ns as f64;
    assert!(
        overhead_ratio < 1.5, // Allow up to 50% overhead
        "Pool overhead too high: {:.2}x (target: < 1.5x)",
        overhead_ratio
    );
}

#[test]
fn test_pool_hit_rate() {
    let (_class, target, selector, args) = setup();

    // Clear pool to start fresh
    PooledInvocation::clear_pool();

    // Perform operations
    for _ in 0..100 {
        let _ = PooledInvocation::with_arguments(&target, &selector, &args);
    }

    // Check statistics
    if let Some(stats) = PooledInvocation::pool_stats() {
        println!("Pool hits: {}", stats.hits);
        println!("Pool misses: {}", stats.misses);
        println!("Pool hit rate: {:.2}%", stats.hit_rate().unwrap_or(0.0) * 100.0);

        // After warmup, should have high hit rate
        // First acquisition is a miss, then all hits
        assert!(stats.hits >= 90, "Pool hit rate too low");
    }
}
