//! Core type representation for type checking.
//!
//! This module defines the `Ty` enum, which represents **inferred types** during type checking.
//! This is distinct from `ast::Type` which represents source-level type annotations.
//!
//! # Design
//!
//! - **Type variables** (`TypeVar`) are used during inference and will be unified via union-find
//! - **Primitives** are the built-in types (Int, Float, Bool, String, etc.)
//! - **Composite types** include structs, classes, enums, tuples, functions
//! - **Special types** like `SelfType`, `Never`, and `Error` serve specific purposes

use oxidex_mem::Symbol;

/// Internal type representation for type checking.
///
/// This differs from `ast::Type` which represents source annotations.
/// `Ty` represents inferred types with unification variables.
///
/// # Type Variables
///
/// During type inference, we use `TypeVar(u32)` as placeholders for unknown types.
/// These are later unified via the unification algorithm (union-find).
///
/// # Example
///
/// ```ignore
/// // Before inference: x has type TypeVar(0)
/// let x = 42;
///
/// // After unification: TypeVar(0) is unified with Int
/// // x: Int
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Ty {
    /// Type variable for inference (unification variable).
    ///
    /// The `u32` is an index into the substitution array (union-find structure).
    TypeVar(u32),

    /// Primitive type: Int, Float, Bool, String, Unit
    Primitive(PrimTy),

    /// Struct type (value type).
    ///
    /// Structs are value types with static dispatch.
    /// They are immutable by default and allocated on the stack.
    Struct {
        name: Symbol,
        type_args: Vec<Ty>,
    },

    /// Class type (reference type).
    ///
    /// Classes are reference types with dynamic dispatch.
    /// They are allocated on the heap with reference counting.
    Class {
        name: Symbol,
        type_args: Vec<Ty>,
    },

    /// Enum type (tagged union).
    ///
    /// Enums are value types with exhaustive pattern matching.
    Enum {
        name: Symbol,
        type_args: Vec<Ty>,
    },

    /// Protocol type (interface/trait constraint).
    ///
    /// Protocols define interfaces that types can implement.
    Protocol {
        name: Symbol,
        type_args: Vec<Ty>,
    },

    /// Tuple type.
    ///
    /// Example: `(Int, Bool, String)`
    Tuple(Vec<Ty>),

    /// Function type.
    ///
    /// Functions are first-class values.
    ///
    /// # Example
    ///
    /// ```ignore
    /// (Int, Bool) -> String
    /// ```
    Function {
        /// Parameter types
        params: Vec<Ty>,
        /// Return type
        return_type: Box<Ty>,
        /// Optional labels for parameters (Swift-style)
        labels: Vec<Option<Symbol>>,
    },

    /// Array type.
    ///
    /// Example: `Array<Int>`
    Array(Box<Ty>),

    /// Dictionary type.
    ///
    /// Example: `Dict<String, Int>`
    Dict {
        key: Box<Ty>,
        value: Box<Ty>,
    },

    /// Optional type (syntactic sugar for `Option<T>`).
    ///
    /// Example: `Int?` desugars to `Optional<Int>`
    Optional(Box<Ty>),

    /// Result type (syntactic sugar for `Result<T, E>`).
    ///
    /// Example: `Result<Int, String>`
    Result {
        ok: Box<Ty>,
        error: Box<Ty>,
    },

    /// Self type (within impl/class context).
    ///
    /// `Self` refers to the implementing type within a protocol or impl block.
    SelfType,

    /// Never type (bottom).
    ///
    /// The never type is the type of expressions that never return,
    /// such as functions that always panic or infinite loops.
    Never,

    /// Error type (for type errors that don't stop compilation).
    ///
    /// The error type unifies with anything (for error recovery).
    /// This allows us to report multiple errors in a single compilation.
    Error,
}

/// Primitive types built into the language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrimTy {
    /// Signed integers
    Int8,
    Int16,
    Int32,
    Int64,
    Int128,

    /// Unsigned integers
    UInt8,
    UInt16,
    UInt32,
    UInt64,
    UInt128,

    /// Floating point numbers
    Float32,
    Float64,

    /// Boolean type
    Bool,

    /// String type (Unicode-aware)
    String,

    /// Unit type (empty tuple, `()`)
    Unit,

    /// Character type
    Char,
}

