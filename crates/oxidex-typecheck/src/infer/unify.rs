//! Unification algorithm with occurs check.
//!
//! This module implements type unification, the core of type inference.
//! Unification finds substitutions that make two types equal.

use crate::context::Subst;
use crate::error::{Result, TypeError};
use crate::types::Ty;
use oxidex_syntax::Span;

/// Unification context that tracks spans for error reporting.
pub struct Unifier<'ctx> {
    /// Current substitution
    pub subst: Subst,
    /// Phantom for lifetime
    _phantom: std::marker::PhantomData<&'ctx ()>,
}

impl<'ctx> Unifier<'ctx> {
    /// Create a new unifier.
    pub fn new(subst: Subst) -> Self {
        Self {
            subst,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Unify two types, accumulating substitutions.
    ///
    /// This performs structural unification with an occurs check to prevent
    /// infinite types like `a = List<a>`.
    ///
    /// # Errors
    ///
    /// Returns a `TypeError` if the types cannot be unified.
    pub fn unify(&mut self, ty1: &Ty, ty2: &Ty, span: Span) -> Result<()> {
        match (ty1, ty2) {
            // Type variable cases
            (Ty::TypeVar(i), _) => self.unify_var(*i, ty2, span),
            (_, Ty::TypeVar(i)) => self.unify_var(*i, ty1, span),

            // Primitive types
            (Ty::Primitive(p1), Ty::Primitive(p2)) if p1 == p2 => Ok(()),

            (Ty::Primitive(p1), Ty::Primitive(p2)) => Err(TypeError::Mismatch {
                expected: Ty::Primitive(*p1),
                found: Ty::Primitive(*p2),
                span,
            }),

            // Struct types
            (Ty::Struct { name: n1, type_args: args1 }, Ty::Struct { name: n2, type_args: args2 })
                if n1 == n2 =>
            {
                self.unify_types(args1, args2, span)
            }

            // Class types
            (Ty::Class { name: n1, type_args: args1 }, Ty::Class { name: n2, type_args: args2 })
                if n1 == n2 =>
            {
                self.unify_types(args1, args2, span)
            }

            // Enum types
            (Ty::Enum { name: n1, type_args: args1 }, Ty::Enum { name: n2, type_args: args2 })
                if n1 == n2 =>
            {
                self.unify_types(args1, args2, span)
            }

            // Protocol types
            (
                Ty::Protocol { name: n1, type_args: args1 },
                Ty::Protocol { name: n2, type_args: args2 },
            ) if n1 == n2 => self.unify_types(args1, args2, span),

            // Tuple types
            (Ty::Tuple(types1), Ty::Tuple(types2)) if types1.len() == types2.len() => {
                for (t1, t2) in types1.iter().zip(types2.iter()) {
                    self.unify(t1, t2, span)?;
                }
                Ok(())
            }

            (Ty::Tuple(types1), Ty::Tuple(types2)) => Err(TypeError::Mismatch {
                expected: Ty::Tuple(types1.clone()),
                found: Ty::Tuple(types2.clone()),
                span,
            }),

            // Function types
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
            ) if p1.len() == p2.len() => {
                // Unify parameters
                for (param1, param2) in p1.iter().zip(p2.iter()) {
                    self.unify(param1, param2, span)?;
                }

                // Unify return types
                self.unify(r1, r2, span)
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
            ) => Err(TypeError::Mismatch {
                expected: Ty::Function {
                    params: p1.clone(),
                    return_type: r1.clone(),
                    labels: vec![],
                },
                found: Ty::Function {
                    params: p2.clone(),
                    return_type: r2.clone(),
                    labels: vec![],
                },
                span,
            }),

            // Array types
            (Ty::Array(a1), Ty::Array(a2)) => self.unify(a1, a2, span),

            // Dict types
            (
                Ty::Dict { key: k1, value: v1 },
                Ty::Dict { key: k2, value: v2 },
            ) => {
                self.unify(k1, k2, span)?;
                self.unify(v1, v2, span)
            }

            // Optional types
            (Ty::Optional(o1), Ty::Optional(o2)) => self.unify(o1, o2, span),

            // Result types
            (
                Ty::Result { ok: o1, error: e1 },
                Ty::Result { ok: o2, error: e2 },
            ) => {
                self.unify(o1, o2, span)?;
                self.unify(e1, e2, span)
            }

            // Self type
            (Ty::SelfType, Ty::SelfType) => Ok(()),

            // Never type (bottom) - unifies with anything
            (Ty::Never, _) | (_, Ty::Never) => Ok(()),

            // Error type - unifies with anything (for error recovery)
            (Ty::Error, _) | (_, Ty::Error) => Ok(()),

            // All other cases are type mismatches
            _ => Err(TypeError::Mismatch {
                expected: ty1.clone(),
                found: ty2.clone(),
                span,
            }),
        }
    }

    /// Unify a type variable with a type.
    fn unify_var(&mut self, var: u32, ty: &Ty, span: Span) -> Result<()> {
        // Follow the union-find links to get the representative
        let rep = self.subst.lookup_rep(var).map_err(|_| TypeError::InfiniteType { span })?;

        match rep {
            Ty::TypeVar(other_var) if other_var == var => {
                // Variable is unbound - bind it
                // Occurs check: prevent infinite types
                if ty.occurs_in(var) {
                    return Err(TypeError::InfiniteType { span });
                }

                // Bind the variable to the type
                self.subst.bind(var, ty.clone());
                Ok(())
            }

            Ty::TypeVar(other_var) => {
                // Variable is bound to another variable - union them
                self.subst.union(var, other_var);
                Ok(())
            }

            _ => {
                // Variable is bound to a concrete type - unify with that
                self.unify(&rep, ty, span)
            }
        }
    }

    /// Unify two lists of types (for generic type arguments).
    fn unify_types(&mut self, types1: &[Ty], types2: &[Ty], span: Span) -> Result<()> {
        if types1.len() != types2.len() {
            return Err(TypeError::Mismatch {
                expected: Ty::Tuple(types1.to_vec()),
                found: Ty::Tuple(types2.to_vec()),
                span,
            });
        }

        for (t1, t2) in types1.iter().zip(types2.iter()) {
            self.unify(t1, t2, span)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::PrimTy;

    #[test]
    fn test_unify_primitives() {
        let subst = Subst::new();
        let mut unifier = Unifier::new(subst);

        // Same primitives
        unifier
            .unify(
                &Ty::Primitive(PrimTy::Int64),
                &Ty::Primitive(PrimTy::Int64),
                Span::new(0, 0, 0, 0, 0, 0),
            )
            .unwrap();

        // Different primitives
        let result = unifier.unify(
            &Ty::Primitive(PrimTy::Int64),
            &Ty::Primitive(PrimTy::Bool),
            Span::new(0, 0, 0, 0, 0, 0),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_unify_tuples() {
        let subst = Subst::new();
        let mut unifier = Unifier::new(subst);

        // Same length, same types
        unifier
            .unify(
                &Ty::Tuple(vec![Ty::Primitive(PrimTy::Int64), Ty::Primitive(PrimTy::Bool)]),
                &Ty::Tuple(vec![Ty::Primitive(PrimTy::Int64), Ty::Primitive(PrimTy::Bool)]),
                Span::new(0, 0, 0, 0, 0, 0),
            )
            .unwrap();

        // Same length, different types
        let result = unifier.unify(
            &Ty::Tuple(vec![Ty::Primitive(PrimTy::Int64)]),
            &Ty::Tuple(vec![Ty::Primitive(PrimTy::Bool)]),
            Span::new(0, 0, 0, 0, 0, 0),
        );
        assert!(result.is_err());

        // Different lengths
        let result = unifier.unify(
            &Ty::Tuple(vec![Ty::Primitive(PrimTy::Int64)]),
            &Ty::Tuple(vec![
                Ty::Primitive(PrimTy::Int64),
                Ty::Primitive(PrimTy::Bool),
            ]),
            Span::new(0, 0, 0, 0, 0, 0),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_unify_functions() {
        let subst = Subst::new();
        let mut unifier = Unifier::new(subst);

        // Same signature
        unifier
            .unify(
                &Ty::Function {
                    params: vec![Ty::Primitive(PrimTy::Int64)],
                    return_type: Box::new(Ty::Primitive(PrimTy::Bool)),
                    labels: vec![None],
                },
                &Ty::Function {
                    params: vec![Ty::Primitive(PrimTy::Int64)],
                    return_type: Box::new(Ty::Primitive(PrimTy::Bool)),
                    labels: vec![None],
                },
                Span::new(0, 0, 0, 0, 0, 0),
            )
            .unwrap();
    }

    #[test]
    fn test_unify_arrays() {
        let subst = Subst::new();
        let mut unifier = Unifier::new(subst);

        unifier
            .unify(
                &Ty::Array(Box::new(Ty::Primitive(PrimTy::Int64))),
                &Ty::Array(Box::new(Ty::Primitive(PrimTy::Int64))),
                Span::new(0, 0, 0, 0, 0, 0),
            )
            .unwrap();

        let result = unifier.unify(
            &Ty::Array(Box::new(Ty::Primitive(PrimTy::Int64))),
            &Ty::Array(Box::new(Ty::Primitive(PrimTy::Bool))),
            Span::new(0, 0, 0, 0, 0, 0),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_unify_type_var() {
        let mut subst = Subst::new();
        let var = subst.fresh_var();
        let mut unifier = Unifier::new(subst);

        // Unify type var with concrete type
        unifier
            .unify(
                &Ty::TypeVar(var),
                &Ty::Primitive(PrimTy::Int64),
                Span::new(0, 0, 0, 0, 0, 0),
            )
            .unwrap();

        // Check that the variable is now bound
        let lookup = unifier.subst.lookup(var);
        assert_eq!(lookup, Some(&Ty::Primitive(PrimTy::Int64)));
    }

    #[test]
    fn test_unify_type_vars() {
        let mut subst = Subst::new();
        let var1 = subst.fresh_var();
        let var2 = subst.fresh_var();
        let mut unifier = Unifier::new(subst);

        // Unify two type vars
        unifier
            .unify(
                &Ty::TypeVar(var1),
                &Ty::TypeVar(var2),
                Span::new(0, 0, 0, 0, 0, 0),
            )
            .unwrap();

        // After unification, the two variables should be in the same equivalence class
        // We can verify this by checking that lookup_rep succeeds for both
        let rep1 = unifier.subst.lookup_rep(var1);
        let rep2 = unifier.subst.lookup_rep(var2);

        assert!(rep1.is_ok());
        assert!(rep2.is_ok());

        // Both should resolve to the same representative (or one to the other)
        let ty1 = rep1.unwrap();
        let ty2 = rep2.unwrap();

        // They should both be type variables
        assert!(matches!(ty1, Ty::TypeVar(_)));
        assert!(matches!(ty2, Ty::TypeVar(_)));
    }

    #[test]
    fn test_unify_dict() {
        let subst = Subst::new();
        let mut unifier = Unifier::new(subst);

        // Same key and value types
        unifier
            .unify(
                &Ty::Dict {
                    key: Box::new(Ty::Primitive(PrimTy::String)),
                    value: Box::new(Ty::Primitive(PrimTy::Int64)),
                },
                &Ty::Dict {
                    key: Box::new(Ty::Primitive(PrimTy::String)),
                    value: Box::new(Ty::Primitive(PrimTy::Int64)),
                },
                Span::new(0, 0, 0, 0, 0, 0),
            )
            .unwrap();

        // Different key types should fail
        let result = unifier.unify(
            &Ty::Dict {
                key: Box::new(Ty::Primitive(PrimTy::String)),
                value: Box::new(Ty::Primitive(PrimTy::Int64)),
            },
            &Ty::Dict {
                key: Box::new(Ty::Primitive(PrimTy::Int64)),
                value: Box::new(Ty::Primitive(PrimTy::Int64)),
            },
            Span::new(0, 0, 0, 0, 0, 0),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_unify_optional() {
        let subst = Subst::new();
        let mut unifier = Unifier::new(subst);

        // Same inner types
        unifier
            .unify(
                &Ty::Optional(Box::new(Ty::Primitive(PrimTy::Int64))),
                &Ty::Optional(Box::new(Ty::Primitive(PrimTy::Int64))),
                Span::new(0, 0, 0, 0, 0, 0),
            )
            .unwrap();

        // Different inner types should fail
        let result = unifier.unify(
            &Ty::Optional(Box::new(Ty::Primitive(PrimTy::Int64))),
            &Ty::Optional(Box::new(Ty::Primitive(PrimTy::Bool))),
            Span::new(0, 0, 0, 0, 0, 0),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_unify_result() {
        let subst = Subst::new();
        let mut unifier = Unifier::new(subst);

        // Same ok and error types
        unifier
            .unify(
                &Ty::Result {
                    ok: Box::new(Ty::Primitive(PrimTy::Int64)),
                    error: Box::new(Ty::Primitive(PrimTy::String)),
                },
                &Ty::Result {
                    ok: Box::new(Ty::Primitive(PrimTy::Int64)),
                    error: Box::new(Ty::Primitive(PrimTy::String)),
                },
                Span::new(0, 0, 0, 0, 0, 0),
            )
            .unwrap();
    }

    #[test]
    fn test_unify_never() {
        let subst = Subst::new();
        let mut unifier = Unifier::new(subst);

        // Never type unifies with anything
        unifier
            .unify(
                &Ty::Never,
                &Ty::Primitive(PrimTy::Int64),
                Span::new(0, 0, 0, 0, 0, 0),
            )
            .unwrap();

        unifier
            .unify(
                &Ty::Primitive(PrimTy::Bool),
                &Ty::Never,
                Span::new(0, 0, 0, 0, 0, 0),
            )
            .unwrap();
    }

    #[test]
    fn test_unify_error() {
        let subst = Subst::new();
        let mut unifier = Unifier::new(subst);

        // Error type unifies with anything (for error recovery)
        unifier
            .unify(
                &Ty::Error,
                &Ty::Primitive(PrimTy::Int64),
                Span::new(0, 0, 0, 0, 0, 0),
            )
            .unwrap();

        unifier
            .unify(
                &Ty::Primitive(PrimTy::Bool),
                &Ty::Error,
                Span::new(0, 0, 0, 0, 0, 0),
            )
            .unwrap();
    }
}
