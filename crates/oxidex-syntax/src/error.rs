//! Error types for the lexer and parser.
//!
//! This module defines the error types used throughout the `OxideX` frontend.
//! Lexer errors occur during tokenization, while parser errors occur during
//! syntactic analysis.

use crate::span::Span;
use std::fmt;

/// Errors that can occur during lexical analysis (tokenization).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LexerError {
    /// Unknown character encountered in source.
    ///
    /// # Examples
    ///
    /// ```text
    /// let x = @;
    ///         ^
    /// error: unknown character '@'
    /// ```
    UnknownChar {
        /// The unexpected character
        ch: char,

        /// Location in source
        span: Span,
    },

    /// Unterminated string literal.
    ///
    /// # Examples
    ///
    /// ```text
    /// let msg = "hello
    ///               ^-------^
    /// error: unterminated string literal
    /// ```
    UnterminatedString {
        /// Location where string started
        start: Span,
    },

    /// Invalid numeric literal.
    ///
    /// # Examples
    ///
    /// ```text
    /// let x = 123abc;
    ///          ^^^^^^^
    /// error: invalid numeric literal
    /// ```
    InvalidNumeric {
        /// The invalid literal text
        literal: String,

        /// Location in source
        span: Span,
    },

    /// Unterminated block comment.
    ///
    /// # Examples
    ///
    /// ```text
    /// let x = /* comment
    ///           ^-------^
    /// error: unterminated block comment
    /// ```
    UnterminatedComment {
        /// Location where comment started
        start: Span,
    },

    /// Unterminated string interpolation.
    ///
    /// Occurs when `\(expr` is not closed with `)`.
    UnterminatedInterpolation {
        /// Location where interpolation started
        start: Span,
    },
}

impl fmt::Display for LexerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownChar { ch, .. } => write!(f, "unknown character '{ch}'"),
            Self::UnterminatedString { .. } => write!(f, "unterminated string literal"),
            Self::InvalidNumeric { literal, .. } => {
                write!(f, "invalid numeric literal '{literal}'")
            }
            Self::UnterminatedComment { .. } => write!(f, "unterminated block comment"),
            Self::UnterminatedInterpolation { .. } => {
                write!(f, "unterminated string interpolation")
            }
        }
    }
}

impl std::error::Error for LexerError {}

/// Errors that can occur during parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParserError {
    /// Unexpected token encountered.
    ///
    /// # Examples
    ///
    /// ```text
    /// let = 42;
    ///     ^
    /// error: expected identifier, found '='
    /// ```
    UnexpectedToken {
        /// List of expected token kinds
        expected: Vec<String>,

        /// The actual token kind found
        found: String,

        /// Location in source
        span: Span,
    },

    /// Expected identifier but found another token.
    ExpectedIdentifier {
        /// Location in source
        span: Span,
    },

    /// Expected type but found another token.
    ExpectedType {
        /// Location in source
        span: Span,
    },

    /// Expected expression but found another token.
    ExpectedExpression {
        /// Location in source
        span: Span,
    },

    /// Expected statement but found another token.
    ExpectedStatement {
        /// Location in source
        span: Span,
    },

    /// Invalid pattern syntax.
    InvalidPattern {
        /// Description of the error
        message: String,

        /// Location in source
        span: Span,
    },

    /// Missing delimiter.
    MissingDelimiter {
        /// The delimiter that was expected (e.g., "}", ")", "]")
        delimiter: String,

        /// Location in source
        span: Span,
    },

    /// Invalid generic parameter syntax.
    InvalidGenericParams {
        /// Description of the error
        message: String,

        /// Location in source
        span: Span,
    },

    /// Invalid type annotation.
    InvalidTypeAnnotation {
        /// Description of the error
        message: String,

        /// Location in source
        span: Span,
    },

    /// Mismatched types in a context (e.g., match arms).
    MismatchedTypes {
        /// Expected type description
        expected: String,

        /// Found type description
        found: String,

        /// Location in source
        span: Span,
    },
}

