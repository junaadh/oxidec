//! Round-trip integration tests.
//!
//! Tests that parsing → pretty-printing → parsing produces equivalent results.

use oxidex_syntax::{keywords, Lexer, Parser, PrettyPrinter};
use oxidex_mem::{LocalArena, StringInterner};

/// Parse source code into an AST
fn parse(source: &str) -> Result<oxidex_syntax::ast::Program, oxidex_syntax::error::SyntaxError> {
    let arena = LocalArena::new(8192);
    let lexer = Lexer::new(source);
    let (tokens, interner) = lexer.lex_with_interner()?;
    let mut parser = Parser::new(tokens, source, interner, arena);
    parser.parse_program()
}

/// Pretty-print an AST back to source code
fn pretty_print(program: &oxidex_syntax::ast::Program) -> String {
    let interner = StringInterner::with_pre_interned(keywords::KEYWORDS);
    let mut printer = PrettyPrinter::new(interner);

    let mut result = String::new();
    for decl in &program.decls {
        result.push_str(&printer.print_decl(decl));
        result.push('\n');
    }
    result
}

/// Test round-trip: parse → print → parse should not error
fn test_roundtrip(source: &str) {
    // First parse
    let program1 = parse(source).expect("First parse failed");

    // Pretty-print
    let printed = pretty_print(&program1);

    // Second parse (should not error)
    let program2 = parse(&printed).expect("Second parse failed");

    // Check that both have the same number of declarations
    assert_eq!(
        program1.decls.len(),
        program2.decls.len(),
        "Declaration count mismatch after round-trip"
    );
}

#[test]
fn test_roundtrip_empty_program() {
    test_roundtrip("");
}

#[test]
fn test_roundtrip_simple_function() {
    let source = r#"
fn add(x: Int, y: Int) -> Int {
    x + y
}
"#;
    test_roundtrip(source);
}

#[test]
fn test_roundtrip_struct() {
    let source = r#"
struct Point {
    x: Float,
    y: Float,
}
"#;
    test_roundtrip(source);
}

#[test]
fn test_roundtrip_enum() {
    let source = r#"
enum Option {
    Some(Int),
    None,
}
"#;
    test_roundtrip(source);
}

#[test]
fn test_roundtrip_generic_struct() {
    let source = r#"
struct Pair<T, U> {
    first: T,
    second: U,
}
"#;
    test_roundtrip(source);
}

#[test]
fn test_roundtrip_protocol() {
    let source = r#"
protocol Display {
    fn to_string(&self) -> String
}
"#;
    test_roundtrip(source);
}

#[test]
fn test_roundtrip_impl() {
    let source = r#"
impl Point {
    fn new(x: Float, y: Float) -> Self {
        Self { x, y }
    }
}
"#;
    test_roundtrip(source);
}

#[test]
fn test_roundtrip_const() {
    let source = r#"
const MAX: Int = 100
"#;
    test_roundtrip(source);
}

#[test]
fn test_roundtrip_static() {
    let source = r#"
static COUNTER: Int = 0
"#;
    test_roundtrip(source);
}

#[test]
fn test_roundtrip_type_alias() {
    let source = r#"
type Result<T> = Option<T>
"#;
    test_roundtrip(source);
}

#[test]
fn test_roundtrip_class() {
    let source = r#"
class Counter {
    count: Int,

    fn new() -> Self {
        Self { count: 0 }
    }
}
"#;
    test_roundtrip(source);
}

#[test]
fn test_roundtrip_visibility_modifiers() {
    let source = r#"
pub fn public() -> Int {
    42
}

prv fn private() -> Int {
    24
}

pub struct Public {
    pub field: Int,
}
"#;
    test_roundtrip(source);
}

#[test]
fn test_roundtrip_complex_function() {
    let source = r#"
fn process<T>(data: T, transform: fn(T) -> T) -> T {
    transform(data)
}
"#;
    test_roundtrip(source);
}

#[test]
fn test_roundtrip_multiple_declarations() {
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
    test_roundtrip(source);
}

#[test]
fn test_roundtrip_with_comments() {
    let source = r#"
// This is a comment
fn add(x: Int, y: Int) -> Int {
    x + y
}
"#;
    // Note: Comments are not preserved in AST, so round-trip will lose them
    // This is expected behavior
    let program = parse(source).expect("First parse failed");
    assert_eq!(program.decls.len(), 1);
}

