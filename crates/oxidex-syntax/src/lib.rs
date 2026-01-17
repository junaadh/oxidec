//! `OxideX` Syntax: Lexer, Parser, and AST
//!
//! This crate provides the language frontend for `OxideX`, including:
//! - Lexical analysis (tokenization)
//! - Parsing (AST construction)
//! - AST node definitions
//! - Source location tracking
//! - Rich error reporting with diagnostics
//!
//! # Phase 5: Language Frontend
//!
//! **Status:** Complete (Lexer, Parser, AST, and Diagnostics)
//!
//! # Modules
//!
//! - [`span`] - Source location tracking
//! - [`token`] - Token types and definitions
//! - [`error`] - Lexer and parser error types
//! - [`lexer`] - Lexical analysis
//! - [`ast`] - Abstract Syntax Tree definitions
//! - [`parser`] - Recursive descent parser
//! - [`diagnostic`] - Error reporting with source highlighting
//! - [`pretty`] - AST pretty-printer
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
//!
//! ## Using Diagnostics for Error Reporting
//!
//! ```
//! use oxidex_syntax::diagnostic::{DiagnosticBuilder, DiagnosticLevel, Emitter};
//! use oxidex_syntax::keywords;
//! use oxidex_syntax::span::Span;
//!
//! // Create an emitter with colors enabled
//! let interner = oxidex_mem::StringInterner::with_pre_interned(keywords::KEYWORDS);
//! let emitter = Emitter::new(interner, true);
//!
//! // Build and emit a diagnostic
//! let span = Span::new(0, 10, 1, 1, 1, 11);
//! let diagnostic = DiagnosticBuilder::new(
//!     DiagnosticLevel::Error,
//!     "expected expression".to_string(),
//!     span,
//! )
//! .suggest("add an expression here".to_string())
//! .build();
//!
//! let source = "let x = ;";
//! emitter.emit(&diagnostic, source);
//! ```

#![warn(missing_docs)]

// Public modules
pub mod span;
pub mod keywords;
pub mod token;
pub mod error;
pub mod lexer;
pub mod ast;
pub mod parser;
pub mod diagnostic;
pub mod pretty;

// Re-exports for convenience
pub use span::{LineCol, Span, Spanned};
pub use token::{Token, TokenKind};
pub use error::{LexerError, ParserError, SyntaxError, LexerResult, ParserResult, SyntaxResult};
pub use lexer::Lexer;
pub use ast::{Expr, Stmt, Type, Pattern, Decl};
