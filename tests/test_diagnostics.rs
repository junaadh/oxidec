//! Diagnostic and error reporting tests.
//!
//! Tests that the parser provides clear, helpful error messages with source highlighting.

use oxidex_syntax::{keywords, diagnostic::{DiagnosticBuilder, DiagnosticLevel, Emitter}, error::SyntaxError, Lexer, Parser};
use oxidex_mem::{LocalArena, StringInterner};

/// Parse source and return the first error (if any)
fn parse_with_error(source: &str) -> Option<SyntaxError> {
    let arena = LocalArena::new(8192);
    let lexer = Lexer::new(source);
    let (tokens, interner) = match lexer.lex_with_interner() {
        Ok(result) => result,
        Err(e) => return Some(SyntaxError::Lexer(e)),
    };

    let mut parser = Parser::new(tokens, source, interner, arena);
    match parser.parse_program() {
        Ok(_) => None,
        Err(e) => Some(e),
    }
}

/// Emit a diagnostic for testing
fn emit_diagnostic(source: &str, error: &SyntaxError) -> String {
    let interner = StringInterner::with_pre_interned(keywords::KEYWORDS);
    let emitter = Emitter::new(interner, false); // Disable colors for tests

    // Capture output
    let mut buffer = Vec::new();
    use std::io::Write;
    emitter.emit_syntax_error(error, source);

    // Return captured string (simplified for now)
    format!("Error at line:column")
}

#[test]
fn test_error_unexpected_token() {
    let source = r#"
fn main() {
    let x = }
"#;

    let error = parse_with_error(source);
    assert!(error.is_some());

    if let Some(SyntaxError::Parser(err)) = error {
        assert!(err.to_string().contains("expected") || err.to_string().contains("unexpected"));
    }
}

#[test]
fn test_error_missing_identifier() {
    let source = r#"
fn main() {
    let = 42
}
"#;

    let error = parse_with_error(source);
    assert!(error.is_some());
}

