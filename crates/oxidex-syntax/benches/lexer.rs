// Lexer performance benchmarks for OxideX
//
// These benchmarks measure the lexer's performance on various inputs
// to ensure we meet the target of >100k LOC/sec.

use criterion::{black_box, Bencher, BenchmarkId, criterion_group, criterion_main, Criterion};
use oxidex_syntax::Lexer;

/// Lexes a simple expression with basic tokens.
fn bench_simple_expression(c: &mut Criterion) {
    let source = "let x = 42 + 10 * 5";

    c.bench_function("simple_expression", |b: &mut Bencher| {
        b.iter(|| {
            let lexer = Lexer::new(black_box(source));
            lexer.lex().unwrap()
        })
    });
}

/// Lexes a complex function with multiple statements.
fn bench_function(c: &mut Criterion) {
    let source = r#"
        fn calculateArea(width: Float64, height: Float64) -> Float64 {
            let pi = 3.1415926535
            let radius = width / 2.0
            let area = pi * radius * radius
            return area
        }
    "#;

    c.bench_function("function", |b: &mut Bencher| {
        b.iter(|| {
            let lexer = Lexer::new(black_box(source));
            lexer.lex().unwrap()
        })
    });
}

/// Lexes a struct definition with multiple fields.
fn bench_struct_definition(c: &mut Criterion) {
    let source = r#"
        struct Person {
            let name: String
            let age: Int32
            let email: String
            let address: String
            let phone: String
        }
    "#;

    c.bench_function("struct_definition", |b: &mut Bencher| {
        b.iter(|| {
            let lexer = Lexer::new(black_box(source));
            lexer.lex().unwrap()
        })
    });
}

/// Lexes a complex expression with many operators.
fn bench_complex_expression(c: &mut Criterion) {
    let source = "((a + b) * (c - d) / e) + (f * g - h / i) % j";

    c.bench_function("complex_expression", |b: &mut Bencher| {
        b.iter(|| {
            let lexer = Lexer::new(black_box(source));
            lexer.lex().unwrap()
        })
    });
}

/// Benchmarks lexing different sizes of input to measure throughput.
fn bench_input_size(c: &mut Criterion) {
    let mut group = c.benchmark_group("input_size");

    // Create sources of different sizes (in lines of code)
    let base_line = "let x: Int32 = 42 + 10 * 5\n";
    let sizes = vec![1, 10, 50, 100, 500, 1000];

    for size in sizes {
        let source = base_line.repeat(size);
        let line_count = source.lines().count();

        group.bench_with_input(BenchmarkId::from_parameter(line_count), &line_count, |b: &mut Bencher, _| {
            b.iter(|| {
                let lexer = Lexer::new(black_box(&source));
                lexer.lex().unwrap()
            })
        });
    }

    group.finish();
}

/// Benchmarks different numeric literal types.
fn bench_numeric_literals(c: &mut Criterion) {
    let mut group = c.benchmark_group("numeric_literals");

    // Decimal integers
    group.bench_function("decimal_integer", |b: &mut Bencher| {
        let source = "let x = 1234567890";
        b.iter(|| {
            let lexer = Lexer::new(black_box(source));
            lexer.lex().unwrap()
        })
    });

    // Hexadecimal
    group.bench_function("hexadecimal", |b: &mut Bencher| {
        let source = "let x = 0xFFFFFFFF";
        b.iter(|| {
            let lexer = Lexer::new(black_box(source));
            lexer.lex().unwrap()
        })
    });

    // Binary
    group.bench_function("binary", |b: &mut Bencher| {
        let source = "let x = 0b10101010";
        b.iter(|| {
            let lexer = Lexer::new(black_box(source));
            lexer.lex().unwrap()
        })
    });

    // Float with exponent
    group.bench_function("float_exponent", |b: &mut Bencher| {
        let source = "let x = 1.5e10";
        b.iter(|| {
            let lexer = Lexer::new(black_box(source));
            lexer.lex().unwrap()
        })
    });

    // Number with underscores
    group.bench_function("number_underscores", |b: &mut Bencher| {
        let source = "let x = 1_000_000_000";
        b.iter(|| {
            let lexer = Lexer::new(black_box(source));
            lexer.lex().unwrap()
        })
    });

    group.finish();
}

/// Benchmarks string literals with different content.
fn bench_string_literals(c: &mut Criterion) {
    let mut group = c.benchmark_group("string_literals");

    // Empty string
    group.bench_function("empty_string", |b: &mut Bencher| {
        let source = r#""""#;
        b.iter(|| {
            let lexer = Lexer::new(black_box(source));
            lexer.lex().unwrap()
        })
    });

    // Short string
    group.bench_function("short_string", |b: &mut Bencher| {
        let source = r#""hello world""#;
        b.iter(|| {
            let lexer = Lexer::new(black_box(source));
            lexer.lex().unwrap()
        })
    });

    // String with escapes
    group.bench_function("string_escapes", |b: &mut Bencher| {
        let source = r#""hello\nworld\ttab\""#;
        b.iter(|| {
            let lexer = Lexer::new(black_box(source));
            lexer.lex().unwrap()
        })
    });

    // String with interpolation
    group.bench_function("string_interpolation", |b: &mut Bencher| {
        let source = r#""Hello \(name)!"#;
        b.iter(|| {
            let lexer = Lexer::new(black_box(source));
            lexer.lex().unwrap()
        })
    });

    group.finish();
}

