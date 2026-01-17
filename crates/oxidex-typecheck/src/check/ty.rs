//! Type annotation conversion.
//!
//! This module converts AST type annotations to the internal `Ty` representation,
//! enabling proper type checking of annotated signatures and fields.

use crate::error::{Result, TypeError};
use crate::infer::Context;
use crate::types::{PrimTy, Ty};
use oxidex_syntax::ast::ty::Type;
use oxidex_syntax::Span;

/// Convert an AST type annotation to a `Ty`.
///
/// This function resolves type names, handles generic parameters, and
/// ensures the type annotation is well-formed.
pub fn ast_to_ty<'ctx>(ctx: &mut Context<'ctx>, ast_ty: &Type) -> Result<Ty> {
    match ast_ty {
        // Simple type identifier: `Int`, `String`, `MyType`, `T`
        Type::Simple { name, span: _ } => {
            let name_str = ctx.interner.resolve(*name).unwrap_or("");

            // Handle special types first
            if name_str == "Never" {
                return Ok(Ty::Never);
            }

            // Check if this is a generic parameter in scope
            if let Some(type_var) = ctx.lookup_generic_param(*name) {
                return Ok(Ty::TypeVar(type_var));
            }

            // Check for primitive types
            if let Some(prim) = resolve_primitive(name_str) {
                return Ok(Ty::Primitive(prim));
            }

            // Look up type in environment
            // TODO: Implement proper type lookup
            // For now, create a struct type reference
            Ok(Ty::Struct {
                name: *name,
                type_args: vec![],
            })
        }

        // Generic type: `List<T>`, `Map<K, V>`
        Type::Generic { name, params, span } => {
            let name_str = ctx.interner.resolve(*name).unwrap_or("");

            // Convert type parameters
            let mut ty_params = Vec::new();
            for param in params {
                ty_params.push(ast_to_ty(ctx, param)?);
            }

            // Check for special generic types
            match name_str {
                "Array" | "List" => {
                    if ty_params.len() == 1 {
                        return Ok(Ty::Array(Box::new(ty_params.into_iter().next().unwrap())));
                    }
                }
                "Dict" | "Map" => {
                    if ty_params.len() == 2 {
                        let mut iter = ty_params.into_iter();
                        return Ok(Ty::Dict {
                            key: Box::new(iter.next().unwrap()),
                            value: Box::new(iter.next().unwrap()),
                        });
                    }
                }
                "Option" | "Optional" => {
                    if ty_params.len() == 1 {
                        return Ok(Ty::Optional(Box::new(ty_params.into_iter().next().unwrap())));
                    }
                }
                "Result" => {
                    if ty_params.len() == 2 {
                        let mut iter = ty_params.into_iter();
                        return Ok(Ty::Result {
                            ok: Box::new(iter.next().unwrap()),
                            error: Box::new(iter.next().unwrap()),
                        });
                    }
                }
                _ => {
                    // User-defined generic type
                    // TODO: Look up the type definition and validate parameter count
                }
            }

            // Default to struct with type args
            Ok(Ty::Struct {
                name: *name,
                type_args: ty_params,
            })
        }

        // Tuple type: `(T1, T2, T3)`
        Type::Tuple { elements, span: _ } => {
            let mut ty_elements = Vec::new();
            for elem in elements {
                ty_elements.push(ast_to_ty(ctx, elem)?);
            }
            Ok(Ty::Tuple(ty_elements))
        }

        // Function type: `(T1, T2) -> T3`
        Type::Function {
            params,
            return_type,
            span: _,
        } => {
            let mut ty_params = Vec::new();
            for param in params {
                ty_params.push(ast_to_ty(ctx, param)?);
            }

            let ty_ret = ast_to_ty(ctx, return_type)?;
            let num_params = ty_params.len();

            Ok(Ty::Function {
                params: ty_params,
                return_type: Box::new(ty_ret),
                labels: vec![None; num_params],
            })
        }

        // Array type: `[T]` or `[T; N]`
        Type::Array {
            element,
            size: _,
            span: _,
        } => {
            let ty_elem = ast_to_ty(ctx, element)?;
            Ok(Ty::Array(Box::new(ty_elem)))
        }

        // Dictionary type: `[K: V]`
        Type::Dict { key, value, span: _ } => {
            let ty_key = ast_to_ty(ctx, key)?;
            let ty_value = ast_to_ty(ctx, value)?;
            Ok(Ty::Dict {
                key: Box::new(ty_key),
                value: Box::new(ty_value),
            })
        }

        // Optional type: `T?`
        Type::Optional { inner, span: _ } => {
            let ty_inner = ast_to_ty(ctx, inner)?;
            Ok(Ty::Optional(Box::new(ty_inner)))
        }

        // Self type: `Self`
        Type::SelfType { span: _ } => Ok(Ty::SelfType),
    }
}