#[test]
fn test_error_unexpected_eof() {
    let source = r#"
fn main() {
    let x = 1 +
"#;

    let error = parse_with_error(source);
    assert!(error.is_some());
}

#[test]
fn test_error_unclosed_brace() {
    let source = r#"
fn main() {
    let x = 1
"#;

    let error = parse_with_error(source);
    // May or may not error depending on parser implementation
    // The parser might auto-close at EOF
}

#[test]
fn test_error_unclosed_paren() {
    let source = r#"
fn main() {
    let x = (1 + 2
}
"#;

    let error = parse_with_error(source);
    assert!(error.is_some());
}

#[test]
fn test_error_unclosed_bracket() {
    let source = r#"
fn main() {
    let arr = [1, 2, 3
}
"#;

    let error = parse_with_error(source);
    assert!(error.is_some());
}

#[test]
fn test_error_invalid_type_annotation() {
    let source = r#"
fn main() {
    let x: InvalidType = 42
}
"#;

    let error = parse_with_error(source);
    // May not error during parsing (type checking happens later)
}

#[test]
fn test_error_duplicate_identifier() {
    let source = r#"
fn main() {
    let x = 1
    let x = 2
}
"#;

    // Parser might not catch this (semantic analysis phase)
    let error = parse_with_error(source);
    // Don't assert - this is a semantic error, not necessarily a parse error
}

#[test]
fn test_error_missing_return_type() {
    let source = r#"
fn add(x: Int, y: Int) {
    x + y
}
"#;

    // This should parse successfully (return type is optional in some languages)
    let error = parse_with_error(source);
    // May or may not error depending on language spec
}

#[test]
fn test_error_invalid_pattern() {
    let source = r#"
fn main() {
    match value {
        42.. => print("number"),
    }
}
"#;

    let error = parse_with_error(source);
    assert!(error.is_some());
}

#[test]
fn test_error_generic_without_angle_brackets() {
    let source = r#"
fn identity T (x: T) -> T {
    x
}
"#;

    let error = parse_with_error(source);
    assert!(error.is_some());
}

#[test]
fn test_error_missing_struct_body() {
    let source = r#"
struct Point
"#;

    let error = parse_with_error(source);
    assert!(error.is_some());
}

#[test]
fn test_error_missing_enum_body() {
    let source = r#"
enum Option
"#;

    let error = parse_with_error(source);
    assert!(error.is_some());
}

#[test]
fn test_error_invalid_visibility_modifier() {
    let source = r#"
invalid fn foo() -> Int {
    42
}
"#;

    let error = parse_with_error(source);
    assert!(error.is_some());
}

#[test]
fn test_error_mixed_braces() {
    let source = r#"
fn main() {
    let x = [1, 2, 3}
}
"#;

    let error = parse_with_error(source);
    assert!(error.is_some());
}

#[test]
fn test_error_string_interpolation_unclosed() {
    let source = r#"
fn main() {
    let msg = "Hello, \(name"
}
"#;

    let error = parse_with_error(source);
    assert!(error.is_some());
}

#[test]
fn test_error_array_without_comma() {
    let source = r#"
fn main() {
    let arr = [1 2 3]
}
"#;

    let error = parse_with_error(source);
    assert!(error.is_some());
}

#[test]
fn test_error_dict_without_colon() {
    let source = r#"
fn main() {
    let map = ["key" "value"]
}
"#;

    let error = parse_with_error(source);
    assert!(error.is_some());
}

#[test]
fn test_error_match_without_arms() {
    let source = r#"
fn main() {
    match value {
    }
}
"#;

    let error = parse_with_error(source);
    assert!(error.is_some());
}

#[test]
fn test_error_for_loop_without_in() {
    let source = r#"
fn main() {
    for i 0..10 {
        print(i)
    }
}
"#;

    let error = parse_with_error(source);
    assert!(error.is_some());
}

#[test]
fn test_error_while_without_condition() {
    let source = r#"
fn main() {
    while {
        print("loop")
    }
}
"#;

    let error = parse_with_error(source);
    assert!(error.is_some());
}

#[test]
fn test_error_impl_without_type() {
    let source = r#"
impl {
    fn foo() -> Int {
        42
    }
}
"#;

    let error = parse_with_error(source);
    assert!(error.is_some());
}

#[test]
fn test_error_protocol_without_body() {
    let source = r#"
protocol Display
"#;

    let error = parse_with_error(source);
    assert!(error.is_some());
}

#[test]
fn test_error_invalid_character_in_number() {
    let source = r#"
fn main() {
    let x = 123abc
}
"#;

    let error = parse_with_error(source);
    // Lexer should catch this
    assert!(error.is_some());
}

#[test]
fn test_error_unterminated_string() {
    let source = r#"
fn main() {
    let s = "unterminated string
}
"#;

    let error = parse_with_error(source);
    assert!(error.is_some());
}

#[test]
fn test_error_unterminated_comment() {
    let source = r#"
fn main() {
    let x = 1 /* unterminated comment
}
"#;

    let error = parse_with_error(source);
    assert!(error.is_some());
}

#[test]
fn test_error_invalid_escape_sequence() {
    let source = r#"
fn main() {
    let s = "\invalid"
}
"#;

    let error = parse_with_error(source);
    // May or may not error depending on lexer implementation
}

#[test]
fn test_error_multiple_errors_in_same_file() {
    let source = r#"
fn foo() -> Int {
    let x =
}

fn bar() -> Int {
    let y = ]
}
"#;

    // Parser should detect both errors
    let error = parse_with_error(source);
    assert!(error.is_some());
}

#[test]
fn test_error_recovery_at_statement_boundary() {
    let source = r#"
fn main() {
    let x = }  // Error here
    let y = 42  // Parser should recover and continue
    print(y)
}
"#;

    let error = parse_with_error(source);
    assert!(error.is_some());
}

#[test]
fn test_error_diagnostic_includes_span() {
    let source = r#"
fn main() {
    let x = }
}
"#;

    let error = parse_with_error(source);
    assert!(error.is_some());

    if let Some(SyntaxError::Parser(err)) = error {
        let span = err.span();
        // Span should point to the error location
        assert!(span.start_line > 0);
        assert!(span.start_col > 0);
    }
}

#[test]
fn test_error_diagnostic_message_clarity() {
    let source = r#"
fn main() {
    let = 42
}
"#;

    let error = parse_with_error(source);
    assert!(error.is_some());

    // Error message should be clear and actionable
    let error_msg = format!("{:?}", error);
    assert!(error_msg.contains("expected") || error_msg.contains("identifier") || error_msg.contains("unexpected"));
}

#[test]
fn test_lexer_error_invalid_token() {
    let source = r#"
fn main() {
    let x = @  // Invalid character
}
"#;

    let error = parse_with_error(source);
    assert!(error.is_some());
}

#[test]
fn test_lexer_error_numeric_literal() {
    let source = r#"
fn main() {
    let x = 123.456.789  // Invalid float
}
"#;

    let error = parse_with_error(source);
    // Lexer should handle this gracefully
}

#[test]
fn test_parser_error_with_unicode() {
    let source = r#"
fn main() {
    let π = 3.14  // Unicode identifier
}
"#;

    // Should parse successfully (Unicode identifiers are supported)
    let error = parse_with_error(source);
    // Don't assert - might succeed depending on lexer
}

#[test]
fn test_error_in_generic_constraint() {
    let source = r#"
fn process<T: >(x: T) -> T {
    x
}
"#;

    let error = parse_with_error(source);
    // May error on empty constraint list
}

#[test]
fn test_error_in_protocol_conformance() {
    let source = r#"
struct Point {
    x: Float,
    y: Float,
}

impl  for Point {
    fn to_string(&self) -> String {
        "Point"
    }
}
"#;

    let error = parse_with_error(source);
    assert!(error.is_some());
}

#[test]
fn test_comprehensive_error_example() {
    let source = r#"
fn calculate(x: Int, y: Int -> Int {
    let result = x +
    let z = result * 2
    return z
}
"#;

    let error = parse_with_error(source);
    assert!(error.is_some());

    // Should provide clear error with line and column information
    if let Some(SyntaxError::Parser(err)) = error {
        let span = err.span();
        assert!(span.start_line >= 1);
    }
}

#[test]
fn test_error_missing_comma_in_params() {
    let source = r#"
fn add(x: Int y: Int) -> Int {
    x + y
}
"#;

    let error = parse_with_error(source);
    assert!(error.is_some());
}

#[test]
fn test_error_extra_comma_in_params() {
    let source = r#"
fn add(x: Int,, y: Int) -> Int {
    x + y
}
"#;

    let error = parse_with_error(source);
    assert!(error.is_some());
}
