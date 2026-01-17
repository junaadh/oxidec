use oxidex_syntax::diagnostic::{DiagnosticBuilder, DiagnosticLevel, Emitter};
use oxidex_syntax::error::SyntaxError;
use oxidex_syntax::keywords;
use oxidex_syntax::span::Span;

fn main() {
    let interner1 = oxidex_mem::StringInterner::with_pre_interned(keywords::KEYWORDS);
    let interner2 = oxidex_mem::StringInterner::with_pre_interned(keywords::KEYWORDS);
    let interner3 = oxidex_mem::StringInterner::with_pre_interned(keywords::KEYWORDS);

    // Test with colors
    println!("=== WITH COLORS ===\n");
    let emitter_colored = Emitter::new(interner1, true);

    let source = "let x: Int = 42;\nlet y = ;";
    let span = Span::new(19, 20, 2, 9, 2, 10);

    let diagnostic = DiagnosticBuilder::new(
        DiagnosticLevel::Error,
        "expected expression, found semicolon".to_string(),
        span,
    )
    .code("E0001".to_string())
    .suggest("add an expression after '='".to_string())
    .note("every value must have a type".to_string(), Span::new(16, 19, 2, 6, 2, 9))
    .build();

    emitter_colored.emit(&diagnostic, source);

    println!("\n\n=== WITHOUT COLORS ===\n");
    let emitter_plain = Emitter::new(interner2, false);
    emitter_plain.emit(&diagnostic, source);

    println!("\n\n=== SYNTAX ERROR ===\n");
    let error = SyntaxError::Parser(oxidex_syntax::error::ParserError::ExpectedIdentifier {
        span: Span::new(4, 5, 1, 5, 1, 6),
    });
    let emitter3 = Emitter::new(interner3, true);
    emitter3.emit_syntax_error(&error, "let = 42;");
}
