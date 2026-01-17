//! Type checking errors.
//!
//! This module defines all error types that can occur during type checking,
//! with support for rich error reporting and suggestions.

use crate::types::Ty;
use oxidex_syntax::Span;
use std::fmt;

/// Type checking errors.
#[derive(Debug, Clone)]
pub enum TypeError {
    /// Type mismatch between expected and found types.
    Mismatch {
        /// The expected type
        expected: Ty,
        /// The actual type found
        found: Ty,
        /// Source location of the error
        span: Span,
    },

    /// Undefined variable.
    UndefinedVar {
        /// Name of the undefined variable
        name: String,
        /// Source location
        span: Span,
    },

    /// Undefined type.
    UndefinedType {
        /// Name of the undefined type
        name: String,
        /// Source location
        span: Span,
    },

    /// Undefined function.
    UndefinedFunction {
        /// Name of the undefined function
        name: String,
        /// Functions with similar names (for suggestions)
        candidates: Vec<String>,
        /// Source location
        span: Span,
    },

    /// Non-exhaustive match expression.
    NonExhaustiveMatch {
        /// Missing patterns/variants
        missing: Vec<String>,
        /// Source location (of the match expression)
        span: Span,
    },

    /// Infinite type (occurs check failure).
    InfiniteType {
        /// Source location
        span: Span,
    },

    /// Missing protocol method.
    MissingProtocolMethod {
        /// The type that should implement the method
        ty: String,
        /// The protocol requiring the method
        protocol: String,
        /// The missing method name
        method: String,
        /// Source location
        span: Span,
    },

    /// Assignment to immutable binding.
    AssignToImmutable {
        /// Name of the immutable binding
        name: String,
        /// Source location
        span: Span,
    },

    /// Wrong number of type arguments.
    WrongTypeArgCount {
        /// Name of the type/constructor
        name: String,
        /// Expected number of arguments
        expected: usize,
        /// Actual number of arguments
        found: usize,
        /// Source location
        span: Span,
    },

    /// Protocol constraint not satisfied.
    ProtocolConstraintNotSatisfied {
        /// The type that doesn't satisfy the constraint
        ty: String,
        /// The required protocol
        protocol: String,
        /// Source location
        span: Span,
    },

    /// Recursive type without indirection.
    RecursiveType {
        /// Name of the type
        name: String,
        /// Source location
        span: Span,
    },

    /// Ambiguous type (needs annotation).
    AmbiguousType {
        /// Source location
        span: Span,
    },

    /// Match on non-enum type.
    MatchOnNonEnum {
        /// The type being matched on
        ty: String,
        /// Source location
        span: Span,
    },

    /// Field access on non-struct/class type.
    FieldAccessOnNonStruct {
        /// The type being accessed
        ty: String,
        /// The field name
        field: String,
        /// Source location
        span: Span,
    },

    /// Invalid assignment target.
    InvalidAssignmentTarget {
        /// Source location
        span: Span,
    },

    /// Missing else branch.
    MissingElse {
        /// Source location of the if expression
        span: Span,
    },

    /// Non-boolean condition.
    NonBooleanCondition {
        /// The actual type of the condition
        found: Ty,
        /// Source location
        span: Span,
    },

    /// Break/continue outside loop.
    BreakOutsideLoop {
        /// Source location
        span: Span,
    },

    /// Return outside function.
    ReturnOutsideFunction {
        /// Source location
        span: Span,
    },

    /// Invalid return type.
    InvalidReturnType {
        /// Expected return type
        expected: Ty,
        /// Actual return type
        found: Ty,
        /// Source location
        span: Span,
    },

    /// Unknown type.
    UnknownType {
        /// Name of the unknown type
        name: String,
        /// Source location
        span: Span,
    },

    /// Unknown field.
    UnknownField {
        /// The type being accessed
        ty: String,
        /// The unknown field name
        field: String,
        /// Source location
        span: Span,
    },