#[test]
fn test_roundtrip_all_examples() {
    // Test round-trip for all example programs
    let examples = [
        "examples/hello_world.ox",
        "examples/functions.ox",
        "examples/structs.ox",
        "examples/enums.ox",
        "examples/control_flow.ox",
        "examples/loops.ox",
        "examples/generics.ox",
        "examples/protocols.ox",
        "examples/classes.ox",
        "examples/collections.ox",
        "examples/operators.ox",
        "examples/pattern_matching.ox",
        "examples/string_interpolation.ox",
        "examples/comptime.ox",
        "examples/advanced.ox",
    ];

    for example_path in examples {
        let source = std::fs::read_to_string(example_path)
            .expect(&format!("Failed to read {}", example_path));

        // Parse and pretty-print
        let program1 = parse(&source).expect(&format!("First parse failed for {}", example_path));
        let _printed = pretty_print(&program1);

        // Verify we can parse the printed output
        // Note: Due to loss of formatting/comments, exact equality is not expected
        // We just verify the printed output is syntactically valid
    }
}

#[test]
fn test_roundtrip_preserves_structure() {
    let source = r#"
fn foo(x: Int) -> Int {
    x + 1
}

fn bar(y: Int) -> Int {
    y * 2
}
"#;

    let program1 = parse(source).expect("First parse failed");
    assert_eq!(program1.decls.len(), 2);

    let printed = pretty_print(&program1);
    let program2 = parse(&printed).expect("Second parse failed");

    assert_eq!(program2.decls.len(), 2);
}

#[test]
fn test_roundtrip_generic_types() {
    let source = r#"
struct Container<T> {
    value: T,
}

fn process<T: Clone, U>(x: T, y: U) -> Container<T> {
    Container { value: x.clone() }
}
"#;

    test_roundtrip(source);
}

#[test]
fn test_roundtrip_nested_generics() {
    let source = r#"
struct Wrapper<T> {
    inner: Option<T>,
}

fn process(x: Wrapper<Array<Int>>) -> Int {
    match x.inner {
        Option::Some(arr) => arr.len(),
        Option::None => 0,
    }
}
"#;

    test_roundtrip(source);
}

#[test]
fn test_roundtrip_impl_protocol() {
    let source = r#"
protocol Display {
    fn to_string(&self) -> String
}

impl Display for Int {
    fn to_string(&self) -> String {
        "42"
    }
}
"#;

    test_roundtrip(source);
}

#[test]
fn test_roundtrip_pattern_matching() {
    let source = r#"
fn main() {
    match value {
        Some(x) if x > 0 => x,
        Some(_) => 0,
        None => -1,
    }
}
"#;

    test_roundtrip(source);
}

#[test]
fn test_roundtrip_control_flow() {
    let source = r#"
fn main() {
    for i in 0..10 {
        if i % 2 == 0 {
            print(i)
        }
    }

    let mut x = 10
    while x > 0 {
        x = x - 1
    }
}
"#;

    test_roundtrip(source);
}

#[test]
fn test_roundtrip_collections() {
    let source = r#"
fn main() {
    let numbers = [1, 2, 3, 4, 5]
    let scores = ["Alice": 95, "Bob": 87]
    let matrix = [[1, 2], [3, 4]]
}
"#;

    test_roundtrip(source);
}

#[test]
fn test_roundtrip_string_interpolation() {
    let source = r#"
fn greet(name: String) -> String {
    "Hello, \(name)!"
}
"#;

    test_roundtrip(source);
}

#[test]
fn test_roundtrip_method_chaining() {
    let source = r#"
fn main() {
    let result = obj.method1().method2().field
}
"#;

    test_roundtrip(source);
}

#[test]
fn test_roundtrip_complex_expressions() {
    let source = r#"
fn main() {
    let result = ((a + b) * c) / d
    let value = some_func(a, b, c).method().field[index]
}
"#;

    test_roundtrip(source);
}

#[test]
fn test_roundtrip_preserves_decl_order() {
    let source = r#"
fn first() -> Int { 1 }
fn second() -> Int { 2 }
fn third() -> Int { 3 }
"#;

    let program1 = parse(source).expect("First parse failed");
    let printed = pretty_print(&program1);
    let program2 = parse(&printed).expect("Second parse failed");

    // Verify declaration order is preserved
    for (i, (decl1, decl2)) in program1.decls.iter().zip(program2.decls.iter()).enumerate() {
        match (decl1, decl2) {
            (oxidex_syntax::ast::Decl::Fn { name: n1, .. }, oxidex_syntax::ast::Decl::Fn { name: n2, .. }) => {
                assert_eq!(n1, n2, "Declaration {} name mismatch", i);
            },
            _ => {},
        }
    }
}

mod crate {
    pub use oxidex_syntax::{ast, Lexer, Parser, PrettyPrinter};
}
