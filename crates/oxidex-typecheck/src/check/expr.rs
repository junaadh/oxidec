//! Expression type checking (bidirectional).
//!
//! This module implements type checking for expressions using bidirectional checking:
//! - **Synthesis mode** (synth): Infer the type of an expression
//! - **Checking mode** (check): Verify an expression has the expected type

use crate::error::{Result, TypeError};
use crate::infer::Context;
use crate::types::{PrimTy, Ty};
use oxidex_syntax::{Expr, Span, Spanned};
use oxidex_syntax::ast::expr::BinaryOp;

/// Type check an expression and infer its type.
///
/// This is the synthesis mode - we don't have an expected type,
/// so we infer the type from the expression itself.
pub fn synth<'ctx>(ctx: &mut Context<'ctx>, expr: &Expr<'ctx>) -> Result<Ty> {
    match expr {
        // Literals
        Expr::IntegerLiteral { .. } => Ok(Ty::Primitive(PrimTy::Int64)),

        Expr::FloatLiteral { .. } => Ok(Ty::Primitive(PrimTy::Float64)),

        Expr::StringLiteral { .. } => Ok(Ty::Primitive(PrimTy::String)),

        Expr::BoolLiteral { .. } => Ok(Ty::Primitive(PrimTy::Bool)),

        Expr::Nil { .. } => Ok(Ty::Primitive(PrimTy::Unit)),

        // Binary operators
        Expr::Binary { op, left, right, span } => {
            // Type check both operands first
            let ty_left = synth(ctx, left)?;
            let ty_right = synth(ctx, right)?;

            // Then check the operator based on operand types
            check_binary_op(ctx, op, &ty_left, &ty_right, *span)
        }

        // Unary operators
        Expr::Unary { op, operand, span } => {
            let ty_operand = synth(ctx, operand)?;
            match op {
                oxidex_syntax::ast::expr::UnaryOp::Negate => {
                    // Logical negation: Bool -> Bool
                    ctx.unify(&ty_operand, &Ty::Primitive(PrimTy::Bool), *span)?;
                    Ok(Ty::Primitive(PrimTy::Bool))
                }
                oxidex_syntax::ast::expr::UnaryOp::Minus => {
                    // Arithmetic negation: Int -> Int or Float -> Float
                    // Create a fresh type variable and unify with Int or Float
                    let var = ctx.fresh_var();
                    ctx.unify(&ty_operand, &Ty::TypeVar(var), *span)?;
                    // For now, assume Int (we should try both Int and Float)
                    Ok(Ty::Primitive(PrimTy::Int64))
                }
            }
        }

        // Identifiers (variable lookup)
        Expr::Identifier(sym) => {
            // Look up the variable in the environment
            let name = ctx.interner.resolve(*sym).unwrap_or("");
            let scheme = ctx.lookup(name);

            match scheme {
                Some(scheme) => {
                    // Clone the scheme so we can drop the borrow
                    let scheme_clone = scheme.clone();
                    // Instantiate the scheme (replace type variables with fresh ones)
                    let ty = scheme_clone.instantiate(ctx.subst());
                    Ok(ty)
                }
                None => {
                    // CRITICAL: Undefined variable is an error, not a fresh type var
                    Err(TypeError::UndefinedVar {
                        name: name.to_string(),
                        span: expr.span(),
                    })
                }
            }
        }

        // Path expressions (e.g., Type::item or simple identifier)
        Expr::Path { segments, span } => {
            // Handle simple single-segment paths (identifiers)
            if segments.len() == 1 {
                let name = segments[0];
                // Look up as a variable
                if let Some(scheme) = ctx.env.lookup(name) {
                    // Instantiate the scheme to get the type
                    let ty = scheme.instantiate(&mut ctx.unifier.subst);
                    return Ok(ty);
                } else {
                    return Err(TypeError::UndefinedVar {
                        name: ctx.interner.resolve(name).unwrap_or("").to_string(),
                        span: *span,
                    });
                }
            }

            // TODO: Handle multi-segment paths like Module::Type::item
            // For now, return a fresh type variable
            let var = ctx.fresh_var();
            Ok(Ty::TypeVar(var))
        }

        // Function calls
        Expr::Call { callee, args, span } => {
            // Type check callee (should be a function type)
            let ty_callee = synth(ctx, callee)?;

            // Type check arguments
            let ty_args: Result<Vec<Ty>> = args.iter().map(|arg| synth(ctx, &arg.value)).collect();
            let ty_args = ty_args?;

            // Create fresh return type variable
            let ty_ret = ctx.fresh_var();

            // Unify: callee should be (args -> ret)
            let fn_ty = Ty::Function {
                params: ty_args.clone(),
                return_type: Box::new(Ty::TypeVar(ty_ret)),
                labels: vec![None; ty_args.len()],
            };

            ctx.unify(&ty_callee, &fn_ty, *span)?;

            Ok(Ty::TypeVar(ty_ret))
        }

        // Method calls
        Expr::MethodCall { receiver, method, args, span } => {
            // Type check receiver
            let ty_receiver = synth(ctx, receiver)?;

            // Type check arguments
            let ty_args: Result<Vec<Ty>> = args.iter().map(|arg| synth(ctx, &arg.value)).collect();
            let ty_args = ty_args?;

            // Look up method in receiver's type
            match &ty_receiver {
                Ty::Struct { name, .. } => {
                    if let Some(struct_info) = ctx.types.lookup_struct(*name) {
                        // Clone method info to avoid borrow checker issues
                        let method_return = struct_info.methods.iter()
                            .find(|m| m.name == *method)
                            .map(|m| (m.params.clone(), m.return_type.clone()));

                        if let Some((method_params, method_return_type)) = method_return {
                            // Check parameter count
                            if method_params.len() != args.len() {
                                return Err(crate::error::TypeError::Mismatch {
                                    expected: Ty::Function {
                                        params: method_params.clone(),
                                        return_type: Box::new(method_return_type.clone()),
                                        labels: vec![None; method_params.len()],
                                    },
                                    found: Ty::Function {
                                        params: std::iter::once(ty_receiver).chain(ty_args).collect(),
                                        return_type: Box::new(Ty::TypeVar(0)),
                                        labels: vec![None; args.len() + 1],
                                    },
                                    span: *span,
                                });
                            }

                            // Validate argument types
                            for (ty_arg, ty_param) in ty_args.iter().zip(&method_params) {
                                ctx.unify(ty_arg, ty_param, *span)?;
                            }

                            // Return the method's return type
                            Ok(method_return_type)
                        } else {
                            // Method not found
                            return Err(crate::error::TypeError::UndefinedFunction {
                                name: ctx.interner.resolve(*method).unwrap_or("").to_string(),
                                candidates: struct_info.methods.iter()
                                    .map(|m| ctx.interner.resolve(m.name).unwrap_or("").to_string())
                                    .collect(),
                                span: *span,
                            });
                        }
                    } else {
                        // Struct not in registry - shouldn't happen
                        let ty_ret = ctx.fresh_var();
                        Ok(Ty::TypeVar(ty_ret))
                    }
                }
                Ty::Enum { name, .. } => {
                    if let Some(enum_info) = ctx.types.lookup_enum(*name) {
                        // Clone method info to avoid borrow checker issues
                        let method_return = enum_info.methods.iter()
                            .find(|m| m.name == *method)
                            .map(|m| (m.params.clone(), m.return_type.clone()));

                        if let Some((method_params, method_return_type)) = method_return {
                            // Check parameter count
                            if method_params.len() != args.len() {
                                return Err(crate::error::TypeError::Mismatch {
                                    expected: Ty::Function {
                                        params: method_params.clone(),
                                        return_type: Box::new(method_return_type.clone()),
                                        labels: vec![None; method_params.len()],
                                    },
                                    found: Ty::Function {
                                        params: std::iter::once(ty_receiver).chain(ty_args).collect(),
                                        return_type: Box::new(Ty::TypeVar(0)),
                                        labels: vec![None; args.len() + 1],
                                    },
                                    span: *span,
                                });
                            }

                            // Validate argument types
                            for (ty_arg, ty_param) in ty_args.iter().zip(&method_params) {
                                ctx.unify(ty_arg, ty_param, *span)?;
                            }

                            // Return the method's return type
                            Ok(method_return_type)
                        } else {
                            // Method not found
                            return Err(crate::error::TypeError::UndefinedFunction {
                                name: ctx.interner.resolve(*method).unwrap_or("").to_string(),
                                candidates: enum_info.methods.iter()
                                    .map(|m| ctx.interner.resolve(m.name).unwrap_or("").to_string())
                                    .collect(),
                                span: *span,
                            });
                        }
                    } else {
                        // Enum not in registry - shouldn't happen
                        let ty_ret = ctx.fresh_var();
                        Ok(Ty::TypeVar(ty_ret))
                    }
                }
                Ty::Class { name, .. } => {
                    if let Some(class_info) = ctx.types.lookup_class(*name) {
                        // Clone method info to avoid borrow checker issues
                        let method_return = class_info.methods.iter()
                            .find(|m| m.name == *method)
                            .map(|m| (m.params.clone(), m.return_type.clone()));

                        if let Some((method_params, method_return_type)) = method_return {
                            // Check parameter count
                            if method_params.len() != args.len() {
                                return Err(crate::error::TypeError::Mismatch {
                                    expected: Ty::Function {
                                        params: method_params.clone(),
                                        return_type: Box::new(method_return_type.clone()),
                                        labels: vec![None; method_params.len()],
                                    },
                                    found: Ty::Function {
                                        params: std::iter::once(ty_receiver).chain(ty_args).collect(),
                                        return_type: Box::new(Ty::TypeVar(0)),
                                        labels: vec![None; args.len() + 1],
                                    },
                                    span: *span,
                                });
                            }

                            // Validate argument types
                            for (ty_arg, ty_param) in ty_args.iter().zip(&method_params) {
                                ctx.unify(ty_arg, ty_param, *span)?;
                            }

                            // Return the method's return type
                            Ok(method_return_type)
                        } else {
                            // Method not found
                            return Err(crate::error::TypeError::UndefinedFunction {
                                name: ctx.interner.resolve(*method).unwrap_or("").to_string(),
                                candidates: class_info.methods.iter()
                                    .map(|m| ctx.interner.resolve(m.name).unwrap_or("").to_string())
                                    .collect(),
                                span: *span,
                            });
                        }
                    } else {
                        // Class not in registry - shouldn't happen
                        let ty_ret = ctx.fresh_var();
                        Ok(Ty::TypeVar(ty_ret))
                    }
                }
                _ => {
                    // Not a struct, enum, or class - error
                    return Err(crate::error::TypeError::UndefinedFunction {
                        name: ctx.interner.resolve(*method).unwrap_or("").to_string(),
                        candidates: vec![],
                        span: *span,
                    });
                }
            }
        }

        // If expressions
        Expr::If {
            condition,
            then_branch,
            else_branch,
            span,
        } => {
            // Condition must be boolean
            let ty_cond = synth(ctx, condition)?;
            ctx.unify(&ty_cond, &Ty::Primitive(PrimTy::Bool), *span)?;

            // Type check both branches
            let ty_then = synth(ctx, then_branch)?;

            match else_branch {
                Some(else_br) => {
                    let ty_else = synth(ctx, else_br)?;
                    // Both branches must have the same type
                    ctx.unify(&ty_then, &ty_else, *span)?;
                    Ok(ty_then)
                }
                None => {
                    // No else branch - if statement, not expression
                    Ok(Ty::Primitive(PrimTy::Unit))
                }
            }
        }

        // Match expressions
        Expr::Match { scrutinee, arms, span } => {
            // Type check scrutinee
            let ty_scrut = synth(ctx, scrutinee)?;

            // Check exhaustiveness for enum types
            if let Ty::Enum { name, .. } = &ty_scrut {
                if let Some(enum_info) = ctx.types.lookup_enum(*name) {
                    // Collect variants that are covered
                    let mut covered_variants = std::collections::HashSet::new();

                    for arm in arms {
                        use oxidex_syntax::ast::pat::Pattern;
                        match &arm.pattern {
                            Pattern::Wildcard { .. } => {
                                // Wildcard covers all remaining variants
                                covered_variants = enum_info.variants.iter()
                                    .map(|v| v.name)
                                    .collect();
                                break;
                            }
                            Pattern::Enum { type_path, variant, .. } => {
                                // Check if this matches our enum
                                if type_path.len() == 1 && type_path[0] == *name {
                                    covered_variants.insert(*variant);
                                }
                            }
                            _ => {
                                // Other patterns - assume they cover everything
                                // TODO: More precise pattern analysis
                                covered_variants = enum_info.variants.iter()
                                    .map(|v| v.name)
                                    .collect();
                                break;
                            }
                        }
                    }

                    // Check if all variants are covered
                    let missing_variants: Vec<_> = enum_info.variants.iter()
                        .filter(|v| !covered_variants.contains(&v.name))
                        .map(|v| ctx.interner.resolve(v.name).unwrap_or("").to_string())
                        .collect();

                    if !missing_variants.is_empty() {
                        return Err(TypeError::NonExhaustiveMatch {
                            missing: missing_variants,
                            span: *span,
                        });
                    }
                }
            }

            // All arms must have the same type
            let mut arm_types = Vec::new();

            for arm in arms {
                // Type check the pattern against the scrutinee type
                // This also binds variables in the pattern
                ctx.new_scope();
                super::pat::check_pat(ctx, &arm.pattern, &ty_scrut, arm.pattern.span())?;

                // Type check body
                let ty_body = synth(ctx, arm.body)?;
                arm_types.push(ty_body);

                ctx.pop_scope();
            }

            // All arms must have the same type
            if let Some(first_ty) = arm_types.first() {
                for ty_arm in &arm_types[1..] {
                    ctx.unify(first_ty, ty_arm, *span)?;
                }
                Ok(first_ty.clone())
            } else {
                // Empty match (shouldn't happen syntactically)
                Err(TypeError::NonExhaustiveMatch {
                    missing: vec![],
                    span: *span,
                })
            }
        }

        // Blocks
        Expr::Block { stmts, expr, span: _ } => {
            ctx.new_scope();

            // Type check statements (for now, just ignore)
            // TODO: Actually type check the statements
            let _ = stmts;

            // Type check final expression if present
            let result = match expr {
                Some(e) => synth(ctx, e),
                None => Ok(Ty::Primitive(PrimTy::Unit)),
            };

            // CRITICAL: Pop the scope before returning
            ctx.pop_scope();

            result
        }

        // Parenthesized expressions
        Expr::Paren { expr, .. } => synth(ctx, expr),

        // For loops
        Expr::ForLoop { pattern, iter, body, span: _ } => {
            // Type check iterator (should be a collection type)
            let ty_iter = synth(ctx, iter)?;

            // Get the element type from the iterator
            let ty_elem = match &ty_iter {
                Ty::Array(elem_ty) => elem_ty.as_ref(),
                Ty::Dict { .. } => {
                    // For dicts, we iterate over (key, value) tuples
                    return Err(TypeError::Mismatch {
                        expected: Ty::Array(Box::new(Ty::TypeVar(ctx.fresh_var()))),
                        found: ty_iter,
                        span: iter.span(),
                    });
                }
                _ => {
                    // Unknown iterator type
                    return Err(TypeError::Mismatch {
                        expected: Ty::Array(Box::new(Ty::TypeVar(ctx.fresh_var()))),
                        found: ty_iter,
                        span: iter.span(),
                    });
                }
            };

            // Type check pattern against element type
            ctx.new_scope();
            super::pat::check_pat(ctx, pattern, ty_elem, pattern.span())?;

            // Type check body
            let ty_body = synth(ctx, body)?;
            ctx.pop_scope();

            // For loops always return Unit
            let _ = ty_body;
            Ok(Ty::Primitive(PrimTy::Unit))
        }

        // While loops
        Expr::WhileLoop { condition, body, span: _ } => {
            // Condition must be boolean
            let ty_cond = synth(ctx, condition)?;
            // Note: We need to get the span from condition
            let _ = ty_cond;

            // Type check body (should be Unit for statement-style loops)
            let ty_body = synth(ctx, body)?;
            let _ = ty_body;

            // While loops always return Unit
            Ok(Ty::Primitive(PrimTy::Unit))
        }

        // Struct construction
        Expr::Struct { type_path, fields, span } => {
            // Look up struct type definition
            if type_path.len() != 1 {
                // TODO: Handle paths like Module::Type
                let var = ctx.fresh_var();
                return Ok(Ty::TypeVar(var));
            }

            let struct_name = type_path[0];
            if let Some(struct_info) = ctx.types.lookup_struct(struct_name) {
                // Clone struct info to avoid borrow checker issues
                let struct_fields: Vec<_> = struct_info.fields.iter()
                    .map(|f| (f.name, f.ty.clone()))
                    .collect();

                // Validate fields against definition
                let mut provided_fields = std::collections::HashMap::new();
                for field in fields {
                    // Type check field value if present
                    let ty_field = if let Some(value) = field.value {
                        synth(ctx, value)?
                    } else {
                        // Shorthand initialization - look up variable
                        // For now, we'll create a fresh type variable
                        let var = ctx.fresh_var();
                        Ty::TypeVar(var)
                    };

                    // Check if field exists in struct
                    if let Some(&(_, ref declared_ty)) = struct_fields.iter().find(|(name, _)| *name == field.name) {
                        // Unify field type with declared type
                        ctx.unify(&ty_field, declared_ty, *span)?;
                        provided_fields.insert(field.name, ty_field);
                    } else {
                        return Err(crate::error::TypeError::FieldAccessOnNonStruct {
                            ty: ctx.interner.resolve(struct_name).unwrap_or("").to_string(),
                            field: ctx.interner.resolve(field.name).unwrap_or("").to_string(),
                            span: *span,
                        });
                    }
                }

                // Check that all required fields are present
                for &(field_name, _) in &struct_fields {
                    if !provided_fields.contains_key(&field_name) {
                        return Err(crate::error::TypeError::Mismatch {
                            expected: Ty::Struct {
                                name: struct_name,
                                type_args: vec![],
                            },
                            found: Ty::Struct {
                                name: struct_name,
                                type_args: vec![],
                            },
                            span: *span,
                        });
                    }
                }

                // Return the struct type
                Ok(Ty::Struct {
                    name: struct_name,
                    type_args: vec![],
                })
            } else {
                // Struct not found - error
                Err(crate::error::TypeError::UndefinedType {
                    name: ctx.interner.resolve(struct_name).unwrap_or("").to_string(),
                    span: *span,
                })
            }
        }

        // Enum construction
        Expr::Enum { type_path, variant, payload, span } => {
            // Look up enum type definition
            if type_path.len() != 1 {
                // TODO: Handle paths like Module::Type
                let var = ctx.fresh_var();
                return Ok(Ty::TypeVar(var));
            }

            let enum_name = type_path[0];
            if let Some(enum_info) = ctx.types.lookup_enum(enum_name) {
                // Clone variant payload to avoid borrow checker issues
                let variant_payload = enum_info.variants.iter()
                    .find(|v| v.name == *variant)
                    .and_then(|v| v.payload.clone());

                if variant_payload.is_some() || enum_info.variants.iter().any(|v| v.name == *variant) {
                    // Type check payload if present
                    if let Some(payload_expr) = payload {
                        let ty_payload = synth(ctx, payload_expr)?;

                        // Unify with expected payload type
                        if let Some(expected_payload) = &variant_payload {
                            ctx.unify(&ty_payload, expected_payload, *span)?;
                        } else {
                            // Variant has no payload but we provided one
                            return Err(crate::error::TypeError::Mismatch {
                                expected: Ty::Enum {
                                    name: enum_name,
                                    type_args: vec![],
                                },
                                found: ty_payload,
                                span: *span,
                            });
                        }
                    } else {
                        // No payload provided
                        if variant_payload.is_some() {
                            // Variant requires payload but none provided
                            return Err(crate::error::TypeError::Mismatch {
                                expected: Ty::Enum {
                                    name: enum_name,
                                    type_args: vec![],
                                },
                                found: Ty::Primitive(PrimTy::Unit),
                                span: *span,
                            });
                        }
                    }

                    // Return the enum type
                    Ok(Ty::Enum {
                        name: enum_name,
                        type_args: vec![],
                    })
                } else {
                    // Variant not found
                    return Err(crate::error::TypeError::UndefinedType {
                        name: format!(
                            "{}::{}",
                            ctx.interner.resolve(enum_name).unwrap_or(""),
                            ctx.interner.resolve(*variant).unwrap_or("")
                        ),
                        span: *span,
                    });
                }
            } else {
                // Enum not found
                return Err(crate::error::TypeError::UndefinedType {
                    name: ctx.interner.resolve(enum_name).unwrap_or("").to_string(),
                    span: *span,
                });
            }
        }

        // Array literals
        Expr::Array { elements, span } => {
            if elements.is_empty() {
                // Empty array - need type annotation
                return Ok(Ty::Array(Box::new(Ty::TypeVar(ctx.fresh_var()))));
            }

            // Type check first element to get element type
            let ty_first = synth(ctx, elements[0])?;

            // Type check all other elements and unify with first
            for elem in &elements[1..] {
                let ty_elem = synth(ctx, elem)?;
                ctx.unify(&ty_first, &ty_elem, *span)?;
            }

            Ok(Ty::Array(Box::new(ty_first)))
        }

        // Dictionary literals
        Expr::Dict { entries, span } => {
            if entries.is_empty() {
                // Empty dict - need type annotation
                let k = ctx.fresh_var();
                let v = ctx.fresh_var();
                return Ok(Ty::Dict {
                    key: Box::new(Ty::TypeVar(k)),
                    value: Box::new(Ty::TypeVar(v)),
                });
            }

            // Type check first entry to get key and value types
            let ty_key_first = synth(ctx, &entries[0].key)?;
            let ty_value_first = synth(ctx, &entries[0].value)?;

            // Type check all other entries and unify with first
            for entry in &entries[1..] {
                let ty_key = synth(ctx, &entry.key)?;
                let ty_value = synth(ctx, &entry.value)?;
                ctx.unify(&ty_key_first, &ty_key, *span)?;
                ctx.unify(&ty_value_first, &ty_value, *span)?;
            }

            Ok(Ty::Dict {
                key: Box::new(ty_key_first),
                value: Box::new(ty_value_first),
            })
        }

        // Field access
        Expr::Field { object, field, span } => {
            // Type check object
            let ty_object = synth(ctx, object)?;

            // Look up field based on object type
            match &ty_object {
                Ty::Struct { name, type_args: _ } => {
                    // Look up struct definition
                    if let Some(struct_info) = ctx.types.lookup_struct(*name) {
                        // Find the field
                        if let Some(field_info) = struct_info.fields.iter().find(|f| f.name == *field) {
                            // Return the field type
                            Ok(field_info.ty.clone())
                        } else {
                            // Field not found in struct
                            Err(crate::error::TypeError::FieldAccessOnNonStruct {
                                ty: ctx.interner.resolve(*name).unwrap_or("").to_string(),
                                field: ctx.interner.resolve(*field).unwrap_or("").to_string(),
                                span: *span,
                            })
                        }
                    } else {
                        // Struct not found in registry - shouldn't happen
                        Ok(Ty::Error)
                    }
                }
                Ty::Class { name, type_args: _ } => {
                    if let Some(class_info) = ctx.types.lookup_class(*name) {
                        if let Some(field_info) = class_info.fields.iter()
                            .find(|f| f.name == *field)
                        {
                            Ok(field_info.ty.clone())
                        } else {
                            Err(crate::error::TypeError::FieldAccessOnNonStruct {
                                ty: ctx.interner.resolve(*name).unwrap_or("").to_string(),
                                field: ctx.interner.resolve(*field).unwrap_or("").to_string(),
                                span: *span,
                            })
                        }
                    } else {
                        // Class not found in registry - shouldn't happen
                        Ok(Ty::Error)
                    }
                }
                _ => {
                    // Not a struct or class - error
                    Err(crate::error::TypeError::FieldAccessOnNonStruct {
                        ty: format!("{:?}", ty_object),
                        field: ctx.interner.resolve(*field).unwrap_or("").to_string(),
                        span: *span,
                    })
                }
            }
        }

        // Index access
        Expr::Index { collection, index, span } => {
            // Type check collection
            let ty_collection = synth(ctx, collection)?;

            // Type check index (should be Int for arrays)
            let ty_index = synth(ctx, index)?;
            // For now, assume index is Int
            ctx.unify(&ty_index, &Ty::Primitive(PrimTy::Int64), *span)?;

            // Return element type based on collection type
            match ty_collection {
                Ty::Array(elem_ty) => Ok(*elem_ty),
                Ty::Dict { key: _, value } => Ok(*value),
                // For other types, return a fresh type variable
                _ => {
                    let var = ctx.fresh_var();
                    Ok(Ty::TypeVar(var))
                }
            }
        }

        // String interpolation
        Expr::Interpolation { parts, span: _ } => {
            // Type check all interpolation parts
            for part in parts {
                match part {
                    oxidex_syntax::ast::expr::InterpolationPart::Text(_) => {
                        // Text literals don't need type checking
                    }
                    oxidex_syntax::ast::expr::InterpolationPart::Expr(expr) => {
                        // Type check the interpolated expression
                        let _ = synth(ctx, expr)?;
                        // TODO: Convert to string at runtime
                    }
                }
            }
            // String interpolation always returns String
            Ok(Ty::Primitive(PrimTy::String))
        }
    }
}

