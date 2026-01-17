//! Expression nodes in the `OxideX` AST.
//!
//! Expressions represent values and computations in `OxideX`. All expressions
//! preserve source location information via [`Span`] for accurate error reporting.
//!
//! # Arena Allocation
//!
//! Expression nodes are allocated in arenas and referenced via `&'arena Expr<'arena>`.
//! This enables zero-overhead allocation and cache-friendly memory layout.

use crate::span::{Span, Spanned};
use oxidex_mem::Symbol;
use std::fmt;

/// An expression in the `OxideX` language.
///
/// Expressions represent values, computations, and control flow. All expression
/// variants carry span information for accurate error reporting.
///
/// # Lifetime
///
/// The `'arena` lifetime represents the arena allocator lifetime. Nested expressions
/// are arena-allocated and referenced as `&'arena Expr<'arena>`.
///
/// # Examples
///
/// ```
/// use oxidex_syntax::ast::Expr;
/// use oxidex_syntax::ast::expr::UnaryOp;
/// use oxidex_syntax::span::Span;
/// use oxidex_mem::Symbol;
///
/// // Create an integer literal expression
/// let int_expr = Expr::IntegerLiteral {
///     value: Symbol::new(42),
///     type_suffix: None,
///     span: Span::new(0, 2, 1, 1, 1, 3),
/// };
///
/// // Create a nil literal
/// let nil_expr = Expr::Nil {
///     span: Span::new(0, 3, 1, 1, 1, 4),
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Expr<'arena> {
    // ===== Literals =====

    /// Integer literal: `42`, `0xFF`, `0b1010`, `42u32`
    IntegerLiteral {
        /// The integer value (as interned string)
        value: Symbol,
        /// Optional type suffix (u8, i32, etc.)
        type_suffix: Option<Symbol>,
        /// Source location
        span: Span,
    },

    /// Floating-point literal: `3.14`, `1e10`, `3.14f32`
    FloatLiteral {
        /// The float value (as interned string)
        value: Symbol,
        /// Optional type suffix (f32, f64)
        type_suffix: Option<Symbol>,
        /// Source location
        span: Span,
    },

    /// String literal: `"hello"`
    StringLiteral {
        /// The string value (as interned string)
        value: Symbol,
        /// Source location
        span: Span,
    },

    /// Boolean literal: `true` or `false`
    BoolLiteral {
        /// The boolean value
        value: bool,
        /// Source location
        span: Span,
    },

    /// Nil literal: `nil`
    Nil {
        /// Source location
        span: Span,
    },

    // ===== Identifiers =====

    /// Simple identifier: `x`, `myVariable`
    Identifier(Symbol),

    /// Path expression: `Type::item`, `module::submodule::item`
    Path {
        /// Path segments
        segments: Vec<Symbol>,
        /// Source location
        span: Span,
    },

    // ===== Operators =====

    /// Unary operator expression: `-x`, `!flag`
    Unary {
        /// The operator
        op: UnaryOp,
        /// The operand
        operand: &'arena Expr<'arena>,
        /// Source location
        span: Span,
    },

    /// Binary operator expression: `a + b`, `x == y`
    Binary {
        /// Left operand
        left: &'arena Expr<'arena>,
        /// The operator
        op: BinaryOp,
        /// Right operand
        right: &'arena Expr<'arena>,
        /// Source location
        span: Span,
    },

    // ===== Control Flow =====

    /// If expression: `if cond { then } else { else }`
    If {
        /// Condition
        condition: &'arena Expr<'arena>,
        /// Then branch
        then_branch: &'arena Expr<'arena>,
        /// Else branch (required if used as expression)
        else_branch: Option<&'arena Expr<'arena>>,
        /// Source location
        span: Span,
    },

    /// Match expression: `match value { pattern => expr }`
    Match {
        /// Scrutinee (value being matched)
        scrutinee: &'arena Expr<'arena>,
        /// Match arms
        arms: Vec<MatchArm<'arena>>,
        /// Source location
        span: Span,
    },

    /// Block expression: `{ stmts; expr }`
    Block {
        /// Statements in the block
        stmts: Vec<crate::ast::stmt::Stmt<'arena>>,
        /// Optional final expression
        expr: Option<&'arena Expr<'arena>>,
        /// Source location
        span: Span,
    },

    /// For loop: `for pat in iter { body }`
    ForLoop {
        /// Loop pattern (e.g., `x`, `(key, value)`)
        pattern: crate::ast::pat::Pattern,
        /// Iterator expression
        iter: &'arena Expr<'arena>,
        /// Loop body
        body: &'arena Expr<'arena>,
        /// Source location
        span: Span,
    },

    /// While loop: `while cond { body }`
    WhileLoop {
        /// Loop condition
        condition: &'arena Expr<'arena>,
        /// Loop body
        body: &'arena Expr<'arena>,
        /// Source location
        span: Span,
    },

    // ===== Function Calls =====

    /// Function call: `foo(arg1, arg2)`
    Call {
        /// Function expression
        callee: &'arena Expr<'arena>,
        /// Arguments
        args: Vec<CallArg<'arena>>,
        /// Source location
        span: Span,
    },

    /// Method call: `obj.method(arg1, arg2)`
    MethodCall {
        /// Receiver object
        receiver: &'arena Expr<'arena>,
        /// Method name
        method: Symbol,
        /// Arguments
        args: Vec<CallArg<'arena>>,
        /// Source location
        span: Span,
    },

    // ===== Struct and Enum Construction =====

    /// Struct construction: `Point { x: 0, y: 0 }`
    Struct {
        /// Struct type (path)
        type_path: Vec<Symbol>,
        /// Field initializers
        fields: Vec<StructField<'arena>>,
        /// Source location
        span: Span,
    },

    /// Enum construction: `Option::Some(value)`
    Enum {
        /// Enum type (path)
        type_path: Vec<Symbol>,
        /// Variant name
        variant: Symbol,
        /// Optional payload
        payload: Option<&'arena Expr<'arena>>,
        /// Source location
        span: Span,
    },

    // ===== Collections =====

    /// Array literal: `[1, 2, 3]`
    Array {
        /// Elements
        elements: Vec<&'arena Expr<'arena>>,
        /// Source location
        span: Span,
    },

    /// Dictionary literal: `["key": value]`
    Dict {
        /// Key-value pairs
        entries: Vec<DictEntry<'arena>>,
        /// Source location
        span: Span,
    },

    // ===== Field and Index Access =====

    /// Field access: `obj.field`
    Field {
        /// Object expression
        object: &'arena Expr<'arena>,
        /// Field name
        field: Symbol,
        /// Source location
        span: Span,
    },

    /// Index access: `arr[index]`
    Index {
        /// Collection expression
        collection: &'arena Expr<'arena>,
        /// Index expression
        index: &'arena Expr<'arena>,
        /// Source location
        span: Span,
    },

    // ===== Special =====

    /// Parenthesized expression: `(expr)`
    Paren {
        /// Inner expression
        expr: &'arena Expr<'arena>,
        /// Source location
        span: Span,
    },

    /// String interpolation: `"Hello \(name)!"`
    Interpolation {
        /// String parts and interpolations
        parts: Vec<InterpolationPart<'arena>>,
        /// Source location
        span: Span,
    },
}

