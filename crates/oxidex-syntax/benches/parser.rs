//! Parser performance benchmarks for OxideX.
//!
//! These benchmarks measure the throughput of parsing various OxideX constructs.
//! Target: >50k LOC/sec according to RFC requirements.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use oxidex_syntax::{Lexer, Parser};
use oxidex_mem::LocalArena;

/// Benchmark parsing simple expressions
fn bench_simple_expressions(c: &mut Criterion) {
    let source = r#"
1 + 2 * 3
x + y * z
(a + b) * (c + d)
x.foo()
x.bar().baz()
!true
-42
"#;

    let mut group = c.benchmark_group("parse/simple_expressions");
    group.throughput(Throughput::Bytes(source.len() as u64));

    group.bench_function("simple", |b| {
        b.iter(|| {
            let arena = LocalArena::new(8192);
            let lexer = Lexer::new(black_box(source));
            let (tokens, interner) = lexer.lex_with_interner().unwrap();
            let mut parser = Parser::new(tokens, source, interner, arena);
            black_box(parser.parse_program())
        })
    });

    group.finish();
}

/// Benchmark parsing complex nested expressions
fn bench_complex_expressions(c: &mut Criterion) {
    let source = r#"
((a + b) * (c - d)) / ((e * f) + (g / h))
foo.bar().baz().qux()
some_func(a, b, c, d, e, f)
arr[0].field.method(arg1, arg2)
map["key"].value.method()
"#;

    let mut group = c.benchmark_group("parse/complex_expressions");
    group.throughput(Throughput::Bytes(source.len() as u64));

    group.bench_function("complex", |b| {
        b.iter(|| {
            let arena = LocalArena::new(8192);
            let lexer = Lexer::new(black_box(source));
            let (tokens, interner) = lexer.lex_with_interner().unwrap();
            let mut parser = Parser::new(tokens, source, interner, arena);
            black_box(parser.parse_program())
        })
    });

    group.finish();
}

