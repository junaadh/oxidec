//! String interning performance benchmarks.
//!
//! Measures the performance of string interning operations including:
//! - Interning new strings (hash insert)
//! - Interning duplicate strings (hash lookup)
//! - Symbol resolution (array indexing)
//! - Keyword pre-interning overhead

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use oxidex_mem::{StringInterner, Symbol};

fn bench_intern_new(c: &mut Criterion) {
    let mut group = c.benchmark_group("intern_new");

    for size in [10, 100, 1_000, 10_000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let strings: Vec<String> = (0..size).map(|i| format!("identifier_{}", i)).collect();

            b.iter(|| {
                let mut interner = StringInterner::new();
                for s in &strings {
                    black_box(interner.intern(s));
                }
            });
        });
    }

    group.finish();
}

fn bench_intern_duplicates(c: &mut Criterion) {
    let mut group = c.benchmark_group("intern_duplicates");

    for size in [10, 100, 1_000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let strings: Vec<String> = (0..size).map(|i| format!("identifier_{}", i)).collect();

            b.iter(|| {
                let mut interner = StringInterner::new();
                // First pass: intern all strings
                for s in &strings {
                    interner.intern(s);
                }
                // Second pass: intern duplicates (should be hash lookups only)
                for s in &strings {
                    black_box(interner.intern(s));
                }
            });
        });
    }

    group.finish();
}

fn bench_resolve(c: &mut Criterion) {
    let mut group = c.benchmark_group("resolve");

    for size in [10, 100, 1_000, 10_000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let mut interner = StringInterner::new();
            let symbols: Vec<Symbol> = (0..size)
                .map(|i| interner.intern(&format!("identifier_{}", i)))
                .collect();

            b.iter(|| {
                for &sym in &symbols {
                    black_box(interner.resolve(sym));
                }
            });
        });
    }

    group.finish();
}

fn bench_keyword_lookup(c: &mut Criterion) {
    c.bench_function("keyword_lookup", |b| {
        let mut interner = StringInterner::new();
        let keywords = ["let", "mut", "fn", "return", "if"];

        b.iter(|| {
            for keyword in &keywords {
                let sym = interner.intern(keyword);
                black_box(interner.resolve(sym));
            }
        });
    });
}

fn bench_mixed_workload(c: &mut Criterion) {
    let mut group = c.benchmark_group("mixed_workload");

    for size in [100, 1_000, 10_000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let strings: Vec<String> = (0..size)
                .map(|i| {
                    if i % 3 == 0 {
                        // Keywords (highly likely to be interned)
                        match i % 19 {
                            0 => "let",
                            1 => "mut",
                            2 => "fn",
                            3 => "struct",
                            4 => "return",
                            5 => "if",
                            6 => "else",
                            7 => "while",
                            8 => "for",
                            9 => "match",
                            10 => "enum",
                            11 => "impl",
                            12 => "class",
                            13 => "protocol",
                            14 => "comptime",
                            15 => "const",
                            16 => "static",
                            17 => "pub",
                            _ => "prv",
                        }
                        .to_string()
                    } else if i % 2 == 0 {
                        // Identifiers (moderate duplication)
                        format!("var_{}", i % 100)
                    } else {
                        // Unique identifiers
                        format!("unique_identifier_{}", i)
                    }
                })
                .collect();

            b.iter(|| {
                let mut interner = StringInterner::new();
                for s in &strings {
                    let sym = interner.intern(s);
                    if sym.as_u32() as i32 % 10 == 0 {
                        // Resolve 10% of symbols
                        black_box(interner.resolve(sym));
                    }
                }
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_intern_new,
    bench_intern_duplicates,
    bench_resolve,
    bench_keyword_lookup,
    bench_mixed_workload
);
criterion_main!(benches);