impl Spanned for Expr<'_> {
    fn span(&self) -> Span {
        match self {
            Self::IntegerLiteral { span, .. }
            | Self::FloatLiteral { span, .. }
            | Self::StringLiteral { span, .. }
            | Self::BoolLiteral { span, .. }
            | Self::Nil { span }
            | Self::Path { span, .. }
            | Self::Unary { span, .. }
            | Self::Binary { span, .. }
            | Self::If { span, .. }
            | Self::Match { span, .. }
            | Self::Block { span, .. }
            | Self::ForLoop { span, .. }
            | Self::WhileLoop { span, .. }
            | Self::Call { span, .. }
            | Self::MethodCall { span, .. }
            | Self::Struct { span, .. }
            | Self::Enum { span, .. }
            | Self::Array { span, .. }
            | Self::Dict { span, .. }
            | Self::Field { span, .. }
            | Self::Index { span, .. }
            | Self::Paren { span, .. }
            | Self::Interpolation { span, .. } => *span,
            Self::Identifier(_sym) => {
                // Identifier doesn't have a direct span - this is a limitation
                // In practice, we'd get the span from the original token
                Span::new(0, 0, 0, 0, 0, 0)
            }
        }
    }
}

/// Unary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnaryOp {
    /// Logical negation: `!`
    Negate,
    /// Arithmetic negation: `-`
    Minus,
}

