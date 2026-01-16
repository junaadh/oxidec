//! `OxideX` Syntax: Lexer, Parser, and AST
//!
//! This crate provides the language frontend for `OxideX`, including:
//! - Lexical analysis (tokenization)
//! - Parsing (AST construction)
//! - AST node definitions
//! - Source location tracking
//!
//! # Phase 5: Language Frontend
//!
//! **Status:** In Progress (Foundation Complete)
//!
//! # Modules
//!
//! - [`span`] - Source location tracking
//! - [`token`] - Token types and definitions
//! - [`error`] - Lexer and parser error types
//!
//! # Examples
//!
//! ## Creating a Span
//!
//! ```
//! use oxidex_syntax::span::Span;
//!
//! let span = Span::new(0, 10, 1, 1, 1, 11);
//! ```
//!
//! ## Creating a Token
//!
//! ```
//! use oxidex_syntax::token::{Token, TokenKind};
//! use oxidex_syntax::span::Span;
//!
//! let token = Token::new(TokenKind::Let, Span::new(0, 3, 1, 1, 1, 4));
//! ```
//!
//! ## Using the Spanned Trait
//!
//! ```
//! use oxidex_syntax::span::{Span, Spanned};
//!
//! struct MyNode {
//!     span: Span,
//! }
//!
//! impl Spanned for MyNode {
//!     fn span(&self) -> Span {
//!         self.span
//!     }
//! }
//! ```

#![warn(missing_docs)]

// Public modules
pub mod span;
pub mod token;
pub mod error;
pub mod lexer;

// Re-exports for convenience
pub use span::{LineCol, Span, Spanned};
pub use token::{Token, TokenKind};
pub use error::{LexerError, ParserError, SyntaxError, LexerResult, ParserResult, SyntaxResult};
pub use lexer::Lexer;
