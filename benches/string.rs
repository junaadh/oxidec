// RuntimeString benchmarks for OxideC runtime
//
// These benchmarks measure the performance of RuntimeString operations,
// including SSO (Small String Optimization), heap allocation, interning,
// cloning, comparison, and conversions.

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use oxidec::runtime::{RuntimeString, Arena};
use std::hash::Hash;

/// Benchmark inline string creation (SSO).
///
/// Tests performance of creating strings that fit in inline storage (≤15 bytes).
/// This should be very fast as it involves no heap allocation.
fn bench_inline_string_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("inline_creation");
    group.sample_size(1000); // Reduced to avoid excessive allocations

    for len in [0, 1, 4, 8, 15].iter() {
        let s = "a".repeat(*len);
        group.bench_with_input(BenchmarkId::from_parameter(len), len, |b, _len| {
            let arena = Arena::new(4096);
            b.iter(|| RuntimeString::new(black_box(&s), &arena));
        });
    }

    group.finish();
}

/// Benchmark heap string creation.
///
/// Tests performance of creating strings that require heap allocation (>15 bytes).
/// This measures arena allocation overhead.
fn bench_heap_string_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("heap_creation");
    group.sample_size(100); // Reduced to avoid filling the arena

    for len in [16, 32, 64, 256, 1024].iter() {
        let s = "a".repeat(*len);
        group.bench_with_input(BenchmarkId::from_parameter(len), len, |b, _len| {
            let arena = Arena::new(4096);
            b.iter(|| RuntimeString::new(black_box(&s), &arena));
        });
    }

    group.finish();
}

/// Benchmark string cloning.
///
/// Tests the performance of cloning RuntimeString for both inline and heap strings.
/// Inline strings copy by value; heap strings increment a refcount.
fn bench_string_clone(c: &mut Criterion) {
    let mut group = c.benchmark_group("clone");
    group.sample_size(10000);

    // Inline string clone (fast - just memcpy)
    group.bench_function("inline_string", |b| {
        let arena = Arena::new(4096);
        let rs = RuntimeString::new("short", &arena);
        b.iter(|| black_box(&rs).clone());
    });

    // Heap string clone (atomic refcount increment)
    group.bench_function("heap_string", |b| {
        let arena = Arena::new(4096);
        let rs = RuntimeString::new("This is a longer string that requires heap allocation", &arena);
        b.iter(|| black_box(&rs).clone());
    });

    group.finish();
}

/// Benchmark string comparison (PartialEq).
///
/// Tests performance of comparing RuntimeString for equality in various scenarios:
/// - Inline vs inline (fast byte comparison)
/// - Heap vs heap with same pointer (fastest - pointer equality)
/// - Heap vs heap with different pointers (slower - byte comparison)
fn bench_string_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("comparison");
    group.sample_size(10000);

    // Inline string comparison
    group.bench_function("inline_equal", |b| {
        let arena = Arena::new(4096);
        let rs1 = RuntimeString::new("hello", &arena);
        let rs2 = RuntimeString::new("hello", &arena);
        b.iter(|| black_box(&rs1) == black_box(&rs2));
    });

    // Heap string comparison (same allocation - pointer equality)
    group.bench_function("heap_same_pointer", |b| {
        let arena = Arena::new(4096);
        let rs1 = RuntimeString::new("This is a long string that requires heap allocation", &arena);
        let rs2 = rs1.clone();
        b.iter(|| black_box(&rs1) == black_box(&rs2));
    });

    // Heap string comparison (different allocations - byte comparison)
    group.bench_function("heap_content_compare", |b| {
        let arena = Arena::new(4096);
        let rs1 = RuntimeString::new("This is a long string that requires heap allocation", &arena);
        let rs2 = RuntimeString::new("This is a long string that requires heap allocation", &arena);
        b.iter(|| black_box(&rs1) == black_box(&rs2));
    });

    group.finish();
}