    /// Unknown enum variant.
    UnknownVariant {
        /// The enum type
        ty: String,
        /// The unknown variant name
        variant: String,
        /// Source location
        span: Span,
    },
}

impl TypeError {
    /// Get the span of this error.
    pub fn span(&self) -> Span {
        match self {
            TypeError::Mismatch { span, .. }
            | TypeError::UndefinedVar { span, .. }
            | TypeError::UndefinedType { span, .. }
            | TypeError::UndefinedFunction { span, .. }
            | TypeError::NonExhaustiveMatch { span, .. }
            | TypeError::InfiniteType { span, .. }
            | TypeError::MissingProtocolMethod { span, .. }
            | TypeError::AssignToImmutable { span, .. }
            | TypeError::WrongTypeArgCount { span, .. }
            | TypeError::ProtocolConstraintNotSatisfied { span, .. }
            | TypeError::RecursiveType { span, .. }
            | TypeError::AmbiguousType { span, .. }
            | TypeError::MatchOnNonEnum { span, .. }
            | TypeError::FieldAccessOnNonStruct { span, .. }
            | TypeError::InvalidAssignmentTarget { span, .. }
            | TypeError::MissingElse { span, .. }
            | TypeError::NonBooleanCondition { span, .. }
            | TypeError::BreakOutsideLoop { span, .. }
            | TypeError::ReturnOutsideFunction { span, .. }
            | TypeError::InvalidReturnType { span, .. }
            | TypeError::UnknownType { span, .. }
            | TypeError::UnknownField { span, .. }
            | TypeError::UnknownVariant { span, .. } => *span,
        }
    }

    /// Get a short description of this error.
    pub fn description(&self) -> String {
        match self {
            TypeError::Mismatch { .. } => "type mismatch".to_string(),
            TypeError::UndefinedVar { .. } => "undefined variable".to_string(),
            TypeError::UndefinedType { .. } => "undefined type".to_string(),
            TypeError::UndefinedFunction { .. } => "undefined function".to_string(),
            TypeError::NonExhaustiveMatch { .. } => "non-exhaustive match expression".to_string(),
            TypeError::InfiniteType { .. } => "infinite type".to_string(),
            TypeError::MissingProtocolMethod { .. } => "missing protocol method".to_string(),
            TypeError::AssignToImmutable { .. } => "assignment to immutable variable".to_string(),
            TypeError::WrongTypeArgCount { .. } => "wrong number of type arguments".to_string(),
            TypeError::ProtocolConstraintNotSatisfied { .. } => {
                "protocol constraint not satisfied".to_string()
            }
            TypeError::RecursiveType { .. } => "recursive type without indirection".to_string(),
            TypeError::AmbiguousType { .. } => "ambiguous type".to_string(),
            TypeError::MatchOnNonEnum { .. } => "match on non-enum type".to_string(),
            TypeError::FieldAccessOnNonStruct { .. } => "field access on non-struct type".to_string(),
            TypeError::InvalidAssignmentTarget { .. } => "invalid assignment target".to_string(),
            TypeError::MissingElse { .. } => "missing else branch".to_string(),
            TypeError::NonBooleanCondition { .. } => "non-boolean condition".to_string(),
            TypeError::BreakOutsideLoop { .. } => "break outside loop".to_string(),
            TypeError::ReturnOutsideFunction { .. } => "return outside function".to_string(),
            TypeError::InvalidReturnType { .. } => "invalid return type".to_string(),
            TypeError::UnknownType { .. } => "unknown type".to_string(),
            TypeError::UnknownField { .. } => "unknown field".to_string(),
            TypeError::UnknownVariant { .. } => "unknown enum variant".to_string(),
        }
    }
}