/// Resolve a primitive type name to a `PrimTy`.
fn resolve_primitive(name: &str) -> Option<PrimTy> {
    match name {
        "Int8" => Some(PrimTy::Int8),
        "Int16" => Some(PrimTy::Int16),
        "Int32" => Some(PrimTy::Int32),
        "Int64" => Some(PrimTy::Int64),
        "Int128" => Some(PrimTy::Int128),
        "Int" => Some(PrimTy::Int64),
        "UInt8" => Some(PrimTy::UInt8),
        "UInt16" => Some(PrimTy::UInt16),
        "UInt32" => Some(PrimTy::UInt32),
        "UInt64" => Some(PrimTy::UInt64),
        "UInt128" => Some(PrimTy::UInt128),
        "UInt" => Some(PrimTy::UInt64),
        "Float32" => Some(PrimTy::Float32),
        "Float64" => Some(PrimTy::Float64),
        "Float" => Some(PrimTy::Float64),
        "Bool" => Some(PrimTy::Bool),
        "String" => Some(PrimTy::String),
        "Unit" | "Nil" => Some(PrimTy::Unit),
        "Char" => Some(PrimTy::Char),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxidex_mem::StringInterner;

    #[test]
    fn test_ast_to_ty_primitive() {
        let mut interner = StringInterner::new();
        let int_sym = interner.intern("Int");
        let mut ctx = Context::new(&interner);

        let ast_ty = Type::Simple {
            name: int_sym,
            span: Span::new(0, 0, 0, 0, 0, 0),
        };

        let ty = ast_to_ty(&mut ctx, &ast_ty).unwrap();
        assert_eq!(ty, Ty::Primitive(PrimTy::Int64));
    }

    #[test]
    fn test_ast_to_ty_tuple() {
        let mut interner = StringInterner::new();
        let int_sym = interner.intern("Int");
        let bool_sym = interner.intern("Bool");
        let mut ctx = Context::new(&interner);

        let ast_ty = Type::Tuple {
            elements: vec![
                Type::Simple {
                    name: int_sym,
                    span: Span::new(0, 0, 0, 0, 0, 0),
                },
                Type::Simple {
                    name: bool_sym,
                    span: Span::new(0, 0, 0, 0, 0, 0),
                },
            ],
            span: Span::new(0, 0, 0, 0, 0, 0),
        };

        let ty = ast_to_ty(&mut ctx, &ast_ty).unwrap();
        assert_eq!(
            ty,
            Ty::Tuple(vec![
                Ty::Primitive(PrimTy::Int64),
                Ty::Primitive(PrimTy::Bool),
            ])
        );
    }

    #[test]
    fn test_ast_to_ty_array() {
        let mut interner = StringInterner::new();
        let int_sym = interner.intern("Int");
        let mut ctx = Context::new(&interner);

        let ast_ty = Type::Array {
            element: Box::new(Type::Simple {
                name: int_sym,
                span: Span::new(0, 0, 0, 0, 0, 0),
            }),
            size: None,
            span: Span::new(0, 0, 0, 0, 0, 0),
        };

        let ty = ast_to_ty(&mut ctx, &ast_ty).unwrap();
        assert_eq!(ty, Ty::Array(Box::new(Ty::Primitive(PrimTy::Int64))));
    }

    #[test]
    fn test_ast_to_ty_optional() {
        let mut interner = StringInterner::new();
        let int_sym = interner.intern("Int");
        let mut ctx = Context::new(&interner);

        let ast_ty = Type::Optional {
            inner: Box::new(Type::Simple {
                name: int_sym,
                span: Span::new(0, 0, 0, 0, 0, 0),
            }),
            span: Span::new(0, 0, 0, 0, 0, 0),
        };

        let ty = ast_to_ty(&mut ctx, &ast_ty).unwrap();
        assert_eq!(ty, Ty::Optional(Box::new(Ty::Primitive(PrimTy::Int64))));
    }

    #[test]
    fn test_ast_to_ty_function() {
        let mut interner = StringInterner::new();
        let int_sym = interner.intern("Int");
        let bool_sym = interner.intern("Bool");
        let mut ctx = Context::new(&interner);

        let ast_ty = Type::Function {
            params: vec![Type::Simple {
                name: int_sym,
                span: Span::new(0, 0, 0, 0, 0, 0),
            }],
            return_type: Box::new(Type::Simple {
                name: bool_sym,
                span: Span::new(0, 0, 0, 0, 0, 0),
            }),
            span: Span::new(0, 0, 0, 0, 0, 0),
        };

        let ty = ast_to_ty(&mut ctx, &ast_ty).unwrap();
        assert_eq!(
            ty,
            Ty::Function {
                params: vec![Ty::Primitive(PrimTy::Int64)],
                return_type: Box::new(Ty::Primitive(PrimTy::Bool)),
                labels: vec![None],
            }
        );
    }
}