impl fmt::Display for ParserError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedToken {
                expected, found, ..
            } => {
                write!(f, "expected ")?;
                if expected.len() == 1 {
                    write!(f, "{}", expected[0])?;
                } else {
                    write!(f, "one of: {}", expected.join(", "))?;
                }
                write!(f, ", found '{found}'")
            }
            Self::ExpectedIdentifier { .. } => write!(f, "expected identifier"),
            Self::ExpectedType { .. } => write!(f, "expected type"),
            Self::ExpectedExpression { .. } => write!(f, "expected expression"),
            Self::ExpectedStatement { .. } => write!(f, "expected statement"),
            Self::InvalidPattern { message, .. } => write!(f, "invalid pattern: {message}"),
            Self::MissingDelimiter { delimiter, .. } => {
                write!(f, "missing closing delimiter '{delimiter}'")
            }
            Self::InvalidGenericParams { message, .. } => {
                write!(f, "invalid generic parameters: {message}")
            }
            Self::InvalidTypeAnnotation { message, .. } => {
                write!(f, "invalid type annotation: {message}")
            }
            Self::MismatchedTypes {
                expected, found, ..
            } => {
                write!(f, "expected type {expected}, found type {found}")
            }
        }
    }
}

impl std::error::Error for ParserError {}

/// Combined syntax error for the frontend.
///
/// This type unifies lexer and parser errors into a single error type
/// that can be used throughout the syntax crate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyntaxError {
    /// Lexer error
    Lexer(LexerError),

    /// Parser error
    Parser(ParserError),
}

impl fmt::Display for SyntaxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lexer(err) => write!(f, "lexer error: {err}"),
            Self::Parser(err) => write!(f, "parser error: {err}"),
        }
    }
}

impl std::error::Error for SyntaxError {}

impl From<LexerError> for SyntaxError {
    fn from(err: LexerError) -> Self {
        Self::Lexer(err)
    }
}

impl From<ParserError> for SyntaxError {
    fn from(err: ParserError) -> Self {
        Self::Parser(err)
    }
}

/// Result type for lexer operations.
pub type LexerResult<T> = Result<T, LexerError>;

/// Result type for parser operations.
pub type ParserResult<T> = Result<T, ParserError>;

/// Result type for syntax operations (lexer or parser).
pub type SyntaxResult<T> = Result<T, SyntaxError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lexer_error_display() {
        let err = LexerError::UnknownChar {
            ch: '@',
            span: Span::new(10, 11, 1, 11, 1, 12),
        };
        assert_eq!(format!("{}", err), "unknown character '@'");

        let err = LexerError::UnterminatedString {
            start: Span::new(0, 1, 1, 1, 1, 2),
        };
        assert_eq!(format!("{}", err), "unterminated string literal");

        let err = LexerError::InvalidNumeric {
            literal: "123abc".to_string(),
            span: Span::new(0, 6, 1, 1, 1, 7),
        };
        assert_eq!(format!("{}", err), "invalid numeric literal '123abc'");
    }

    #[test]
    fn test_parser_error_display() {
        let err = ParserError::UnexpectedToken {
            expected: vec!["identifier".to_string()],
            found: "=".to_string(),
            span: Span::new(5, 6, 1, 6, 1, 7),
        };
        assert_eq!(format!("{}", err), "expected identifier, found '='");

        let err = ParserError::ExpectedIdentifier {
            span: Span::new(0, 1, 1, 1, 1, 2),
        };
        assert_eq!(format!("{}", err), "expected identifier");

        let err = ParserError::MissingDelimiter {
            delimiter: "}".to_string(),
            span: Span::new(0, 1, 1, 1, 1, 2),
        };
        assert_eq!(format!("{}", err), "missing closing delimiter '}'");
    }

    #[test]
    fn test_syntax_error_from() {
        let lexer_err = LexerError::UnknownChar {
            ch: '@',
            span: Span::new(0, 1, 1, 1, 1, 2),
        };
        let syntax_err: SyntaxError = lexer_err.into();
        assert!(matches!(syntax_err, SyntaxError::Lexer(_)));

        let parser_err = ParserError::ExpectedIdentifier {
            span: Span::new(0, 1, 1, 1, 1, 2),
        };
        let syntax_err: SyntaxError = parser_err.into();
        assert!(matches!(syntax_err, SyntaxError::Parser(_)));
    }

    #[test]
    fn test_syntax_error_display() {
        let lexer_err = LexerError::UnknownChar {
            ch: '@',
            span: Span::new(0, 1, 1, 1, 1, 2),
        };
        let syntax_err: SyntaxError = lexer_err.into();
        assert_eq!(format!("{}", syntax_err), "lexer error: unknown character '@'");
    }
}