impl Ty {
    /// Check if this type contains a specific type variable.
    ///
    /// This is used for the occurs check during unification to prevent infinite types.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Ty::Array(Ty::TypeVar(0)).occurs_in(0) => true
    /// // Ty::Int.occurs_in(0) => false
    /// ```
    pub fn occurs_in(&self, var: u32) -> bool {
        match self {
            Ty::TypeVar(v) if *v == var => true,

            Ty::Struct { type_args, .. }
            | Ty::Class { type_args, .. }
            | Ty::Enum { type_args, .. }
            | Ty::Protocol { type_args, .. }
            | Ty::Tuple(type_args) => type_args.iter().any(|t| t.occurs_in(var)),

            Ty::Function { params, return_type, .. } => {
                params.iter().any(|p| p.occurs_in(var)) || return_type.occurs_in(var)
            }

            Ty::Array(inner) => inner.occurs_in(var),

            Ty::Dict { key, value } => key.occurs_in(var) || value.occurs_in(var),

            Ty::Optional(inner) => inner.occurs_in(var),

            Ty::Result { ok, error } => ok.occurs_in(var) || error.occurs_in(var),

            // These types don't contain other types
            Ty::Primitive(_)
            | Ty::SelfType
            | Ty::Never
            | Ty::Error
            | Ty::TypeVar(_) => false,
        }
    }

    /// Get all free type variables in this type.
    ///
    /// A type variable is "free" if it's not bound by any quantifier.
    /// This is used for generalization (let-polymorphism).
    ///
    /// # Example
    ///
    /// ```ignore
    /// // In `forall a. a -> Int`, the type variable `a` is bound
    /// // In `a -> Int`, the type variable `a` is free
    /// ```
    pub fn free_vars(&self) -> std::collections::HashSet<u32> {
        let mut vars = std::collections::HashSet::new();
        self.collect_free_vars(&mut vars);
        vars
    }

    /// Helper function to collect free type variables.
    fn collect_free_vars(&self, vars: &mut std::collections::HashSet<u32>) {
        match self {
            Ty::TypeVar(v) => {
                vars.insert(*v);
            }

            Ty::Struct { type_args, .. }
            | Ty::Class { type_args, .. }
            | Ty::Enum { type_args, .. }
            | Ty::Protocol { type_args, .. }
            | Ty::Tuple(type_args) => {
                for ty in type_args {
                    ty.collect_free_vars(vars);
                }
            }

            Ty::Function { params, return_type, .. } => {
                for param in params {
                    param.collect_free_vars(vars);
                }
                return_type.collect_free_vars(vars);
            }

            Ty::Array(inner) => {
                inner.collect_free_vars(vars);
            }

            Ty::Dict { key, value } => {
                key.collect_free_vars(vars);
                value.collect_free_vars(vars);
            }

            Ty::Optional(inner) => {
                inner.collect_free_vars(vars);
            }

            Ty::Result { ok, error } => {
                ok.collect_free_vars(vars);
                error.collect_free_vars(vars);
            }

            // These types don't contain type variables
            Ty::Primitive(_) | Ty::SelfType | Ty::Never | Ty::Error => {}
        }
    }