/// Benchmark parsing function declarations
fn bench_functions(c: &mut Criterion) {
    let simple_fn = r#"
fn add(x: Int, y: Int) -> Int { x + y }
"#;

    let generic_fn = r#"
fn identity<T>(x: T) -> T { x }
"#;

    let complex_fn = r#"
fn process<T, U>(data: T, transform: fn(T) -> U) -> U {
    transform(data)
}
"#;

    let mut group = c.benchmark_group("parse/functions");

    for (name, source) in [
        ("simple", simple_fn),
        ("generic", generic_fn),
        ("complex", complex_fn),
    ] {
        group.throughput(Throughput::Bytes(source.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(name), source, |b, src| {
            b.iter(|| {
                let arena = LocalArena::new(8192);
                let lexer = Lexer::new(black_box(src));
                let (tokens, interner) = lexer.lex_with_interner().unwrap();
                let mut parser = Parser::new(tokens, src, interner, arena);
                black_box(parser.parse_program())
            })
        });
    }

    group.finish();
}

/// Benchmark parsing struct declarations
fn bench_structs(c: &mut Criterion) {
    let simple_struct = r#"
struct Point {
    x: Float,
    y: Float,
}
"#;

    let generic_struct = r#"
struct Result<T, E> {
    value: T,
    error: E,
}
"#;

    let complex_struct = r#"
struct HashMap<K: Hash, V: Clone> {
    data: Array<Entry<K, V>>,
    size: Int,
    capacity: Int,
}
"#;

    let mut group = c.benchmark_group("parse/structs");

    for (name, source) in [
        ("simple", simple_struct),
        ("generic", generic_struct),
        ("complex", complex_struct),
    ] {
        group.throughput(Throughput::Bytes(source.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(name), source, |b, src| {
            b.iter(|| {
                let arena = LocalArena::new(8192);
                let lexer = Lexer::new(black_box(src));
                let (tokens, interner) = lexer.lex_with_interner().unwrap();
                let mut parser = Parser::new(tokens, src, interner, arena);
                black_box(parser.parse_program())
            })
        });
    }

    group.finish();
}

/// Benchmark parsing enum declarations
fn bench_enums(c: &mut Criterion) {
    let simple_enum = r#"
enum Option {
    Some(Int),
    None,
}
"#;

    let generic_enum = r#"
enum Result<T, E> {
    Ok(T),
    Err(E),
}
"#;

    let complex_enum = r#"
enum Expr {
    Literal(Int),
    Variable(Symbol),
    Binary { op: BinaryOp, left: Box<Expr>, right: Box<Expr> },
    If { cond: Box<Expr>, then: Box<Expr>, else: Box<Expr> },
}
"#;

    let mut group = c.benchmark_group("parse/enums");

    for (name, source) in [
        ("simple", simple_enum),
        ("generic", generic_enum),
        ("complex", complex_enum),
    ] {
        group.throughput(Throughput::Bytes(source.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(name), source, |b, src| {
            b.iter(|| {
                let arena = LocalArena::new(8192);
                let lexer = Lexer::new(black_box(src));
                let (tokens, interner) = lexer.lex_with_interner().unwrap();
                let mut parser = Parser::new(tokens, src, interner, arena);
                black_box(parser.parse_program())
            })
        });
    }

    group.finish();
}

/// Benchmark parsing control flow statements
fn bench_control_flow(c: &mut Criterion) {
    let if_expr = r#"
if x > 0 {
    x
} else {
    -x
}
"#;

    let match_expr = r#"
match value {
    Some(x) => x,
    None => 0,
}
"#;

    let loop_expr = r#"
for i in 0..10 {
    print(i)
}
"#;

    let mut group = c.benchmark_group("parse/control_flow");

    for (name, source) in [("if", if_expr), ("match", match_expr), ("loop", loop_expr)] {
        group.throughput(Throughput::Bytes(source.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(name), source, |b, src| {
            b.iter(|| {
                let arena = LocalArena::new(8192);
                let lexer = Lexer::new(black_box(src));
                let (tokens, interner) = lexer.lex_with_interner().unwrap();
                let mut parser = Parser::new(tokens, src, interner, arena);
                black_box(parser.parse_program())
            })
        });
    }

    group.finish();
}

/// Benchmark parsing patterns
fn bench_patterns(c: &mut Criterion) {
    let simple_pattern = r#"
match x {
    0 => "zero",
    _ => "other",
}
"#;

    let struct_pattern = r#"
match point {
    Point { x, y } => x + y,
}
"#;

    let complex_pattern = r#"
match result {
    Ok(Counter { value: v }) if v > 0 => v,
    Ok(_) | Err(_) => 0,
}
"#;

    let mut group = c.benchmark_group("parse/patterns");

    for (name, source) in [
        ("simple", simple_pattern),
        ("struct", struct_pattern),
        ("complex", complex_pattern),
    ] {
        group.throughput(Throughput::Bytes(source.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(name), source, |b, src| {
            b.iter(|| {
                let arena = LocalArena::new(8192);
                let lexer = Lexer::new(black_box(src));
                let (tokens, interner) = lexer.lex_with_interner().unwrap();
                let mut parser = Parser::new(tokens, src, interner, arena);
                black_box(parser.parse_program())
            })
        });
    }

    group.finish();
}

/// Benchmark parsing generic types
fn bench_generics(c: &mut Criterion) {
    let simple_generic = r#"
fn identity<T>(x: T) -> T { x }
"#;

    let complex_generic = r#"
struct HashMap<K: Hash, V> {
    data: Array<Entry<K, V>>,
}

fn process<T: Clone, U: Display>(x: T, y: U) -> Result<T, U> {
    Ok(x.clone())
}
"#;

    let mut group = c.benchmark_group("parse/generics");

    for (name, source) in [("simple", simple_generic), ("complex", complex_generic)] {
        group.throughput(Throughput::Bytes(source.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(name), source, |b, src| {
            b.iter(|| {
                let arena = LocalArena::new(8192);
                let lexer = Lexer::new(black_box(src));
                let (tokens, interner) = lexer.lex_with_interner().unwrap();
                let mut parser = Parser::new(tokens, src, interner, arena);
                black_box(parser.parse_program())
            })
        });
    }

    group.finish();
}

/// Benchmark parsing a realistic program
fn bench_realistic_program(c: &mut Criterion) {
    let small_program = r#"
struct Point<T> {
    x: T,
    y: T,
}

impl Point<Float> {
    fn new(x: Float, y: Float) -> Self {
        Self { x, y }
    }

    fn distance(&self, other: Point<Float>) -> Float {
        let dx = self.x - other.x
        let dy = self.y - other.y
        ((dx * dx) + (dy * dy)).sqrt()
    }
}

fn main() {
    let p1 = Point::new(0.0, 0.0)
    let p2 = Point::new(3.0, 4.0)
    print(p1.distance(p2))
}
"#;

    let mut group = c.benchmark_group("parse/programs");
    group.throughput(Throughput::Bytes(small_program.len() as u64));

    group.bench_function("small", |b| {
        b.iter(|| {
            let arena = LocalArena::new(8192);
            let lexer = Lexer::new(black_box(small_program));
            let (tokens, interner) = lexer.lex_with_interner().unwrap();
            let mut parser = Parser::new(tokens, small_program, interner, arena);
            black_box(parser.parse_program())
        })
    });

    group.finish();
}

/// Benchmark parsing throughput in LOC/sec
fn bench_throughput(c: &mut Criterion) {
    let program = r#"
// A realistic program with various constructs
struct Counter {
    count: Int,
}

impl Counter {
    fn new() -> Self {
        Self { count: 0 }
    }

    fn increment(&mut self) {
        self.count = self.count + 1
    }

    fn get(&self) -> Int {
        self.count
    }
}

enum Option<T> {
    Some(T),
    None,
}

fn process_value(value: Int) -> Option<Int> {
    if value > 0 {
        Option::Some(value * 2)
    } else {
        Option::None
    }
}

fn main() {
    let mut counter = Counter::new()
    for i in 0..10 {
        counter.increment()
        match process_value(i) {
            Option::Some(v) => print(v),
            Option::None => print("zero"),
        }
    }
    print(counter.get())
}
"#;

    let lines = program.lines().count() as u64;

    let mut group = c.benchmark_group("parse/throughput");
    group.throughput(Throughput::Lines(lines));

    group.bench_function("loc", |b| {
        b.iter(|| {
            let arena = LocalArena::new(8192);
            let lexer = Lexer::new(black_box(program));
            let (tokens, interner) = lexer.lex_with_interner().unwrap();
            let mut parser = Parser::new(tokens, program, interner, arena);
            black_box(parser.parse_program())
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_simple_expressions,
    bench_complex_expressions,
    bench_functions,
    bench_structs,
    bench_enums,
    bench_control_flow,
    bench_patterns,
    bench_generics,
    bench_realistic_program,
    bench_throughput,
);

criterion_main!(benches);
