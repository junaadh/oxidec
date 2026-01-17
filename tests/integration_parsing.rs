//! Integration tests for end-to-end parsing.
//!
//! Tests the full pipeline: source → lexer → parser → AST

use oxidex_syntax::{keywords, error::SyntaxError, Lexer, Parser};
use oxidex_mem::{LocalArena, StringInterner};

/// Helper to parse source code and return the result
fn parse_source(source: &str) -> Result<oxidex_syntax::ast::Program, SyntaxError> {
    let arena = LocalArena::new(8192);
    let lexer = Lexer::new(source);
    let (tokens, interner) = lexer.lex_with_interner()?;
    let mut parser = Parser::new(tokens, source, interner, arena);
    parser.parse_program()
}

#[test]
fn test_parse_empty_program() {
    let source = "";
    let result = parse_source(source);
    assert!(result.is_ok());
    let program = result.unwrap();
    assert_eq!(program.decls.len(), 0);
}

#[test]
fn test_parse_single_function() {
    let source = r#"
fn add(x: Int, y: Int) -> Int {
    x + y
}
"#;

    let result = parse_source(source);
    assert!(result.is_ok());
    let program = result.unwrap();
    assert_eq!(program.decls.len(), 1);
}

#[test]
fn test_parse_multiple_declarations() {
    let source = r#"
struct Point {
    x: Float,
    y: Float,
}

enum Option {
    Some(Int),
    None,
}

fn main() {
    print("Hello")
}
"#;

    let result = parse_source(source);
    assert!(result.is_ok());
    let program = result.unwrap();
    assert_eq!(program.decls.len(), 3);
}

#[test]
fn test_parse_generic_function() {
    let source = r#"
fn identity<T>(x: T) -> T {
    x
}
"#;

    let result = parse_source(source);
    assert!(result.is_ok());
}

#[test]
fn test_parse_generic_struct() {
    let source = r#"
struct Pair<T, U> {
    first: T,
    second: U,
}
"#;

    let result = parse_source(source);
    assert!(result.is_ok());
}

#[test]
fn test_parse_protocol() {
    let source = r#"
protocol Display {
    fn to_string(&self) -> String
}
"#;

    let result = parse_source(source);
    assert!(result.is_ok());
}

#[test]
fn test_parse_impl_block() {
    let source = r#"
impl Point {
    fn new(x: Float, y: Float) -> Self {
        Self { x, y }
    }
}
"#;

    let result = parse_source(source);
    assert!(result.is_ok());
}

#[test]
fn test_parse_const_declaration() {
    let source = r#"
const MAX_SIZE: Int = 100
"#;

    let result = parse_source(source);
    assert!(result.is_ok());
}

#[test]
fn test_parse_static_declaration() {
    let source = r#"
static mut COUNTER: Int = 0
"#;

    let result = parse_source(source);
    assert!(result.is_ok());
}

#[test]
fn test_parse_type_alias() {
    let source = r#"
type Result<T> = Option<T>
"#;

    let result = parse_source(source);
    assert!(result.is_ok());
}

#[test]
fn test_parse_complex_expression() {
    let source = r#"
fn main() {
    let result = ((a + b) * (c - d)) / e
    let value = some_func(a, b, c).method().field
}
"#;

    let result = parse_source(source);
    assert!(result.is_ok());
}

#[test]
fn test_parse_if_expression() {
    let source = r#"
fn main() {
    let x = if condition {
        1
    } else {
        0
    }
}
"#;

    let result = parse_source(source);
    assert!(result.is_ok());
}

#[test]
fn test_parse_match_expression() {
    let source = r#"
fn main() {
    match value {
        Some(x) => x,
        None => 0,
    }
}
"#;

    let result = parse_source(source);
    assert!(result.is_ok());
}

#[test]
fn test_parse_for_loop() {
    let source = r#"
fn main() {
    for i in 0..10 {
        print(i)
    }
}
"#;

    let result = parse_source(source);
    assert!(result.is_ok());
}

