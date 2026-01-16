//! Token types and lexical tokens for the `OxideX` language.
//!
//! This module defines the `TokenKind` enum, which represents all possible
//! token types in the `OxideX` language, and the `Token` struct, which combines
//! a token kind with its source location (`Span`).
//!
//! # Examples
//!
//! ```
//! use oxidex_syntax::token::{Token, TokenKind};
//! use oxidex_syntax::span::Span;
//! use oxidex_mem::Symbol;
//!
//! // Create a keyword token
//! let let_token = Token::new(TokenKind::Let, Span::new(0, 3, 1, 1, 1, 4));
//!
//! // Create an identifier token with a Symbol
//! let ident_token = Token::new(
//!     TokenKind::Ident(Symbol::new(42)),
//!     Span::new(4, 5, 1, 5, 1, 6)
//! );
//! ```

use crate::span::{Span, Spanned};
use oxidex_mem::Symbol;
use std::fmt;

/// Represents the kind of a token.
///
/// Each variant corresponds to a specific lexical element in the `OxideX`
/// language, such as keywords, identifiers, literals, operators, and
/// delimiters.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TokenKind {
    // ===== Keywords =====
    /// Variable declaration (immutable)
    Let,

    /// Variable declaration (mutable)
    Mut,

    /// Function declaration
    Fn,

    /// Struct declaration
    Struct,

    /// Class declaration
    Class,

    /// Enum declaration
    Enum,

    /// Protocol declaration
    Protocol,

    /// Implementation block
    Impl,

    /// Return statement
    Return,

    /// If statement/expression
    If,

    /// Guard statement
    Guard,

    /// Match expression
    Match,

    /// For loop
    For,

    /// While loop
    While,

    /// Compile-time evaluation
    Comptime,

    /// Compile-time constant
    Const,

    /// Static variable
    Static,

    /// Public visibility
    Pub,

    /// Private visibility (file-private)
    Prv,

    // ===== Literals =====
    /// Identifier (variable name, function name, etc.)
    Ident(Symbol),

    /// Integer literal with optional type suffix
    ///
    /// Examples: `42`, `0xFF`, `0b1010`, `42u32`, `42i64`
    IntegerLiteral(Symbol, Option<Symbol>),

    /// Floating-point literal with optional type suffix
    ///
    /// Examples: `3.14`, `3.14f32`, `1e10`, `1.5e-5`
    FloatLiteral(Symbol, Option<Symbol>),

    /// String literal
    ///
    /// Examples: `"hello"`, `"world\n"`
    StringLiteral(Symbol),

    /// Boolean literal
    BoolLiteral(bool),

    /// Nil/null literal
    Nil,

    // ===== Operators =====
    /// Addition: `+`
    Plus,

    /// Subtraction: `-`
    Minus,

    /// Multiplication: `*`
    Star,

    /// Division: `/`
    Slash,

    /// Modulo: `%`
    Percent,

    /// Equality comparison: `==`
    EqEq,

    /// Inequality comparison: `!=`
    BangEq,

    /// Less than: `<`
    Lt,

    /// Greater than: `>`
    Gt,

    /// Less than or equal: `<=`
    LtEq,

    /// Greater than or equal: `>=`
    GtEq,

    /// Logical AND: `&&`
    AmpAmp,

    /// Logical OR: `||`
    PipePipe,

    /// Logical NOT: `!`
    Bang,

    /// Assignment: `=`
    Eq,

    // ===== Delimiters =====
    /// Left parenthesis: `(`
    LParen,

    /// Right parenthesis: `)`
    RParen,

    /// Left brace: `{`
    LBrace,

    /// Right brace: `}`
    RBrace,

    /// Left bracket: `[`
    LBracket,

    /// Right bracket: `]`
    RBracket,

    /// Dot: `.`
    Dot,

    /// Colon: `:`
    Colon,

    /// Double colon: `::`
    ColonColon,

    /// Comma: `,`
    Comma,

    /// Thin arrow: `->`
    Arrow,

    /// Fat arrow: `=>`
    FatArrow,

    // ===== Special =====
    /// Start of string interpolation: `\(`
    InterpolationStart,

    /// End of string interpolation: `)`
    InterpolationEnd,

    /// End of file
    EOF,
}

