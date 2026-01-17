//! Diagnostic and error reporting for the `OxideX` compiler.
//!
//! This module provides Rust-style error reporting with source highlighting,
//! error codes, and helpful suggestions.

use crate::{error::SyntaxError, span::Span, Spanned};
use oxidex_mem::StringInterner;
use std::fmt;

/// A diagnostic message (error, warning, note, or help).
///
/// Diagnostics provide rich, actionable error messages similar to Rust's compiler.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    /// Diagnostic level
    pub level: DiagnosticLevel,
    /// Error code (e.g., "E0001")
    pub code: Option<String>,
    /// Primary message
    pub message: String,
    /// Source span
    pub span: Span,
    /// Optional suggestions
    pub suggestions: Vec<String>,
    /// Related notes
    pub notes: Vec<DiagnosticNote>,
}

/// Severity level of a diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticLevel {
    /// Error: compilation cannot continue
    Error,
    /// Warning: suspicious code but compilation can continue
    Warning,
    /// Note: additional information
    Note,
    /// Help: suggestion for fixing the issue
    Help,
}

impl fmt::Display for DiagnosticLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Error => write!(f, "error"),
            Self::Warning => write!(f, "warning"),
            Self::Note => write!(f, "note"),
            Self::Help => write!(f, "help"),
        }
    }
}

impl DiagnosticLevel {
    /// Returns the ANSI color code for this level.
    #[must_use]
    pub const fn color_code(&self) -> &'static str {
        match self {
            Self::Error => "\x1b[31m",    // Red
            Self::Warning => "\x1b[33m",  // Yellow
            Self::Note => "\x1b[36m",     // Cyan
            Self::Help => "\x1b[32m",     // Green
        }
    }

    /// Returns the reset ANSI code.
    #[must_use]
    pub const fn reset_code() -> &'static str {
        "\x1b[0m"
    }

    /// Formats this level with colors if enabled.
    #[must_use]
    pub fn format_colored(&self, use_colors: bool) -> String {
        if use_colors {
            format!("{}{}{}", self.color_code(), self, Self::reset_code())
        } else {
            format!("{}", self)
        }
    }
}

/// A note attached to a diagnostic.
#[derive(Debug, Clone)]
pub struct DiagnosticNote {
    /// Note message
    pub message: String,
    /// Source span
    pub span: Span,
}

/// Builder for creating diagnostics.
pub struct DiagnosticBuilder {
    diagnostic: Diagnostic,
}

impl DiagnosticBuilder {
    /// Creates a new diagnostic builder.
    #[must_use] 
    pub fn new(level: DiagnosticLevel, message: String, span: Span) -> Self {
        Self {
            diagnostic: Diagnostic {
                level,
                code: None,
                message,
                span,
                suggestions: Vec::new(),
                notes: Vec::new(),
            },
        }
    }

    /// Adds an error code to the diagnostic.
    #[must_use] 
    pub fn code(mut self, code: String) -> Self {
        self.diagnostic.code = Some(code);
        self
    }

    /// Adds a suggestion to the diagnostic.
    #[must_use] 
    pub fn suggest(mut self, suggestion: String) -> Self {
        self.diagnostic.suggestions.push(suggestion);
        self
    }

    /// Adds a note to the diagnostic.
    #[must_use] 
    pub fn note(mut self, message: String, span: Span) -> Self {
        self.diagnostic.notes.push(DiagnosticNote { message, span });
        self
    }

    /// Builds the diagnostic.
    #[must_use] 
    pub fn build(self) -> Diagnostic {
        self.diagnostic
    }
}

/// Emitter for diagnostics.
///
/// Formats and prints diagnostics with source highlighting.
pub struct Emitter {
    /// String interner for resolving symbols to strings in diagnostics.
    /// TODO: Use for resolving Symbol identifiers in error messages
    #[allow(dead_code)]
    interner: StringInterner,
    /// Use colors in output
    use_colors: bool,
}

impl Emitter {
    /// Creates a new diagnostic emitter.
    #[must_use] 
    pub fn new(interner: StringInterner, use_colors: bool) -> Self {
        Self {
            interner,
            use_colors,
        }
    }

    /// Emits a diagnostic with source highlighting.
    pub fn emit(&self, diagnostic: &Diagnostic, source: &str) {
        let span = diagnostic.span;

        // Print primary error message with location and colored level
        let level_str = diagnostic.level.format_colored(self.use_colors);
        println!(
            "{}:{}:{}: {}",
            span.start_line,
            span.start_col,
            level_str,
            diagnostic.message
        );

        // Print error code if present
        if let Some(code) = &diagnostic.code {
            println!("   [{code}]");
        }

        // Print source highlighting
        self.emit_source_highlight(diagnostic.level, span, source);

        // Print suggestions
        for suggestion in &diagnostic.suggestions {
            let help_prefix = DiagnosticLevel::Help.format_colored(self.use_colors);
            println!("   {}: {}", help_prefix, suggestion);
        }

        // Print notes
        for note in &diagnostic.notes {
            let note_prefix = DiagnosticLevel::Note.format_colored(self.use_colors);
            println!(
                "   {} at {}:{}: {}",
                note_prefix,
                note.span.start_line,
                note.span.start_col,
                note.message
            );
        }
    }