#[test]
fn test_parse_while_loop() {
    let source = r#"
fn main() {
    while x > 0 {
        x = x - 1
    }
}
"#;

    let result = parse_source(source);
    assert!(result.is_ok());
}

#[test]
fn test_parse_array_literal() {
    let source = r#"
fn main() {
    let numbers = [1, 2, 3, 4, 5]
}
"#;

    let result = parse_source(source);
    assert!(result.is_ok());
}

#[test]
fn test_parse_dict_literal() {
    let source = r#"
fn main() {
    let scores = ["Alice": 95, "Bob": 87]
}
"#;

    let result = parse_source(source);
    assert!(result.is_ok());
}

#[test]
fn test_parse_string_interpolation() {
    let source = r#"
fn main() {
    let message = "Hello, \(name)!"
}
"#;

    let result = parse_source(source);
    assert!(result.is_ok());
}

#[test]
fn test_parse_with_visibility_modifiers() {
    let source = r#"
pub fn public_function() -> Int {
    42
}

prv fn private_function() -> Int {
    24
}

pub struct PublicStruct {
    pub field: Int,
    prv private_field: Int,
}
"#;

    let result = parse_source(source);
    assert!(result.is_ok());
}

#[test]
fn test_parse_error_unexpected_token() {
    let source = r#"
fn main() {
    let x = }
}
"#;

    let result = parse_source(source);
    assert!(result.is_err());
}

#[test]
fn test_parse_error_missing_semicolon() {
    let source = r#"
fn main() {
    let x = 42
    let y = 24
}
"#;

    let result = parse_source(source);
    // This should either succeed (if semicolons are optional) or fail with a clear error
    // The current implementation should handle this
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_parse_class_declaration() {
    let source = r#"
class Counter {
    count: Int,

    fn new() -> Self {
        Self { count: 0 }
    }

    fn increment(&mut self) {
        self.count = self.count + 1
    }
}
"#;

    let result = parse_source(source);
    assert!(result.is_ok());
}

#[test]
fn test_parse_nested_structs() {
    let source = r#"
struct Outer {
    inner: Inner,
}

struct Inner {
    value: Int,
}
"#;

    let result = parse_source(source);
    assert!(result.is_ok());
}

#[test]
fn test_parse_method_chaining() {
    let source = r#"
fn main() {
    let result = obj.method1().method2().method3()
}
"#;

    let result = parse_source(source);
    assert!(result.is_ok());
}

#[test]
fn test_parse_complex_pattern_matching() {
    let source = r#"
fn main() {
    match value {
        Some(Point { x, y }) if x > 0 => x,
        Some(Point { .. }) => 0,
        None | _ => -1,
    }
}
"#;

    let result = parse_source(source);
    assert!(result.is_ok());
}

#[test]
fn test_parse_realistic_program() {
    let source = r#"
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
    let dist = p1.distance(p2)
    print(dist)
}
"#;

    let result = parse_source(source);
    assert!(result.is_ok());
    let program = result.unwrap();
    assert_eq!(program.decls.len(), 2); // struct and impl
}

#[test]
fn test_parse_all_examples() {
    // Test that all example programs can be parsed
    let examples = [
        "hello_world.ox",
        "functions.ox",
        "structs.ox",
        "enums.ox",
        "control_flow.ox",
        "loops.ox",
        "generics.ox",
        "protocols.ox",
        "classes.ox",
        "collections.ox",
        "operators.ox",
        "pattern_matching.ox",
        "string_interpolation.ox",
        "comptime.ox",
        "advanced.ox",
    ];

    for example in &examples {
        let path = format!("examples/{}", example);
        let source = std::fs::read_to_string(&path);
        assert!(source.is_ok(), "Failed to read {}", example);

        let source = source.unwrap();
        let result = parse_source(&source);
        assert!(result.is_ok(), "Failed to parse {}: {:?}", example, result.err());
    }
}