impl fmt::Display for TypeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TypeError::Mismatch { expected, found, .. } => {
                write!(
                    f,
                    "type mismatch: expected {:?}, found {:?}",
                    expected, found
                )
            }

            TypeError::UndefinedVar { name, .. } => {
                write!(f, "undefined variable: {}", name)
            }

            TypeError::UndefinedType { name, .. } => {
                write!(f, "undefined type: {}", name)
            }

            TypeError::UndefinedFunction {
                name, candidates, ..
            } => {
                write!(f, "undefined function: {}", name)?;
                if !candidates.is_empty() {
                    write!(f, "\ndid you mean {}?", candidates.join(", "))
                } else {
                    Ok(())
                }
            }

            TypeError::NonExhaustiveMatch { missing, .. } => {
                write!(
                    f,
                    "non-exhaustive match: missing patterns: {}",
                    missing.join(", ")
                )
            }

            TypeError::InfiniteType { .. } => {
                write!(f, "infinite type (type contains itself)")
            }

            TypeError::MissingProtocolMethod {
                ty, protocol, method, ..
            } => {
                write!(
                    f,
                    "type {} is missing method {} required by protocol {}",
                    ty, method, protocol
                )
            }

            TypeError::AssignToImmutable { name, .. } => {
                write!(f, "cannot assign to immutable variable: {}", name)
            }

            TypeError::WrongTypeArgCount {
                name, expected, found, ..
            } => {
                write!(
                    f,
                    "wrong number of type arguments for {}: expected {}, found {}",
                    name, expected, found
                )
            }

            TypeError::ProtocolConstraintNotSatisfied { ty, protocol, .. } => {
                write!(f, "type {} does not satisfy protocol {}", ty, protocol)
            }

            TypeError::RecursiveType { name, .. } => {
                write!(
                    f,
                    "recursive type {} requires indirection (use Box)",
                    name
                )
            }

            TypeError::AmbiguousType { .. } => {
                write!(f, "ambiguous type: add type annotation")
            }

            TypeError::MatchOnNonEnum { ty, .. } => {
                write!(f, "cannot match on non-enum type: {}", ty)
            }

            TypeError::FieldAccessOnNonStruct { ty, field, .. } => {
                write!(f, "type {} has no field {}", ty, field)
            }

            TypeError::InvalidAssignmentTarget { .. } => {
                write!(f, "invalid assignment target")
            }

            TypeError::MissingElse { .. } => {
                write!(f, "if expression missing else branch")
            }

            TypeError::NonBooleanCondition { found, .. } => {
                write!(f, "condition must be boolean, found {:?}", found)
            }

            TypeError::BreakOutsideLoop { .. } => {
                write!(f, "break outside loop")
            }

            TypeError::ReturnOutsideFunction { .. } => {
                write!(f, "return outside function")
            }

            TypeError::InvalidReturnType { expected, found, .. } => {
                write!(
                    f,
                    "invalid return type: expected {:?}, found {:?}",
                    expected, found
                )
            }

            TypeError::UnknownType { name, .. } => {
                write!(f, "unknown type: {}", name)
            }

            TypeError::UnknownField { ty, field, .. } => {
                write!(f, "type {} has no field {}", ty, field)
            }

            TypeError::UnknownVariant { ty, variant, .. } => {
                write!(f, "enum {} has no variant {}", ty, variant)
            }
        }
    }
}

impl std::error::Error for TypeError {}

/// A result type for type checking operations.
pub type Result<T> = std::result::Result<T, TypeError>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::PrimTy;

    #[test]
    fn test_error_display() {
        let err = TypeError::UndefinedVar {
            name: "x".to_string(),
            span: Span::new(0, 0, 0, 0, 0, 0),
        };
        assert_eq!(format!("{}", err), "undefined variable: x");
    }

    #[test]
    fn test_mismatch_error() {
        let err = TypeError::Mismatch {
            expected: Ty::Primitive(PrimTy::Int64),
            found: Ty::Primitive(PrimTy::Bool),
            span: Span::new(0, 0, 0, 0, 0, 0),
        };
        assert!(format!("{}", err).contains("type mismatch"));
    }
}
