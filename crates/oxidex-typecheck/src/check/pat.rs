//! Pattern type checking.
//!
//! This module implements type checking for patterns, including:
//! - Wildcard patterns
//! - Literal patterns
//! - Variable binding patterns
//! - Struct patterns
//! - Enum patterns
//! - Tuple patterns
//! - Array patterns
//! - Or patterns

use crate::error::Result;
use crate::infer::Context;
use crate::types::{PrimTy, Ty};
use oxidex_syntax::ast::pat::{FieldPat, Pattern};
use oxidex_syntax::Span;

/// Type check a pattern against an expected type.
///
/// This function:
/// 1. Verifies the pattern matches the expected type
/// 2. Binds any variables in the pattern to the environment
/// 3. Returns the type of the pattern (usually the same as expected)
pub fn check_pat<'ctx>(ctx: &mut Context<'ctx>, pat: &Pattern, expected: &Ty, span: Span) -> Result<()> {
    match pat {
        // Wildcard pattern: `_`
        // Matches anything and binds nothing
        Pattern::Wildcard { span: _ } => Ok(()),

        // Literal pattern: `42`, `"hello"`, `true`
        Pattern::Literal { value, span: _ } => {
            let ty_literal = ty_from_literal(value);
            ctx.unify(expected, &ty_literal, span)
        }

        // Variable binding pattern: `x`, `mut x`
        Pattern::Variable { name, mutable, span: _ } => {
            // Bind the variable to the expected type
            use crate::context::Scheme;
            let scheme = Scheme::mono(expected.clone());
            ctx.env.bind_mut(*name, scheme, *mutable);
            Ok(())
        }

        // Struct pattern: `Point { x, y }` or `Point { x: x0, y: y0 }`
        Pattern::Struct {
            type_path,
            fields,
            span: _,
        } => {
            // Look up the struct type definition
            if type_path.len() != 1 {
                // TODO: Handle paths like Module::Struct
                // For now, just verify it's a struct type
                match expected {
                    Ty::Struct { .. } => {
                        for field in fields {
                            check_field_pat(ctx, field, expected)?;
                        }
                        Ok(())
                    }
                    _ => Err(crate::error::TypeError::Mismatch {
                        expected: expected.clone(),
                        found: Ty::Struct {
                            name: oxidex_mem::Symbol::new(0),
                            type_args: vec![],
                        },
                        span,
                    })
                }
            } else {
                let struct_name = type_path[0];

                // Verify the expected type matches
                match expected {
                    Ty::Struct { name, .. } if *name == struct_name => {
                        // Look up the struct definition and clone fields to avoid borrow issues
                        let struct_fields = if let Some(struct_info) = ctx.types.lookup_struct(struct_name) {
                            struct_info.fields.clone()
                        } else {
                            // Unknown struct
                            return Err(crate::error::TypeError::UnknownType {
                                name: ctx.interner.resolve(struct_name).unwrap_or("").to_string(),
                                span,
                            });
                        };

                        // Type check each field pattern
                        for field_pat in fields {
                            // Find the field in the struct definition
                            if let Some(field_info) = struct_fields.iter()
                                .find(|f| f.name == field_pat.name)
                            {
                                // If there's a nested pattern, check it with the field's type
                                if let Some(pattern) = &field_pat.pattern {
                                    check_pat(ctx, pattern, &field_info.ty, field_pat.span)?;
                                } else {
                                    // Bind the field name to the field's type
                                    use crate::context::Scheme;
                                    let scheme = Scheme::mono(field_info.ty.clone());
                                    ctx.env.bind_mut(field_pat.name, scheme, false);
                                }
                            } else {
                                // Unknown field
                                return Err(crate::error::TypeError::UnknownField {
                                    ty: ctx.interner.resolve(struct_name).unwrap_or("").to_string(),
                                    field: ctx.interner.resolve(field_pat.name).unwrap_or("").to_string(),
                                    span: field_pat.span,
                                });
                            }
                        }
                        Ok(())
                    }
                    Ty::Struct { .. } => Err(crate::error::TypeError::Mismatch {
                        expected: expected.clone(),
                        found: Ty::Struct {
                            name: struct_name,
                            type_args: vec![],
                        },
                        span,
                    }),
                    _ => Err(crate::error::TypeError::Mismatch {
                        expected: expected.clone(),
                        found: Ty::Struct {
                            name: struct_name,
                            type_args: vec![],
                        },
                        span,
                    })
                }
            }
        }

        // Enum pattern: `Option::Some(x)` or `Option::None`
        Pattern::Enum {
            type_path,
            variant,
            payload,
            span: _,
        } => {
            // Look up the enum type definition
            if type_path.len() != 1 {
                // TODO: Handle paths like Module::Enum
                match expected {
                    Ty::Enum { .. } => {
                        if let Some(payload_pat) = payload {
                            check_pat(ctx, payload_pat, expected, span)?;
                        }
                        Ok(())
                    }
                    _ => Err(crate::error::TypeError::Mismatch {
                        expected: expected.clone(),
                        found: Ty::Enum {
                            name: oxidex_mem::Symbol::new(0),
                            type_args: vec![],
                        },
                        span,
                    })
                }
            } else {
                let enum_name = type_path[0];

                // Verify the expected type matches
                match expected {
                    Ty::Enum { name, .. } if *name == enum_name => {
                        // Look up the enum definition and clone variant info to avoid borrow issues
                        let variant_payload = if let Some(enum_info) = ctx.types.lookup_enum(enum_name) {
                            // Find the variant
                            if let Some(variant_info) = enum_info.variants.iter()
                                .find(|v| v.name == *variant)
                            {
                                variant_info.payload.clone()
                            } else {
                                // Unknown variant
                                return Err(crate::error::TypeError::UnknownVariant {
                                    ty: ctx.interner.resolve(enum_name).unwrap_or("").to_string(),
                                    variant: ctx.interner.resolve(*variant).unwrap_or("").to_string(),
                                    span,
                                });
                            }
                        } else {
                            // Unknown enum
                            return Err(crate::error::TypeError::UnknownType {
                                name: ctx.interner.resolve(enum_name).unwrap_or("").to_string(),
                                span,
                            });
                        };

                        // Type check the payload if present
                        if let Some(payload_pat) = payload {
                            if let Some(payload_ty) = &variant_payload {
                                check_pat(ctx, payload_pat, payload_ty, span)?;
                            } else {
                                // Variant has no payload but pattern provides one
                                return Err(crate::error::TypeError::Mismatch {
                                    expected: Ty::Tuple(vec![]),
                                    found: Ty::Tuple(vec![Ty::TypeVar(ctx.fresh_var())]),
                                    span,
                                });
                            }
                        } else if variant_payload.is_some() {
                            // Variant has payload but pattern doesn't
                            return Err(crate::error::TypeError::Mismatch {
                                expected: variant_payload.unwrap(),
                                found: Ty::Tuple(vec![]),
                                span,
                            });
                        }
                        Ok(())
                    }
                    Ty::Enum { .. } => Err(crate::error::TypeError::Mismatch {
                        expected: expected.clone(),
                        found: Ty::Enum {
                            name: enum_name,
                            type_args: vec![],
                        },
                        span,
                    }),
                    _ => Err(crate::error::TypeError::Mismatch {
                        expected: expected.clone(),
                        found: Ty::Enum {
                            name: enum_name,
                            type_args: vec![],
                        },
                        span,
                    })
                }
            }
        }

        // Tuple pattern: `(x, y, z)`
        Pattern::Tuple { elements, span: _ } => {
            match expected {
                Ty::Tuple(types) if types.len() == elements.len() => {
                    // Type check each element pattern
                    for (elem_pat, ty_elem) in elements.iter().zip(types.iter()) {
                        check_pat(ctx, elem_pat, ty_elem, span)?;
                    }
                    Ok(())
                }
                Ty::Tuple(types) => {
                    // Wrong number of elements
                    Err(crate::error::TypeError::Mismatch {
                        expected: expected.clone(),
                        found: Ty::Tuple(elements.iter().map(|_| Ty::TypeVar(ctx.fresh_var())).collect()),
                        span,
                    })
                }
                _ => {
                    // Expected a tuple
                    Err(crate::error::TypeError::Mismatch {
                        expected: expected.clone(),
                        found: Ty::Tuple(vec![]),
                        span,
                    })
                }
            }
        }

        // Array pattern: `[first, second, ..rest]`
        Pattern::Array { elements, rest, span: _ } => {
            match expected {
                Ty::Array(elem_ty) => {
                    // Type check each element pattern
                    for elem_pat in elements {
                        check_pat(ctx, elem_pat, elem_ty, span)?;
                    }

                    // Type check the rest pattern if present
                    if let Some(rest_pat) = rest {
                        check_pat(ctx, rest_pat, expected, span)?;
                    }

                    Ok(())
                }
                _ => {
                    // Expected an array
                    Err(crate::error::TypeError::Mismatch {
                        expected: expected.clone(),
                        found: Ty::Array(Box::new(Ty::TypeVar(ctx.fresh_var()))),
                        span,
                    })
                }
            }
        }

        // Or pattern: `pattern1 | pattern2`
        Pattern::Or { left, right, span: _ } => {
            // Both patterns must match the same expected type
            check_pat(ctx, left, expected, span)?;
            check_pat(ctx, right, expected, span)
        }
    }
}

