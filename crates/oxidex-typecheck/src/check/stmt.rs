//! Statement type checking.
//!
//! This module implements type checking for statements, including:
//! - Let and mut bindings
//! - Return statements
//! - Assignments with mutability checking
//! - Control flow statements (if, match, for, while)
//! - Expression statements

use crate::error::Result;
use crate::infer::Context;
use crate::types::{PrimTy, Ty};
use oxidex_syntax::ast::stmt::Stmt;
use oxidex_syntax::{Span, Spanned};

/// Type check a statement.
///
/// Statements don't produce values (except expression statements, which we ignore),
/// so this function returns `Result<()>` to indicate success or failure.
pub fn check_stmt<'ctx>(ctx: &mut Context<'ctx>, stmt: &Stmt<'ctx>) -> Result<()> {
    match stmt {
        // Let binding: `let x: Type = expr;`
        Stmt::Let {
            name,
            type_annotation,
            init,
            span,
        } => {
            // Type check initializer if present
            let ty_init = if let Some(init_expr) = init {
                Some(super::expr::synth(ctx, init_expr)?)
            } else {
                None
            };

            // If there's a type annotation, convert it and unify with initializer type
            if let (Some(ty_init), Some(type_anno)) = (&ty_init, type_annotation) {
                let ty_anno = super::ty::ast_to_ty(ctx, type_anno)?;
                ctx.unify(&ty_init, &ty_anno, *span)?;
            }

            // Bind the variable in the environment
            use crate::context::Scheme;
            let ty = ty_init.unwrap_or_else(|| {
                // If there's a type annotation but no initializer, use that
                if let Some(type_anno) = type_annotation {
                    super::ty::ast_to_ty(ctx, type_anno).unwrap_or_else(|_| Ty::TypeVar(ctx.fresh_var()))
                } else {
                    Ty::TypeVar(ctx.fresh_var())
                }
            });
            let scheme = Scheme::mono(ty);
            ctx.env.bind(*name, scheme);

            Ok(())
        }

        // Mutable binding: `mut x: Type = expr;`
        Stmt::Mut {
            name,
            type_annotation,
            init,
            span,
        } => {
            // Type check initializer if present
            let ty_init = if let Some(init_expr) = init {
                Some(super::expr::synth(ctx, init_expr)?)
            } else {
                None
            };

            // If there's a type annotation, convert it and unify with initializer type
            if let (Some(ty_init), Some(type_anno)) = (&ty_init, type_annotation) {
                let ty_anno = super::ty::ast_to_ty(ctx, type_anno)?;
                ctx.unify(&ty_init, &ty_anno, *span)?;
            }

            // Bind the variable as mutable in the environment
            use crate::context::Scheme;
            let ty = ty_init.unwrap_or_else(|| {
                // If there's a type annotation but no initializer, use that
                if let Some(type_anno) = type_annotation {
                    super::ty::ast_to_ty(ctx, type_anno).unwrap_or_else(|_| Ty::TypeVar(ctx.fresh_var()))
                } else {
                    Ty::TypeVar(ctx.fresh_var())
                }
            });
            let scheme = Scheme::mono(ty);
            ctx.env.bind_mut(*name, scheme, true);

            Ok(())
        }

        // Return statement: `return expr;` or `return;`
        Stmt::Return { value, span } => {
            // Type check return value if present
            if let Some(return_expr) = value {
                let ty_return = super::expr::synth(ctx, return_expr)?;

                // CRITICAL: Check against function's return type
                if let Some(expected_ret) = ctx.get_return_type() {
                    let expected_clone = expected_ret.clone();
                    ctx.unify(&ty_return, &expected_clone, *span)?;
                }
            } else {
                // Empty return is equivalent to `return ()`
                // CRITICAL: Check against function's return type
                if let Some(expected_ret) = ctx.get_return_type() {
                    let expected_clone = expected_ret.clone();
                    ctx.unify(&Ty::Primitive(PrimTy::Unit), &expected_clone, *span)?;
                }
            }

            Ok(())
        }

        // If statement: `if cond { then } else { else }`
        Stmt::If {
            condition,
            then_branch,
            else_branch,
            span,
        } => {
            // Condition must be boolean
            let ty_cond = super::expr::synth(ctx, condition)?;
            ctx.unify(&ty_cond, &Ty::Primitive(crate::types::PrimTy::Bool), *span)?;

            // Type check both branches
            super::expr::synth(ctx, then_branch)?;

            if let Some(else_br) = else_branch {
                super::expr::synth(ctx, else_br)?;
            }

            Ok(())
        }

        // Guard statement: `guard condition else { block }`
        Stmt::Guard {
            condition,
            else_branch,
            span,
        } => {
            // Condition must be boolean
            let ty_cond = super::expr::synth(ctx, condition)?;
            ctx.unify(&ty_cond, &Ty::Primitive(crate::types::PrimTy::Bool), *span)?;

            // Type check else branch
            super::expr::synth(ctx, else_branch)?;

            Ok(())
        }

        // Match statement: `match value { pattern => expr }`
        Stmt::Match {
            scrutinee, arms, span,
        } => {
            // Type check scrutinee
            let _ty_scrut = super::expr::synth(ctx, scrutinee)?;

            // All arms must have the same type
            let mut arm_types = Vec::new();

            for arm in arms {
                // Type check body
                let ty_body = super::expr::synth(ctx, arm.body)?;
                arm_types.push(ty_body);
            }

            // All arms must have the same type
            if let Some(first_ty) = arm_types.first() {
                for ty_arm in &arm_types[1..] {
                    ctx.unify(first_ty, ty_arm, *span)?;
                }
            }

            Ok(())
        }

        // For loop: `for pattern in iter { body }`
        Stmt::ForLoop {
            pattern,
            iter,
            body,
            span: _,
        } => {
            // Type check iterator (should be a collection type)
            let _ty_iter = super::expr::synth(ctx, iter)?;

            // TODO: Check pattern against iterator element type
            let _ = pattern;

            // Type check body
            super::expr::synth(ctx, body)?;

            Ok(())
        }

        // While loop: `while condition { body }`
        Stmt::WhileLoop {
            condition, body, span,
        } => {
            // Condition must be boolean
            let ty_cond = super::expr::synth(ctx, condition)?;
            ctx.unify(&ty_cond, &Ty::Primitive(crate::types::PrimTy::Bool), *span)?;

            // Type check body
            super::expr::synth(ctx, body)?;

            Ok(())
        }

        // Assignment: `target = value;`
        Stmt::Assign {
            target, value, span,
        } => {
            // CRITICAL: Check if target is a valid lvalue
            use oxidex_syntax::ast::expr::Expr;
            match target {
                // Identifier: check if mutable
                Expr::Identifier(sym) => {
                    let name = ctx.interner.resolve(*sym).unwrap_or("");
                    if !ctx.env.is_mutable(*sym) {
                        return Err(crate::error::TypeError::AssignToImmutable {
                            name: name.to_string(),
                            span: *span,
                        });
                    }
                }

                // Field access: p.x = value (need to check if p is mutable and field is mutable)
                Expr::Field { .. } => {
                    // TODO: For now, we'll allow field access assignments
                    // In the future, we need to check if the base object is mutable
                    // and if the specific field is declared as mutable
                }

                // Index access: arr[i] = value (need to check if array is mutable)
                Expr::Index { .. } => {
                    // TODO: For now, we'll allow index assignments
                    // In the future, we need to check if the collection is mutable
                }

                // Invalid assignment targets (literals)
                Expr::IntegerLiteral { .. }
                | Expr::FloatLiteral { .. }
                | Expr::StringLiteral { .. }
                | Expr::BoolLiteral { .. }
                | Expr::Nil { .. } => {
                    return Err(crate::error::TypeError::InvalidAssignmentTarget {
                        span: *span,
                    });
                }

                Expr::Binary { .. } | Expr::Unary { .. } | Expr::Call { .. } | Expr::MethodCall { .. } => {
                    return Err(crate::error::TypeError::InvalidAssignmentTarget {
                        span: target.span(),
                    });
                }

                _ => {
                    // Other expressions are not valid lvalues
                    return Err(crate::error::TypeError::InvalidAssignmentTarget {
                        span: target.span(),
                    });
                }
            }

            // Type check target
            let ty_target = super::expr::synth(ctx, target)?;

            // Type check value
            let ty_value = super::expr::synth(ctx, value)?;

            // Unify target and value types
            ctx.unify(&ty_target, &ty_value, *span)?;

            Ok(())
        }

        // Expression statement: `expr;`
        Stmt::Expr { expr, span: _ } => {
            // Type check the expression (ignore the result)
            super::expr::synth(ctx, expr)?;
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxidex_mem::StringInterner;
    use crate::types::PrimTy;
    use oxidex_syntax::ast::expr::Expr;
    use oxidex_syntax::ast::stmt::Stmt;
    use oxidex_syntax::Span;

    #[test]
    fn test_check_let_binding() {
        let interner = StringInterner::new();
        let mut ctx = Context::new(&interner);

        // TODO: Create a proper let binding statement
        // For now, this test just checks that the function exists
        // and doesn't panic
    }

    #[test]
    fn test_assignment_to_immutable_errors() {
        let mut interner = StringInterner::new();
        let x_sym = interner.intern("x");
        let mut ctx = Context::new(&interner);

        // Create an immutable binding
        let scheme = crate::context::Scheme::mono(crate::types::Ty::Primitive(PrimTy::Int64));
        ctx.env.bind(x_sym, scheme);

        // Try to assign to it
        let span = Span::new(0, 0, 0, 0, 0, 0);
        let assign = Stmt::Assign {
            target: &Expr::Identifier(x_sym),
            value: &Expr::IntegerLiteral {
                value: oxidex_mem::Symbol::new(0),
                type_suffix: None,
                span,
            },
            span,
        };

        let result = check_stmt(&mut ctx, &assign);
        assert!(result.is_err());
        match result {
            Err(crate::error::TypeError::AssignToImmutable { name, .. }) => {
                assert_eq!(name, "x");
            }
            _ => panic!("Expected AssignToImmutable error"),
        }
    }

    #[test]
    fn test_assignment_to_mutable_succeeds() {
        let mut interner = StringInterner::new();
        let x_sym = interner.intern("x");
        let mut ctx = Context::new(&interner);

        // Create a mutable binding
        let scheme = crate::context::Scheme::mono(crate::types::Ty::Primitive(PrimTy::Int64));
        ctx.env.bind_mut(x_sym, scheme, true); // Mark as mutable

        // Assign to it
        let span = Span::new(0, 0, 0, 0, 0, 0);
        let assign = Stmt::Assign {
            target: &Expr::Identifier(x_sym),
            value: &Expr::IntegerLiteral {
                value: oxidex_mem::Symbol::new(0),
                type_suffix: None,
                span,
            },
            span,
        };

        let result = check_stmt(&mut ctx, &assign);
        assert!(result.is_ok());
    }
}