impl TokenKind {
    /// Returns `true` if this token is a keyword.
    ///
    /// # Examples
    ///
    /// ```
    /// use oxidex_syntax::token::TokenKind;
    /// use oxidex_mem::Symbol;
    ///
    /// assert!(TokenKind::Let.is_keyword());
    /// assert!(TokenKind::Fn.is_keyword());
    /// assert!(!TokenKind::Ident(Symbol::new(42)).is_keyword());
    /// ```
    #[must_use]
    pub const fn is_keyword(&self) -> bool {
        matches!(
            self,
            Self::Let
                | Self::Mut
                | Self::Fn
                | Self::Struct
                | Self::Class
                | Self::Enum
                | Self::Protocol
                | Self::Impl
                | Self::Return
                | Self::If
                | Self::Guard
                | Self::Match
                | Self::For
                | Self::While
                | Self::Comptime
                | Self::Const
                | Self::Static
                | Self::Pub
                | Self::Prv
        )
    }

    /// Returns `true` if this token is a literal.
    ///
    /// # Examples
    ///
    /// ```
    /// use oxidex_syntax::token::TokenKind;
    /// use oxidex_mem::Symbol;
    ///
    /// assert!(TokenKind::BoolLiteral(true).is_literal());
    /// assert!(TokenKind::IntegerLiteral(Symbol::new(42), None).is_literal());
    /// assert!(!TokenKind::Let.is_literal());
    /// ```
    #[must_use]
    pub const fn is_literal(&self) -> bool {
        matches!(
            self,
            Self::Ident(_) // TODO: Identifiers are not literals, fix this logic
                | Self::IntegerLiteral(_, _)
                | Self::FloatLiteral(_, _)
                | Self::StringLiteral(_)
                | Self::BoolLiteral(_)
                | Self::Nil
        )
    }

    /// Returns `true` if this token is an operator.
    ///
    /// # Examples
    ///
    /// ```
    /// use oxidex_syntax::token::TokenKind;
    ///
    /// assert!(TokenKind::Plus.is_operator());
    /// assert!(TokenKind::EqEq.is_operator());
    /// assert!(!TokenKind::Let.is_operator());
    /// ```
    #[must_use]
    pub const fn is_operator(&self) -> bool {
        matches!(
            self,
            Self::Plus
                | Self::Minus
                | Self::Star
                | Self::Slash
                | Self::Percent
                | Self::EqEq
                | Self::BangEq
                | Self::Lt
                | Self::Gt
                | Self::LtEq
                | Self::GtEq
                | Self::AmpAmp
                | Self::PipePipe
                | Self::Bang
                | Self::Eq
        )
    }

    /// Returns the precedence of binary operators, or `None` if not a binary operator.
    ///
    /// Higher values indicate higher precedence (tighter binding).
    ///
    /// # Examples
    ///
    /// ```
    /// use oxidex_syntax::token::TokenKind;
    ///
    /// assert_eq!(TokenKind::Star.precedence(), Some(7)); // Multiplication
    /// assert_eq!(TokenKind::Plus.precedence(), Some(6)); // Addition
    /// assert_eq!(TokenKind::EqEq.precedence(), Some(4)); // Equality
    /// assert_eq!(TokenKind::Let.precedence(), None); // Not an operator
    /// ```
    #[must_use]
    pub const fn precedence(&self) -> Option<u8> {
        match self {
            // Assignment (lowest precedence, right-associative)
            Self::Eq => Some(1),

            // Logical OR
            Self::PipePipe => Some(2),

            // Logical AND
            Self::AmpAmp => Some(3),

            // Equality
            Self::EqEq | Self::BangEq => Some(4),

            // Comparison
            Self::Lt | Self::Gt | Self::LtEq | Self::GtEq => Some(5),

            // Additive
            Self::Plus | Self::Minus => Some(6),

            // Multiplicative
            Self::Star | Self::Slash | Self::Percent => Some(7),

            // Not a binary operator
            _ => None,
        }
    }
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // Keywords
            Self::Let => write!(f, "let"),
            Self::Mut => write!(f, "mut"),
            Self::Fn => write!(f, "fn"),
            Self::Struct => write!(f, "struct"),
            Self::Class => write!(f, "class"),
            Self::Enum => write!(f, "enum"),
            Self::Protocol => write!(f, "protocol"),
            Self::Impl => write!(f, "impl"),
            Self::Return => write!(f, "return"),
            Self::If => write!(f, "if"),
            Self::Guard => write!(f, "guard"),
            Self::Match => write!(f, "match"),
            Self::For => write!(f, "for"),
            Self::While => write!(f, "while"),
            Self::Comptime => write!(f, "comptime"),
            Self::Const => write!(f, "const"),
            Self::Static => write!(f, "static"),
            Self::Pub => write!(f, "pub"),
            Self::Prv => write!(f, "prv"),

