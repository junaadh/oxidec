//! Statement nodes in the `OxideX` AST.
//!
//! Statements represent actions and declarations in `OxideX`. Unlike expressions,
//! statements do not produce values (except for the final expression in a block).

use crate::span::{Span, Spanned};
use oxidex_mem::Symbol;

/// A statement in the `OxideX` language.
///
/// Statements represent actions, declarations, and control flow constructs that
/// do not produce values (with the exception of expression statements).
///
/// # Lifetime
///
/// The `'arena` lifetime represents the arena allocator lifetime.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Stmt<'arena> {
    /// Let binding: `let x: Type = expr;`
    Let {
        /// Variable name
        name: Symbol,
        /// Optional type annotation
        type_annotation: Option<crate::ast::ty::Type>,
        /// Optional initializer
        init: Option<&'arena super::expr::Expr<'arena>>,
        /// Source location
        span: Span,
    },

    /// Mutable binding: `mut x: Type = expr;`
    Mut {
        /// Variable name
        name: Symbol,
        /// Optional type annotation
        type_annotation: Option<crate::ast::ty::Type>,
        /// Optional initializer
        init: Option<&'arena super::expr::Expr<'arena>>,
        /// Source location
        span: Span,
    },

    /// Return statement: `return expr;` or `return;`
    Return {
        /// Optional return value
        value: Option<&'arena super::expr::Expr<'arena>>,
        /// Source location
        span: Span,
    },

    /// If statement: `if cond { then } else { else }`
    If {
        /// Condition
        condition: &'arena super::expr::Expr<'arena>,
        /// Then branch
        then_branch: &'arena super::expr::Expr<'arena>,
        /// Optional else branch
        else_branch: Option<&'arena super::expr::Expr<'arena>>,
        /// Source location
        span: Span,
    },

    /// Guard statement: `guard condition else { block }`
    Guard {
        /// Condition
        condition: &'arena super::expr::Expr<'arena>,
        /// Else branch (executed if guard fails)
        else_branch: &'arena super::expr::Expr<'arena>,
        /// Source location
        span: Span,
    },

    /// Match statement: `match value { pattern => expr }`
    Match {
        /// Scrutinee
        scrutinee: &'arena super::expr::Expr<'arena>,
        /// Match arms
        arms: Vec<super::expr::MatchArm<'arena>>,
        /// Source location
        span: Span,
    },

    /// For loop: `for pattern in iter { body }`
    ForLoop {
        /// Loop pattern
        pattern: super::pat::Pattern,
        /// Iterator expression
        iter: &'arena super::expr::Expr<'arena>,
        /// Loop body
        body: &'arena super::expr::Expr<'arena>,
        /// Source location
        span: Span,
    },

    /// While loop: `while condition { body }`
    WhileLoop {
        /// Loop condition
        condition: &'arena super::expr::Expr<'arena>,
        /// Loop body
        body: &'arena super::expr::Expr<'arena>,
        /// Source location
        span: Span,
    },

    /// Assignment: `target = value;`
    Assign {
        /// Assignment target (lvalue)
        target: &'arena super::expr::Expr<'arena>,
        /// Value to assign
        value: &'arena super::expr::Expr<'arena>,
        /// Source location
        span: Span,
    },

    /// Expression statement: `expr;`
    Expr {
        /// The expression
        expr: &'arena super::expr::Expr<'arena>,
        /// Source location
        span: Span,
    },
}

impl Spanned for Stmt<'_> {
    fn span(&self) -> Span {
        match self {
            Self::Let { span, .. }
            | Self::Mut { span, .. }
            | Self::Return { span, .. }
            | Self::If { span, .. }
            | Self::Guard { span, .. }
            | Self::Match { span, .. }
            | Self::ForLoop { span, .. }
            | Self::WhileLoop { span, .. }
            | Self::Assign { span, .. }
            | Self::Expr { span, .. } => *span,
        }
    }
}