/// Benchmarks identifier recognition.
fn bench_identifiers(c: &mut Criterion) {
    let mut group = c.benchmark_group("identifiers");

    // Short identifier
    group.bench_function("short_identifier", |b: &mut Bencher| {
        let source = "let x = 42";
        b.iter(|| {
            let lexer = Lexer::new(black_box(source));
            lexer.lex().unwrap()
        })
    });

    // Long identifier
    group.bench_function("long_identifier", |b: &mut Bencher| {
        let source = "let this_is_a_very_long_identifier_name = 42";
        b.iter(|| {
            let lexer = Lexer::new(black_box(source));
            lexer.lex().unwrap()
        })
    });

    // Many identifiers
    group.bench_function("many_identifiers", |b: &mut Bencher| {
        let source = "let a = 1 let b = 2 let c = 3 let d = 4 let e = 5";
        b.iter(|| {
            let lexer = Lexer::new(black_box(source));
            lexer.lex().unwrap()
        })
    });

    group.finish();
}

/// Benchmarks comment handling.
fn bench_comments(c: &mut Criterion) {
    let mut group = c.benchmark_group("comments");

    // Line comment
    group.bench_function("line_comment", |b: &mut Bencher| {
        let source = "// This is a comment\nlet x = 42";
        b.iter(|| {
            let lexer = Lexer::new(black_box(source));
            lexer.lex().unwrap()
        })
    });

    // Block comment
    group.bench_function("block_comment", |b: &mut Bencher| {
        let source = "/* This is a block comment */\nlet x = 42";
        b.iter(|| {
            let lexer = Lexer::new(black_box(source));
            lexer.lex().unwrap()
        })
    });

    // Nested block comment
    group.bench_function("nested_comment", |b: &mut Bencher| {
        let source = "/* outer /* inner */ outer */\nlet x = 42";
        b.iter(|| {
            let lexer = Lexer::new(black_box(source));
            lexer.lex().unwrap()
        })
    });

    // Documentation comment
    group.bench_function("doc_comment", |b: &mut Bencher| {
        let source = "/// Documentation comment\nlet x = 42";
        b.iter(|| {
            let lexer = Lexer::new(black_box(source));
            lexer.lex().unwrap()
        })
    });

    group.finish();
}

/// Benchmarks keyword recognition.
fn bench_keywords(c: &mut Criterion) {
    let keywords = "let mut fn struct class enum protocol impl return if guard match for while comptime const static pub prv";

    c.bench_function("all_keywords", |b: &mut Bencher| {
        b.iter(|| {
            let lexer = Lexer::new(black_box(keywords));
            lexer.lex().unwrap()
        })
    });
}

/// Benchmarks a realistic piece of code with mixed content.
fn bench_realistic_code(c: &mut Criterion) {
    let source = r#"
        // Binary search tree implementation
        struct TreeNode<T> {
            let value: T
            let left: Option<TreeNode<T>>
            let right: Option<TreeNode<T>>
        }

        impl<T> TreeNode<T> {
            fn new(value: T) -> TreeNode<T> {
                return TreeNode {
                    value: value,
                    left: nil,
                    right: nil,
                }
            }

            fn insert(&mut self, value: T) {
                if value < self.value {
                    if self.left == nil {
                        self.left = TreeNode::new(value)
                    } else {
                        self.left.insert(value)
                    }
                } else {
                    if self.right == nil {
                        self.right = TreeNode::new(value)
                    } else {
                        self.right.insert(value)
                    }
                }
            }

            fn search(&self, value: T) -> Bool {
                if value == self.value {
                    return true
                } else if value < self.value {
                    if self.left == nil {
                        return false
                    } else {
                        return self.left.search(value)
                    }
                } else {
                    if self.right == nil {
                        return false
                    } else {
                        return self.right.search(value)
                    }
                }
            }
        }

        fn main() {
            let root = TreeNode::new(42)
            root.insert(10)
            root.insert(20)
            root.insert(30)
            root.insert(40)

            let found = root.search(20)
            if found {
                print("Found!")
            }
        }
    "#;

    c.bench_function("realistic_code", |b: &mut Bencher| {
        b.iter(|| {
            let lexer = Lexer::new(black_box(source));
            lexer.lex().unwrap()
        })
    });
}

criterion_group!(
    benches,
    bench_simple_expression,
    bench_function,
    bench_struct_definition,
    bench_complex_expression,
    bench_input_size,
    bench_numeric_literals,
    bench_string_literals,
    bench_identifiers,
    bench_comments,
    bench_keywords,
    bench_realistic_code,
);

criterion_main!(benches);