            // Literals
            Self::Ident(sym) => write!(f, "identifier(Symbol({}))", sym.as_u32()),
            Self::IntegerLiteral(val, suffix) => {
                write!(f, "integer(Symbol({}))", val.as_u32())?;
                if let Some(suf) = suffix {
                    write!(f, "_Symbol({})", suf.as_u32())
                } else {
                    Ok(())
                }
            }
            Self::FloatLiteral(val, suffix) => {
                write!(f, "float(Symbol({}))", val.as_u32())?;
                if let Some(suf) = suffix {
                    write!(f, "_Symbol({})", suf.as_u32())
                } else {
                    Ok(())
                }
            }
            Self::StringLiteral(sym) => write!(f, "string(Symbol({}))", sym.as_u32()),
            Self::BoolLiteral(b) => write!(f, "{b}"),
            Self::Nil => write!(f, "nil"),

            // Operators
            Self::Plus => write!(f, "+"),
            Self::Minus => write!(f, "-"),
            Self::Star => write!(f, "*"),
            Self::Slash => write!(f, "/"),
            Self::Percent => write!(f, "%"),
            Self::EqEq => write!(f, "=="),
            Self::BangEq => write!(f, "!="),
            Self::Lt => write!(f, "<"),
            Self::Gt => write!(f, ">"),
            Self::LtEq => write!(f, "<="),
            Self::GtEq => write!(f, ">="),
            Self::AmpAmp => write!(f, "&&"),
            Self::PipePipe => write!(f, "||"),
            Self::Bang => write!(f, "!"),
            Self::Eq => write!(f, "="),

            // Delimiters
            Self::LParen => write!(f, "("),
            Self::RParen | Self::InterpolationEnd => write!(f, ")"),
            Self::LBrace => write!(f, "{{"),
            Self::RBrace => write!(f, "}}"),
            Self::LBracket => write!(f, "["),
            Self::RBracket => write!(f, "]"),
            Self::Dot => write!(f, "."),
            Self::Colon => write!(f, ":"),
            Self::ColonColon => write!(f, "::"),
            Self::Comma => write!(f, ","),
            Self::Arrow => write!(f, "->"),
            Self::FatArrow => write!(f, "=>"),

            // Special
            Self::InterpolationStart => write!(f, "\\("),
            Self::EOF => write!(f, "EOF"),
        }
    }
}

/// A lexical token combining a token kind with its source location.
///
/// Tokens are produced by the lexer and consumed by the parser. Each token
/// carries a `Span` indicating its position in the source code, which is
/// essential for error reporting.
///
/// # Fields
///
/// - `kind`: The type of token
/// - `span`: The source location of the token
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Token {
    /// The type of token
    pub kind: TokenKind,

    /// The source location of the token
    pub span: Span,
}