    /// Emits source code highlighting for a span.
    fn emit_source_highlight(&self, level: DiagnosticLevel, span: Span, source: &str) {
        let lines: Vec<&str> = source.lines().collect();

        if lines.is_empty() {
            return;
        }

        let start_line = span.start_line.saturating_sub(1);
        let end_line = span.end_line.saturating_sub(1);

        // Clamp to valid range
        let start_line = start_line.min(lines.len() - 1);
        let end_line = end_line.min(lines.len() - 1);

        // Print each line with highlighting
        for line_idx in start_line..=end_line {
            let line_num = line_idx + 1;
            let line: &str = lines[line_idx];

            // Print line number and source
            println!("{line_num:4} | {line}");

            // Calculate highlight positions
            let line_start = if line_idx == start_line {
                span.start_col
            } else {
                1
            };

            let line_end = if line_idx == end_line {
                span.end_col
            } else {
                line.len() + 1
            };

            // Print underline with colors
            let indent = line_start.saturating_sub(1);
            let width = line_end.saturating_sub(line_start);

            if width > 0 {
                let underline = if self.use_colors {
                    // Use color based on diagnostic level
                    format!(
                        "{}{}{}",
                        " ".repeat(indent + 6), // 6 for "   | "
                        level.color_code(),
                        "^".repeat(width) + DiagnosticLevel::reset_code()
                    )
                } else {
                    format!(
                        "{}{}",
                        " ".repeat(indent + 6),
                        "^".repeat(width)
                    )
                };

                println!("     | {underline}");
            }
        }
    }

    /// Emits a syntax error as a diagnostic.
    pub fn emit_syntax_error(&self, error: &SyntaxError, source: &str) {
        let diagnostic = match error {
            SyntaxError::Lexer(err) => DiagnosticBuilder::new(
                DiagnosticLevel::Error,
                format!("{err}"),
                err.span(),
            ).build(),
            SyntaxError::Parser(err) => DiagnosticBuilder::new(
                DiagnosticLevel::Error,
                format!("{err}"),
                err.span(),
            ).build(),
        };

        self.emit(&diagnostic, source);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{keywords, error::ParserError};

    #[test]
    fn test_diagnostic_level_display() {
        assert_eq!(format!("{}", DiagnosticLevel::Error), "error");
        assert_eq!(format!("{}", DiagnosticLevel::Warning), "warning");
        assert_eq!(format!("{}", DiagnosticLevel::Note), "note");
        assert_eq!(format!("{}", DiagnosticLevel::Help), "help");
    }

    #[test]
    fn test_diagnostic_level_colors() {
        // Test with colors disabled
        assert_eq!(DiagnosticLevel::Error.format_colored(false), "error");
        assert_eq!(DiagnosticLevel::Warning.format_colored(false), "warning");

        // Test with colors enabled (should contain ANSI codes)
        let error_colored = DiagnosticLevel::Error.format_colored(true);
        assert!(error_colored.contains("\x1b[31m")); // Red
        assert!(error_colored.contains("error"));

        let warning_colored = DiagnosticLevel::Warning.format_colored(true);
        assert!(warning_colored.contains("\x1b[33m")); // Yellow
        assert!(warning_colored.contains("warning"));
    }

    #[test]
    fn test_diagnostic_builder() {
        let span = Span::new(0, 10, 1, 1, 1, 11);
        let diagnostic = DiagnosticBuilder::new(
            DiagnosticLevel::Error,
            "test error".to_string(),
            span,
        )
        .code("E0001".to_string())
        .suggest("try this instead".to_string())
        .build();

        assert!(matches!(diagnostic.level, DiagnosticLevel::Error));
        assert_eq!(diagnostic.code, Some("E0001".to_string()));
        assert_eq!(diagnostic.suggestions.len(), 1);
    }

    #[test]
    fn test_emitter_with_source() {
        let interner = StringInterner::with_pre_interned(keywords::KEYWORDS);
        let emitter = Emitter::new(interner, false); // No colors for test

        let source = "let x = 42;";
        let span = Span::new(4, 5, 1, 5, 1, 6); // The 'x'
        let diagnostic = DiagnosticBuilder::new(
            DiagnosticLevel::Error,
            "unexpected identifier".to_string(),
            span,
        )
        .build();

        // This should not panic
        emitter.emit(&diagnostic, source);
    }

    #[test]
    fn test_emit_syntax_error() {
        let interner = StringInterner::with_pre_interned(keywords::KEYWORDS);
        let emitter = Emitter::new(interner, false);

        let source = "let = 42;";
        let error = SyntaxError::Parser(ParserError::ExpectedIdentifier {
            span: Span::new(4, 4, 1, 5, 1, 5),
        });

        // This should not panic
        emitter.emit_syntax_error(&error, source);
    }

    #[test]
    fn test_diagnostic_with_notes() {
        let span = Span::new(0, 10, 1, 1, 1, 11);
        let diagnostic = DiagnosticBuilder::new(
            DiagnosticLevel::Warning,
            "unused variable".to_string(),
            span,
        )
        .note("consider prefixing with underscore".to_string(), Span::point(5, 1, 6))
        .build();

        assert_eq!(diagnostic.notes.len(), 1);
        assert_eq!(diagnostic.notes[0].message, "consider prefixing with underscore");
    }
}