    /// Structural equality check (doesn't follow type variables).
    ///
    /// This is different from `PartialEq` which also doesn't follow type variables,
    /// but this method is explicitly for structural comparison.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Ty::Int.eq_structural(&Ty::Int) => true
    /// // Ty::Array(Ty::Int).eq_structural(&Ty::Array(Ty::Int)) => true
    /// // Ty::Array(Ty::Bool).eq_structural(&Ty::Array(Ty::Int)) => false
    /// ```
    pub fn eq_structural(&self, other: &Ty) -> bool {
        match (self, other) {
            (Ty::TypeVar(a), Ty::TypeVar(b)) => a == b,

            (Ty::Primitive(a), Ty::Primitive(b)) => a == b,

            (
                Ty::Struct {
                    name: n1,
                    type_args: args1,
                },
                Ty::Struct {
                    name: n2,
                    type_args: args2,
            }) => n1 == n2 && args1.len() == args2.len() && {
                args1.iter().zip(args2.iter()).all(|(a, b)| a.eq_structural(b))
            },

            (
                Ty::Class {
                    name: n1,
                    type_args: args1,
                },
                Ty::Class {
                    name: n2,
                    type_args: args2,
            }) => n1 == n2 && args1.len() == args2.len() && {
                args1.iter().zip(args2.iter()).all(|(a, b)| a.eq_structural(b))
            },

            (
                Ty::Enum {
                    name: n1,
                    type_args: args1,
                },
                Ty::Enum {
                    name: n2,
                    type_args: args2,
            }) => n1 == n2 && args1.len() == args2.len() && {
                args1.iter().zip(args2.iter()).all(|(a, b)| a.eq_structural(b))
            },

            (Ty::Tuple(a), Ty::Tuple(b)) => {
                a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| x.eq_structural(y))
            }

            (
                Ty::Function {
                    params: p1,
                    return_type: r1,
                    ..
                },
                Ty::Function {
                    params: p2,
                    return_type: r2,
                    ..
                },
            ) => {
                p1.len() == p2.len()
                    && p1.iter()
                        .zip(p2.iter())
                        .all(|(x, y)| x.eq_structural(y))
                    && r1.eq_structural(r2)
            }

            (Ty::Array(a), Ty::Array(b)) => a.eq_structural(b),

            (
                Ty::Dict { key: k1, value: v1 },
                Ty::Dict { key: k2, value: v2 },
            ) => k1.eq_structural(k2) && v1.eq_structural(v2),

            (Ty::Optional(a), Ty::Optional(b)) => a.eq_structural(b),

            (
                Ty::Result { ok: o1, error: e1 },
                Ty::Result { ok: o2, error: e2 },
            ) => o1.eq_structural(o2) && e1.eq_structural(e2),

            (Ty::SelfType, Ty::SelfType) | (Ty::Never, Ty::Never) => true,

            // Error type unifies with anything
            (Ty::Error, _) | (_, Ty::Error) => true,

            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_equality() {
        assert_eq!(Ty::Primitive(PrimTy::Int64), Ty::Primitive(PrimTy::Int64));
        assert_ne!(
            Ty::Primitive(PrimTy::Int64),
            Ty::Primitive(PrimTy::Float64)
        );
    }

    #[test]
    fn test_occurs_in() {
        let ty = Ty::TypeVar(0);
        assert!(ty.occurs_in(0));
        assert!(!ty.occurs_in(1));

        let nested = Ty::Array(Box::new(Ty::TypeVar(0)));
        assert!(nested.occurs_in(0));
        assert!(!nested.occurs_in(1));
    }

    #[test]
    fn test_free_vars() {
        let ty = Ty::TypeVar(0);
        let vars = ty.free_vars();
        assert_eq!(vars.len(), 1);
        assert!(vars.contains(&0));

        let ty2 = Ty::Tuple(vec![Ty::TypeVar(0), Ty::TypeVar(1)]);
        let vars2 = ty2.free_vars();
        assert_eq!(vars2.len(), 2);
        assert!(vars2.contains(&0));
        assert!(vars2.contains(&1));
    }

    #[test]
    fn test_free_vars_nested() {
        let ty = Ty::Array(Box::new(Ty::TypeVar(0)));
        let vars = ty.free_vars();
        assert_eq!(vars.len(), 1);
        assert!(vars.contains(&0));
    }

    #[test]
    fn test_structural_equality() {
        let ty1 = Ty::Primitive(PrimTy::Int64);
        let ty2 = Ty::Primitive(PrimTy::Int64);
        assert!(ty1.eq_structural(&ty2));

        let arr1 = Ty::Array(Box::new(Ty::Primitive(PrimTy::Int64)));
        let arr2 = Ty::Array(Box::new(Ty::Primitive(PrimTy::Int64)));
        assert!(arr1.eq_structural(&arr2));

        let arr3 = Ty::Array(Box::new(Ty::Primitive(PrimTy::Bool)));
        assert!(!arr1.eq_structural(&arr3));
    }
}