impl Token {
    /// Creates a new token from a kind and span.
    ///
    /// # Arguments
    ///
    /// * `kind` - The type of token
    /// * `span` - The source location of the token
    ///
    /// # Examples
    ///
    /// ```
    /// use oxidex_syntax::token::{Token, TokenKind};
    /// use oxidex_syntax::span::Span;
    ///
    /// let token = Token::new(TokenKind::Let, Span::new(0, 3, 1, 1, 1, 4));
    /// ```
    #[must_use]
    pub const fn new(kind: TokenKind, span: Span) -> Self {
        Self { kind, span }
    }

    /// Returns `true` if this token is a keyword.
    ///
    /// Convenience method that delegates to `TokenKind::is_keyword`.
    #[must_use]
    pub const fn is_keyword(&self) -> bool {
        self.kind.is_keyword()
    }

    /// Returns `true` if this token is a literal.
    ///
    /// Convenience method that delegates to `TokenKind::is_literal`.
    #[must_use]
    pub const fn is_literal(&self) -> bool {
        self.kind.is_literal()
    }

    /// Returns `true` if this token is an operator.
    ///
    /// Convenience method that delegates to `TokenKind::is_operator`.
    #[must_use]
    pub const fn is_operator(&self) -> bool {
        self.kind.is_operator()
    }

    /// Returns the precedence of this token if it's a binary operator.
    ///
    /// Convenience method that delegates to `TokenKind::precedence`.
    #[must_use]
    pub const fn precedence(&self) -> Option<u8> {
        self.kind.precedence()
    }
}

impl Spanned for Token {
    fn span(&self) -> Span {
        self.span
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_kind_is_keyword() {
        assert!(TokenKind::Let.is_keyword());
        assert!(TokenKind::Fn.is_keyword());
        assert!(TokenKind::Return.is_keyword());
        assert!(!TokenKind::Ident(Symbol::new(42)).is_keyword());
        assert!(!TokenKind::Plus.is_keyword());
    }

    #[test]
    fn test_token_kind_is_operator() {
        assert!(TokenKind::Plus.is_operator());
        assert!(TokenKind::EqEq.is_operator());
        assert!(TokenKind::AmpAmp.is_operator());
        assert!(!TokenKind::Let.is_operator());
        assert!(!TokenKind::Ident(Symbol::new(42)).is_operator());
    }

    #[test]
    fn test_token_kind_precedence() {
        // Multiplicative (higher precedence)
        assert_eq!(TokenKind::Star.precedence(), Some(7));
        assert_eq!(TokenKind::Slash.precedence(), Some(7));

        // Additive (lower precedence)
        assert_eq!(TokenKind::Plus.precedence(), Some(6));
        assert_eq!(TokenKind::Minus.precedence(), Some(6));

        // Comparison (even lower)
        assert_eq!(TokenKind::EqEq.precedence(), Some(4));
        assert_eq!(TokenKind::Lt.precedence(), Some(5));

        // Assignment (lowest)
        assert_eq!(TokenKind::Eq.precedence(), Some(1));

        // Not an operator
        assert_eq!(TokenKind::Let.precedence(), None);
    }

    #[test]
    fn test_token_new() {
        let span = Span::new(0, 3, 1, 1, 1, 4);
        let token = Token::new(TokenKind::Let, span);

        assert!(matches!(token.kind, TokenKind::Let));
        assert_eq!(token.span, span);
    }

    #[test]
    fn test_token_helper_methods() {
        let span = Span::new(0, 3, 1, 1, 1, 4);
        let let_token = Token::new(TokenKind::Let, span);
        assert!(let_token.is_keyword());
        assert!(!let_token.is_operator());

        let plus_token = Token::new(TokenKind::Plus, span);
        assert!(!plus_token.is_keyword());
        assert!(plus_token.is_operator());
        assert_eq!(plus_token.precedence(), Some(6));
    }

    #[test]
    fn test_token_kind_display() {
        assert_eq!(format!("{}", TokenKind::Let), "let");
        assert_eq!(format!("{}", TokenKind::Plus), "+");
        assert_eq!(format!("{}", TokenKind::EqEq), "==");
        assert_eq!(format!("{}", TokenKind::BoolLiteral(true)), "true");
        assert_eq!(format!("{}", TokenKind::Nil), "nil");
    }
}
