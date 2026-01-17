//! Pattern matching nodes in the `OxideX` AST.
//!
//! Patterns are used in match expressions, for loops, let bindings, and
//! function parameters to destructure values.

use crate::span::{Span, Spanned};
use crate::token::TokenKind;
use oxidex_mem::Symbol;

/// A pattern in the `OxideX` language.
///
/// Patterns are used to match and destructure values in contexts like:
/// - Match arms: `match value { pattern => expr }`
/// - For loops: `for (key, value) in map`
/// - Let bindings: `let (x, y) = point`
/// - Function parameters: `fn foo(Point { x, y }: Point)`

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Pattern {
    /// Wildcard pattern: `_`
    Wildcard {
        /// Source location
        span: Span,
    },

    /// Literal pattern: `42`, `"hello"`, `true`
    Literal {
        /// The literal value
        value: TokenKind,
        /// Source location
        span: Span,
    },

    /// Variable binding pattern: `x`, `mut x`
    Variable {
        /// Variable name
        name: Symbol,
        /// Is mutable
        mutable: bool,
        /// Source location
        span: Span,
    },

    /// Struct pattern: `Point { x, y }` or `Point { x: x0, y: y0 }`
    Struct {
        /// Struct type path
        type_path: Vec<Symbol>,
        /// Field patterns
        fields: Vec<FieldPat>,
        /// Source location
        span: Span,
    },

    /// Enum pattern: `Option::Some(x)` or `Option::None`
    Enum {
        /// Enum type path
        type_path: Vec<Symbol>,
        /// Variant name
        variant: Symbol,
        /// Optional nested pattern
        payload: Option<Box<Pattern>>,
        /// Source location
        span: Span,
    },

    /// Tuple pattern: `(x, y, z)`
    Tuple {
        /// Element patterns
        elements: Vec<Pattern>,
        /// Source location
        span: Span,
    },

    /// Array pattern: `[first, second, ..rest]`
    Array {
        /// Element patterns
        elements: Vec<Pattern>,
        /// Optional rest pattern (for remaining elements)
        rest: Option<Box<Pattern>>,
        /// Source location
        span: Span,
    },

    /// Or pattern: `pattern1 | pattern2`
    Or {
        /// Left pattern
        left: Box<Pattern>,
        /// Right pattern
        right: Box<Pattern>,
        /// Source location
        span: Span,
    },
}

impl Spanned for Pattern {
    fn span(&self) -> Span {
        match self {
            Self::Wildcard { span, .. }
            | Self::Literal { span, .. }
            | Self::Variable { span, .. }
            | Self::Struct { span, .. }
            | Self::Enum { span, .. }
            | Self::Tuple { span, .. }
            | Self::Array { span, .. }
            | Self::Or { span, .. } => *span,
        }
    }
}

/// A field pattern in a struct pattern.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FieldPat {
    /// Field name
    pub name: Symbol,
    /// Optional nested pattern (defaults to variable pattern with field name)
    pub pattern: Option<Box<Pattern>>,
    /// Source location
    pub span: Span,
}