/// Type check a field pattern in a struct pattern.
fn check_field_pat<'ctx>(ctx: &mut Context<'ctx>, field: &FieldPat, expected: &Ty) -> Result<()> {
    // TODO: Look up the field type in the struct definition
    // For now, if there's a nested pattern, check it with the expected type
    if let Some(pattern) = &field.pattern {
        // Default to a variable pattern with the field name
        check_pat(ctx, pattern, expected, field.span)?;
    } else {
        // Bind the field name to the expected type
        use crate::context::Scheme;
        let scheme = Scheme::mono(expected.clone());
        ctx.env.bind_mut(field.name, scheme, false);
    }
    Ok(())
}

/// Get the type of a literal token.
fn ty_from_literal(token: &oxidex_syntax::token::TokenKind) -> Ty {
    match token {
        oxidex_syntax::token::TokenKind::IntegerLiteral(_, _) => Ty::Primitive(PrimTy::Int64),
        oxidex_syntax::token::TokenKind::FloatLiteral(_, _) => Ty::Primitive(PrimTy::Float64),
        oxidex_syntax::token::TokenKind::StringLiteral(_) => Ty::Primitive(PrimTy::String),
        oxidex_syntax::token::TokenKind::BoolLiteral(_) => Ty::Primitive(PrimTy::Bool),
        _ => Ty::Error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxidex_mem::StringInterner;

    #[test]
    fn test_check_wildcard() {
        let interner = StringInterner::new();
        let mut ctx = Context::new(&interner);

        let pat = Pattern::Wildcard {
            span: Span::new(0, 0, 0, 0, 0, 0),
        };
        let expected = Ty::Primitive(PrimTy::Int64);

        assert!(check_pat(&mut ctx, &pat, &expected, Span::new(0, 0, 0, 0, 0, 0)).is_ok());
    }

    #[test]
    fn test_check_variable() {
        let mut interner = StringInterner::new();
        let x_sym = interner.intern("x");
        let mut ctx = Context::new(&interner);

        let pat = Pattern::Variable {
            name: x_sym,
            mutable: false,
            span: Span::new(0, 0, 0, 0, 0, 0),
        };
        let expected = Ty::Primitive(PrimTy::Int64);

        assert!(check_pat(&mut ctx, &pat, &expected, Span::new(0, 0, 0, 0, 0, 0)).is_ok());

        // Check that x is now bound in the environment
        let scheme = ctx.env.lookup(x_sym);
        assert!(scheme.is_some());
    }

    #[test]
    fn test_check_tuple() {
        let interner = StringInterner::new();
        let mut ctx = Context::new(&interner);

        let pat = Pattern::Tuple {
            elements: vec![
                Pattern::Wildcard {
                    span: Span::new(0, 0, 0, 0, 0, 0),
                },
                Pattern::Wildcard {
                    span: Span::new(0, 0, 0, 0, 0, 0),
                },
            ],
            span: Span::new(0, 0, 0, 0, 0, 0),
        };
        let expected = Ty::Tuple(vec![
            Ty::Primitive(PrimTy::Int64),
            Ty::Primitive(PrimTy::Bool),
        ]);

        assert!(check_pat(&mut ctx, &pat, &expected, Span::new(0, 0, 0, 0, 0, 0)).is_ok());
    }
}