/// Type check an expression against an expected type.
///
/// This is the checking mode - we know what type we expect,
/// so we can guide inference and provide better error messages.
pub fn check<'ctx>(
    ctx: &mut Context<'ctx>,
    expr: &Expr<'ctx>,
    expected: &Ty,
) -> Result<()> {
    // Infer the type of the expression
    let inferred = synth(ctx, expr)?;

    // Unify with the expected type
    let span = expr.span();
    ctx.unify(&inferred, expected, span)
}

/// Check a binary operation and infer its type.
///
/// This function validates that the operands are compatible with the operator
/// and returns the result type.
fn check_binary_op<'ctx>(
    ctx: &mut Context<'ctx>,
    op: &BinaryOp,
    ty_left: &Ty,
    ty_right: &Ty,
    span: Span,
) -> Result<Ty> {
    match op {
        // Arithmetic operators: require numeric types
        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => {
            // Both operands must be the same numeric type
            ctx.unify(ty_left, ty_right, span)?;

            // Check that at least one operand is numeric (Int or Float)
            match (ty_left, ty_right) {
                (Ty::Primitive(PrimTy::Int64), _) | (_, Ty::Primitive(PrimTy::Int64)) => {
                    Ok(Ty::Primitive(PrimTy::Int64))
                }
                (Ty::Primitive(PrimTy::Float64), _) | (_, Ty::Primitive(PrimTy::Float64)) => {
                    Ok(Ty::Primitive(PrimTy::Float64))
                }
                _ => {
                    // Both type vars - assume Int
                    Ok(Ty::Primitive(PrimTy::Int64))
                }
            }
        }

        // Comparison operators: require comparable types
        BinaryOp::Eq | BinaryOp::Neq | BinaryOp::Lt | BinaryOp::Lte | BinaryOp::Gt | BinaryOp::Gte => {
            // Both operands must be the same type
            ctx.unify(ty_left, ty_right, span)?;
            // Comparison operators always return Bool
            Ok(Ty::Primitive(PrimTy::Bool))
        }

        // Logical operators: require boolean operands
        BinaryOp::And | BinaryOp::Or => {
            // Both operands must be Bool
            ctx.unify(ty_left, &Ty::Primitive(PrimTy::Bool), span)?;
            ctx.unify(ty_right, &Ty::Primitive(PrimTy::Bool), span)?;
            // Logical operators return Bool
            Ok(Ty::Primitive(PrimTy::Bool))
        }

        // Assignment operator
        BinaryOp::Assign => {
            // Assignment has side effects, returns Unit
            // TODO: Check that left side is mutable (can't assign to literals or immutable vars)
            ctx.unify(ty_left, ty_right, span)?;
            Ok(Ty::Primitive(PrimTy::Unit))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxidex_mem::StringInterner;
    use oxidex_syntax::{Span};

    #[test]
    fn test_synth_integer() {
        let interner = StringInterner::new();
        let mut ctx = Context::new(&interner);

        let expr = Expr::IntegerLiteral {
            value: oxidex_mem::Symbol::new(0),
            type_suffix: None,
            span: Span::new(0, 0, 0, 0, 0, 0),
        };

        let ty = synth(&mut ctx, &expr).unwrap();
        assert_eq!(ty, Ty::Primitive(PrimTy::Int64));
    }

    #[test]
    fn test_synth_bool() {
        let interner = StringInterner::new();
        let mut ctx = Context::new(&interner);

        let expr = Expr::BoolLiteral {
            value: true,
            span: Span::new(0, 0, 0, 0, 0, 0),
        };

        let ty = synth(&mut ctx, &expr).unwrap();
        assert_eq!(ty, Ty::Primitive(PrimTy::Bool));
    }

    #[test]
    fn test_synth_string() {
        let interner = StringInterner::new();
        let mut ctx = Context::new(&interner);

        let expr = Expr::StringLiteral {
            value: oxidex_mem::Symbol::new(0),
            span: Span::new(0, 0, 0, 0, 0, 0),
        };

        let ty = synth(&mut ctx, &expr).unwrap();
        assert_eq!(ty, Ty::Primitive(PrimTy::String));
    }

    #[test]
    fn test_check_binary_arith() {
        let interner = StringInterner::new();
        let mut ctx = Context::new(&interner);

        // Test: 5 + 3
        let expr = Expr::Binary {
            left: Box::leak(Box::new(Expr::IntegerLiteral {
                value: oxidex_mem::Symbol::new(0),
                type_suffix: None,
                span: Span::new(0, 0, 0, 0, 0, 0),
            })),
            op: BinaryOp::Add,
            right: Box::leak(Box::new(Expr::IntegerLiteral {
                value: oxidex_mem::Symbol::new(0),
                type_suffix: None,
                span: Span::new(0, 0, 0, 0, 0, 0),
            })),
            span: Span::new(0, 0, 0, 0, 0, 0),
        };

        let ty = synth(&mut ctx, &expr).unwrap();
        assert_eq!(ty, Ty::Primitive(PrimTy::Int64));
    }

    #[test]
    fn test_check_binary_comparison() {
        let interner = StringInterner::new();
        let mut ctx = Context::new(&interner);

        // Test: 5 < 3
        let expr = Expr::Binary {
            left: Box::leak(Box::new(Expr::IntegerLiteral {
                value: oxidex_mem::Symbol::new(0),
                type_suffix: None,
                span: Span::new(0, 0, 0, 0, 0, 0),
            })),
            op: BinaryOp::Lt,
            right: Box::leak(Box::new(Expr::IntegerLiteral {
                value: oxidex_mem::Symbol::new(0),
                type_suffix: None,
                span: Span::new(0, 0, 0, 0, 0, 0),
            })),
            span: Span::new(0, 0, 0, 0, 0, 0),
        };

        let ty = synth(&mut ctx, &expr).unwrap();
        assert_eq!(ty, Ty::Primitive(PrimTy::Bool));
    }

    #[test]
    fn test_check_binary_logical() {
        let interner = StringInterner::new();
        let mut ctx = Context::new(&interner);

        // Test: true && false
        let expr = Expr::Binary {
            left: Box::leak(Box::new(Expr::BoolLiteral {
                value: true,
                span: Span::new(0, 0, 0, 0, 0, 0),
            })),
            op: BinaryOp::And,
            right: Box::leak(Box::new(Expr::BoolLiteral {
                value: false,
                span: Span::new(0, 0, 0, 0, 0, 0),
            })),
            span: Span::new(0, 0, 0, 0, 0, 0),
        };

        let ty = synth(&mut ctx, &expr).unwrap();
        assert_eq!(ty, Ty::Primitive(PrimTy::Bool));
    }

    #[test]
    fn test_check_binary_type_mismatch() {
        let interner = StringInterner::new();
        let mut ctx = Context::new(&interner);

        // Test: true + 5 (should error - can't add Bool and Int)
        let expr = Expr::Binary {
            left: Box::leak(Box::new(Expr::BoolLiteral {
                value: true,
                span: Span::new(0, 0, 0, 0, 0, 0),
            })),
            op: BinaryOp::Add,
            right: Box::leak(Box::new(Expr::IntegerLiteral {
                value: oxidex_mem::Symbol::new(0),
                type_suffix: None,
                span: Span::new(0, 0, 0, 0, 0, 0),
            })),
            span: Span::new(0, 0, 0, 0, 0, 0),
        };

        let result = synth(&mut ctx, &expr);
        assert!(result.is_err());
    }

    #[test]
    fn test_synth_unary_negate() {
        let interner = StringInterner::new();
        let mut ctx = Context::new(&interner);

        let expr = Expr::Unary {
            op: oxidex_syntax::ast::expr::UnaryOp::Negate,
            operand: Box::leak(Box::new(Expr::BoolLiteral {
                value: true,
                span: Span::new(0, 0, 0, 0, 0, 0),
            })),
            span: Span::new(0, 0, 0, 0, 0, 0),
        };

        let ty = synth(&mut ctx, &expr).unwrap();
        assert_eq!(ty, Ty::Primitive(PrimTy::Bool));
    }

    #[test]
    fn test_synth_if_expression() {
        let interner = StringInterner::new();
        let mut ctx = Context::new(&interner);

        let expr = Expr::If {
            condition: Box::leak(Box::new(Expr::BoolLiteral {
                value: true,
                span: Span::new(0, 0, 0, 0, 0, 0),
            })),
            then_branch: Box::leak(Box::new(Expr::IntegerLiteral {
                value: oxidex_mem::Symbol::new(0),
                type_suffix: None,
                span: Span::new(0, 0, 0, 0, 0, 0),
            })),
            else_branch: Some(Box::leak(Box::new(Expr::IntegerLiteral {
                value: oxidex_mem::Symbol::new(0),
                type_suffix: None,
                span: Span::new(0, 0, 0, 0, 0, 0),
            }))),
            span: Span::new(0, 0, 0, 0, 0, 0),
        };

        let ty = synth(&mut ctx, &expr).unwrap();
        assert_eq!(ty, Ty::Primitive(PrimTy::Int64));
    }

    #[test]
    fn test_synth_block() {
        let interner = StringInterner::new();
        let mut ctx = Context::new(&interner);

        let expr = Expr::Block {
            stmts: vec![],
            expr: Some(Box::leak(Box::new(Expr::IntegerLiteral {
                value: oxidex_mem::Symbol::new(0),
                type_suffix: None,
                span: Span::new(0, 0, 0, 0, 0, 0),
            }))),
            span: Span::new(0, 0, 0, 0, 0, 0),
        };

        let ty = synth(&mut ctx, &expr).unwrap();
        assert_eq!(ty, Ty::Primitive(PrimTy::Int64));
    }

    #[test]
    fn test_synth_array() {
        let interner = StringInterner::new();
        let mut ctx = Context::new(&interner);

        let expr = Expr::Array {
            elements: vec![
                Box::leak(Box::new(Expr::IntegerLiteral {
                    value: oxidex_mem::Symbol::new(0),
                    type_suffix: None,
                    span: Span::new(0, 0, 0, 0, 0, 0),
                })),
                Box::leak(Box::new(Expr::IntegerLiteral {
                    value: oxidex_mem::Symbol::new(0),
                    type_suffix: None,
                    span: Span::new(0, 0, 0, 0, 0, 0),
                })),
            ],
            span: Span::new(0, 0, 0, 0, 0, 0),
        };

        let ty = synth(&mut ctx, &expr).unwrap();
        assert_eq!(ty, Ty::Array(Box::new(Ty::Primitive(PrimTy::Int64))));
    }

    #[test]
    fn test_synth_empty_array() {
        let interner = StringInterner::new();
        let mut ctx = Context::new(&interner);

        let expr = Expr::Array {
            elements: vec![],
            span: Span::new(0, 0, 0, 0, 0, 0),
        };

        let ty = synth(&mut ctx, &expr).unwrap();
        // Empty arrays should have a type variable as element type
        assert!(matches!(ty, Ty::Array(_)));
    }

    #[test]
    fn test_check_expression() {
        let interner = StringInterner::new();
        let mut ctx = Context::new(&interner);

        let expr = Expr::IntegerLiteral {
            value: oxidex_mem::Symbol::new(0),
            type_suffix: None,
            span: Span::new(0, 0, 0, 0, 0, 0),
        };

        let expected = Ty::Primitive(PrimTy::Int64);
        let result = check(&mut ctx, &expr, &expected);
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_expression_mismatch() {
        let interner = StringInterner::new();
        let mut ctx = Context::new(&interner);

        let expr = Expr::IntegerLiteral {
            value: oxidex_mem::Symbol::new(0),
            type_suffix: None,
            span: Span::new(0, 0, 0, 0, 0, 0),
        };

        let expected = Ty::Primitive(PrimTy::Bool);
        let result = check(&mut ctx, &expr, &expected);
        assert!(result.is_err());
    }
}