impl fmt::Display for UnaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Negate => write!(f, "!"),
            Self::Minus => write!(f, "-"),
        }
    }
}

/// Binary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BinaryOp {
    /// Addition: `+`
    Add,
    /// Subtraction: `-`
    Sub,
    /// Multiplication: `*`
    Mul,
    /// Division: `/`
    Div,
    /// Modulo: `%`
    Mod,
    /// Equality: `==`
    Eq,
    /// Inequality: `!=`
    Neq,
    /// Less than: `<`
    Lt,
    /// Greater than: `>`
    Gt,
    /// Less than or equal: `<=`
    Lte,
    /// Greater than or equal: `>=`
    Gte,
    /// Logical AND: `&&`
    And,
    /// Logical OR: `||`
    Or,
    /// Assignment: `=`
    Assign,
}

impl fmt::Display for BinaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Add => write!(f, "+"),
            Self::Sub => write!(f, "-"),
            Self::Mul => write!(f, "*"),
            Self::Div => write!(f, "/"),
            Self::Mod => write!(f, "%"),
            Self::Eq => write!(f, "=="),
            Self::Neq => write!(f, "!="),
            Self::Lt => write!(f, "<"),
            Self::Gt => write!(f, ">"),
            Self::Lte => write!(f, "<="),
            Self::Gte => write!(f, ">="),
            Self::And => write!(f, "&&"),
            Self::Or => write!(f, "||"),
            Self::Assign => write!(f, "="),
        }
    }
}

/// A match arm in a match expression: `pattern => expr`
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MatchArm<'arena> {
    /// The pattern to match
    pub pattern: crate::ast::pat::Pattern,
    /// Optional guard condition: `if guard`
    pub guard: Option<&'arena Expr<'arena>>,
    /// The expression to execute if pattern matches
    pub body: &'arena Expr<'arena>,
    /// Source location
    pub span: Span,
}

/// A function call argument: `expr` or `label: expr`
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CallArg<'arena> {
    /// Optional label for named arguments
    pub label: Option<Symbol>,
    /// The argument expression
    pub value: &'arena Expr<'arena>,
    /// Source location
    pub span: Span,
}

/// A struct field initializer: `field: value` or `field` (shorthand)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StructField<'arena> {
    /// Field name
    pub name: Symbol,
    /// Field value (None for shorthand initialization)
    pub value: Option<&'arena Expr<'arena>>,
    /// Source location
    pub span: Span,
}

/// A dictionary entry: `key: value`
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DictEntry<'arena> {
    /// Key expression
    pub key: &'arena Expr<'arena>,
    /// Value expression
    pub value: &'arena Expr<'arena>,
    /// Source location
    pub span: Span,
}

/// A part of a string interpolation: either text or an expression
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum InterpolationPart<'arena> {
    /// Literal text
    Text(Symbol),
    /// Interpolated expression
    Expr(&'arena Expr<'arena>),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expr_span() {
        let span = Span::new(0, 2, 1, 1, 1, 3);
        let expr = Expr::IntegerLiteral {
            value: Symbol::new(42),
            type_suffix: None,
            span,
        };
        assert_eq!(expr.span(), span);
    }

    #[test]
    fn test_unary_op_display() {
        assert_eq!(format!("{}", UnaryOp::Negate), "!");
        assert_eq!(format!("{}", UnaryOp::Minus), "-");
    }

    #[test]
    fn test_binary_op_display() {
        assert_eq!(format!("{}", BinaryOp::Add), "+");
        assert_eq!(format!("{}", BinaryOp::Eq), "==");
        assert_eq!(format!("{}", BinaryOp::And), "&&");
    }
}