/// Benchmark string interning.
///
/// Tests the performance of string interning with cache hits and misses.
/// Cache hits should be very fast (just refcount increment).
fn bench_interning(c: &mut Criterion) {
    let mut group = c.benchmark_group("interning");
    group.sample_size(1000); // Reduced to avoid excessive allocations

    // Warm up the cache
    let _ = RuntimeString::intern("initWithObjects:andKeys:");

    // Cache hit - should be very fast
    group.bench_function("cache_hit", |b| {
        b.iter(|| RuntimeString::intern(black_box("initWithObjects:andKeys:")));
    });

    // Cache miss - needs allocation
    group.bench_function("cache_miss", |b| {
        // Use a limited set of strings to avoid unbounded memory growth
        let strings: Vec<String> = (0..100).map(|i| format!("uniqueString{}:", i)).collect();
        let mut counter = 0;
        b.iter(|| {
            let s = &strings[counter % strings.len()];
            counter += 1;
            // Force re-allocation by creating from bytes (bypasses interning cache)
            let arena = Arena::new(4096);
            RuntimeString::new(black_box(s), &arena)
        });
    });

    group.finish();
}

/// Benchmark string conversions.
///
/// Tests performance of converting RuntimeString to different representations:
/// - as_bytes() - returns byte slice
/// - as_str() - validates UTF-8 and returns &str
/// - to_string() - allocates a new Rust String
fn bench_conversions(c: &mut Criterion) {
    let mut group = c.benchmark_group("conversions");
    group.sample_size(1000); // Reduced to avoid excessive allocations

    // as_bytes() - should be fast (just slice extraction)
    group.bench_function("as_bytes_inline", |b| {
        let arena = Arena::new(4096);
        let rs = RuntimeString::new("short", &arena);
        b.iter(|| black_box(&rs).as_bytes());
    });

    group.bench_function("as_bytes_heap", |b| {
        let arena = Arena::new(4096);
        let rs = RuntimeString::new("This is a long string that requires heap allocation", &arena);
        b.iter(|| black_box(&rs).as_bytes());
    });

    // as_str() - includes UTF-8 validation
    group.bench_function("as_str_heap", |b| {
        let arena = Arena::new(4096);
        let rs = RuntimeString::new("This is a long string that requires heap allocation", &arena);
        b.iter(|| black_box(&rs).as_str().unwrap());
    });

    // to_string() - allocates new String
    group.bench_function("to_string_inline", |b| {
        let arena = Arena::new(4096);
        let rs = RuntimeString::new("short", &arena);
        b.iter(|| black_box(&rs).to_string());
    });

    group.bench_function("to_string_heap", |b| {
        let arena = Arena::new(4096);
        let rs = RuntimeString::new("This is a long string that requires heap allocation", &arena);
        b.iter(|| black_box(&rs).to_string());
    });

    group.finish();
}

/// Benchmark hash computation.
///
/// Tests performance of hashing RuntimeString.
/// Inline strings hash inline bytes; heap strings use cached hash.
fn bench_hash(c: &mut Criterion) {
    let mut group = c.benchmark_group("hash");
    group.sample_size(10000);

    group.bench_function("inline_hash", |b| {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::Hasher;

        let arena = Arena::new(4096);
        let rs = RuntimeString::new("short", &arena);
        let mut hasher = DefaultHasher::new();
        rs.hash(&mut hasher);
        let _hash = hasher.finish();

        b.iter(|| {
            let mut h = DefaultHasher::new();
            black_box(&rs).hash(&mut h);
            h.finish()
        });
    });

    group.bench_function("heap_hash", |b| {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::Hasher;

        let arena = Arena::new(4096);
        let rs = RuntimeString::new("This is a long string that requires heap allocation", &arena);
        let mut hasher = DefaultHasher::new();
        rs.hash(&mut hasher);
        let _hash = hasher.finish();

        b.iter(|| {
            let mut h = DefaultHasher::new();
            black_box(&rs).hash(&mut h);
            h.finish()
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_inline_string_creation,
    bench_heap_string_creation,
    bench_string_clone,
    bench_string_comparison,
    bench_interning,
    bench_conversions,
    bench_hash,
);
criterion_main!(benches);
