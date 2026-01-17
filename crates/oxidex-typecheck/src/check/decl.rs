//! Declaration type checking.
//!
//! This module implements type checking for top-level declarations:
//! - Functions (two-pass: signatures then bodies)
//! - Structs and Classes (field type checking)
//! - Enums (variant validation)
//! - Protocols (method signature validation)
//! - Impl blocks (method implementation checking)
//! - Constants and Statics
//! - Type aliases

use crate::error::Result;
use crate::infer::Context;
use crate::types::{PrimTy, Ty};
use oxidex_syntax::ast::decl::Decl;
use oxidex_syntax::Span;

/// Type check a declaration.
///
/// Declarations are type-checked in two passes for functions:
/// - Pass 1: Collect all signatures
/// - Pass 2: Check bodies with complete environment
pub fn check_decl<'ctx>(ctx: &mut Context<'ctx>, decl: &Decl<'ctx>) -> Result<()> {
    match decl {
        // Function declaration
        Decl::Fn {
            name,
            generics,
            params,
            return_type,
            body,
            span,
            is_mut: _,
            is_init: _,
            is_static: _,
            visibility: _,
        } => {
            // Enter a new scope for the function
            ctx.new_scope();

            // Push generic parameters into scope
            ctx.push_generic_params(generics);

            // Set the return type if specified
            if let Some(ret_type) = return_type {
                let ty_ret = super::ty::ast_to_ty(ctx, ret_type)?;
                ctx.set_return_type(ty_ret);
            }

            // Type check parameters and bind them in the environment
            for param in params {
                // Convert the Type annotation to Ty
                let ty_param = super::ty::ast_to_ty(ctx, &param.type_annotation)?;

                // Bind the parameter in the environment
                use crate::context::Scheme;
                let scheme = Scheme::mono(ty_param);
                ctx.env.bind(param.name, scheme);
            }

            // Type check the function body
            let ty_body = super::expr::synth(ctx, body)?;

            // If there's a return type annotation, unify with body type
            if let Some(_) = return_type {
                // Already handled by set_return_type + return statement validation
                let _ = ty_body;
            }

            // Clear the return type
            ctx.clear_return_type();

            // Pop generic parameters from scope
            ctx.pop_generic_params(generics);

            // Exit the function scope
            ctx.pop_scope();

            Ok(())
        }

        // Struct declaration
        Decl::Struct {
            name,
            generics,
            fields,
            protocols,
            span,
            visibility: _,
        } => {
            // Push generic parameters into scope
            ctx.push_generic_params(generics);

            // Convert field type annotations and build struct info
            let mut field_infos = Vec::new();
            for field in fields {
                let ty_field = super::ty::ast_to_ty(ctx, &field.type_annotation)?;
                field_infos.push(crate::context::FieldInfo {
                    name: field.name,
                    ty: ty_field,
                });
            }

            // Register the struct definition (without methods for now)
            let struct_info = crate::context::StructInfo {
                name: *name,
                fields: field_infos,
                methods: vec![], // Methods will be added in impl blocks
                generics: generics.clone(),
            };
            ctx.types.register_struct(struct_info);

            // Pop generic parameters from scope
            ctx.pop_generic_params(generics);

            // TODO: Register protocol conformances
            let _ = (name, protocols, span);

            Ok(())
        }

        // Class declaration
        Decl::Class {
            name,
            generics,
            superclass,
            fields,
            protocols,
            span,
            visibility: _,
        } => {
            // Push generic parameters into scope
            ctx.push_generic_params(generics);

            // Type check all fields
            let mut field_infos = Vec::new();
            for field in fields {
                let ty_field = super::ty::ast_to_ty(ctx, &field.type_annotation)?;
                field_infos.push(crate::context::FieldInfo {
                    name: field.name,
                    ty: ty_field,
                });
            }

            // Process superclass
            let super_sym = if let Some(super_path) = superclass {
                if super_path.len() == 1 {
                    Some(super_path[0])
                } else {
                    // TODO: Handle paths like Module::SuperClass
                    None
                }
            } else {
                None
            };

            // Register the class definition
            let class_info = crate::context::ClassInfo {
                name: *name,
                superclass: super_sym,
                fields: field_infos,
                methods: vec![],
                generics: generics.clone(),
            };
            ctx.types.register_class(class_info);

            // Pop generic parameters from scope
            ctx.pop_generic_params(generics);

            // TODO: Register protocol conformances
            let _ = (name, protocols, span);

            Ok(())
        }

        // Enum declaration
        Decl::Enum {
            name,
            generics,
            variants,
            methods,
            protocols,
            span,
            visibility: _,
        } => {
            // Push generic parameters into scope
            ctx.push_generic_params(generics);

            // Convert variant information
            let mut variant_infos = Vec::new();
            for variant in variants {
                match variant {
                    oxidex_syntax::ast::decl::EnumVariant::Unit { name, span: _ } => {
                        variant_infos.push(crate::context::EnumVariantInfo {
                            name: *name,
                            payload: None,
                        });
                    }
                    oxidex_syntax::ast::decl::EnumVariant::Tuple {
                        name,
                        fields,
                        span: _,
                    } => {
                        // For tuple variants, we create a tuple type for the payload
                        let mut field_types = Vec::new();
                        for ty in fields {
                            let ty_field = super::ty::ast_to_ty(ctx, ty)?;
                            field_types.push(ty_field);
                        }

                        let payload = if field_types.len() == 1 {
                            Some(field_types.into_iter().next().unwrap())
                        } else {
                            Some(Ty::Tuple(field_types))
                        };

                        variant_infos.push(crate::context::EnumVariantInfo {
                            name: *name,
                            payload,
                        });
                    }
                    oxidex_syntax::ast::decl::EnumVariant::Struct {
                        name,
                        fields,
                        span: _,
                    } => {
                        // For struct variants, we create a struct type for the payload
                        let mut field_types = Vec::new();
                        for field in fields {
                            let ty_field = super::ty::ast_to_ty(ctx, &field.type_annotation)?;
                            field_types.push(ty_field);
                        }
                        let payload = Some(Ty::Tuple(field_types));
                        variant_infos.push(crate::context::EnumVariantInfo {
                            name: *name,
                            payload,
                        });
                    }
                }
            }

            // Register the enum definition (methods will be added below)
            let enum_info = crate::context::EnumInfo {
                name: *name,
                variants: variant_infos,
                methods: vec![], // Methods will be collected during type checking
                generics: generics.clone(),
            };
            ctx.types.register_enum(enum_info);

            // Pop generic parameters from scope
            ctx.pop_generic_params(generics);

            // Type check methods defined in the enum body
            for method in methods {
                check_fn_decl(ctx, method)?;
            }

            // TODO: Register protocol conformances
            let _ = (name, protocols, span);

            Ok(())
        }

        // Protocol declaration
        Decl::Protocol {
            name,
            generics,
            methods,
            span,
            visibility: _,
        } => {
            // Push generic parameters into scope
            ctx.push_generic_params(generics);

            // Collect method signatures
            let mut method_infos = Vec::new();
            for method in methods {
                // Convert parameter types
                let mut param_types = Vec::new();
                for param in &method.params {
                    let ty_param = super::ty::ast_to_ty(ctx, &param.type_annotation)?;
                    param_types.push(ty_param);
                }

                // Convert return type
                let ty_return = if let Some(ret_type) = &method.return_type {
                    super::ty::ast_to_ty(ctx, ret_type)?
                } else {
                    Ty::Primitive(PrimTy::Unit)
                };

                method_infos.push(crate::context::ProtocolMethodInfo {
                    name: method.name,
                    params: param_types,
                    return_type: ty_return,
                });
            }

            // Register the protocol definition
            let protocol_info = crate::context::ProtocolInfo {
                name: *name,
                methods: method_infos,
                generics: generics.clone(),
            };
            ctx.types.register_protocol(protocol_info);

            // Pop generic parameters from scope
            ctx.pop_generic_params(generics);

            Ok(())
        }

        // Impl block
        Decl::Impl {
            type_path,
            protocol,
            methods,
            span,
        } => {
            // Look up the type being implemented
            if type_path.len() != 1 {
                // TODO: Handle paths like Module::Type
                return Ok(());
            }

            let type_name = type_path[0];

            // If implementing a protocol, validate conformance
            if let Some(proto_path) = protocol {
                if proto_path.len() != 1 {
                    // TODO: Handle paths like Module::Protocol
                    return Ok(());
                }

                let proto_name = proto_path[0];

                // Look up the protocol
                if let Some(protocol_info) = ctx.types.lookup_protocol(proto_name) {
                    // Clone the protocol methods to avoid holding the borrow
                    let protocol_methods = protocol_info.methods.clone();

                    // Collect methods that are implemented
                    let mut implemented_methods = std::collections::HashSet::new();
                    for method in methods {
                        if let Some(method_name) = method.name {
                            implemented_methods.insert(method_name);
                        }
                    }

                    // Check that all required methods are implemented
                    for required_method in &protocol_methods {
                        if !implemented_methods.contains(&required_method.name) {
                            return Err(crate::error::TypeError::MissingProtocolMethod {
                                ty: ctx.interner.resolve(type_name).unwrap_or("").to_string(),
                                protocol: ctx.interner.resolve(proto_name).unwrap_or("").to_string(),
                                method: ctx.interner.resolve(required_method.name).unwrap_or("").to_string(),
                                span: *span,
                            });
                        }
                    }

                    // Now validate each method's signature matches the protocol
                    for method in methods {
                        // Find the corresponding protocol method
                        if let Some(method_name) = method.name {
                            if let Some(proto_method) = protocol_methods.iter()
                                .find(|m| m.name == method_name)
                            {
                                // Clone the protocol method data to avoid holding the borrow
                                let proto_params = proto_method.params.clone();
                                let proto_return = proto_method.return_type.clone();

                                // Enter a new scope for the method
                                ctx.new_scope();

                                // Push generic parameters
                                ctx.push_generic_params(&method.generics);

                                // Check parameter count matches
                                if method.params.len() != proto_params.len() {
                                    // Build the found type by converting parameter types
                                    let mut found_params = Vec::new();
                                    for param in &method.params {
                                        let ty_param = super::ty::ast_to_ty(ctx, &param.type_annotation)?;
                                        found_params.push(ty_param);
                                    }

                                    let found_return = if let Some(ret_type) = &method.return_type {
                                        super::ty::ast_to_ty(ctx, ret_type)?
                                    } else {
                                        Ty::Primitive(PrimTy::Unit)
                                    };

                                    return Err(crate::error::TypeError::Mismatch {
                                        expected: Ty::Function {
                                            params: proto_params.clone(),
                                            return_type: Box::new(proto_return.clone()),
                                            labels: vec![None; proto_params.len()],
                                        },
                                        found: Ty::Function {
                                            params: found_params,
                                            return_type: Box::new(found_return),
                                            labels: vec![None; method.params.len()],
                                        },
                                        span: method.span,
                                    });
                                }

                                // Validate each parameter type matches
                                for (param, proto_param_ty) in method.params.iter().zip(&proto_params) {
                                    let ty_param = super::ty::ast_to_ty(ctx, &param.type_annotation)?;
                                    ctx.unify(&ty_param, proto_param_ty, param.span)?;
                                }

                                // Validate return type matches
                                if let Some(ret_type) = &method.return_type {
                                    let ty_return = super::ty::ast_to_ty(ctx, ret_type)?;
                                    ctx.unify(&ty_return, &proto_return, *span)?;
                                }

                                // Pop generic parameters
                                ctx.pop_generic_params(&method.generics);

                                // Exit the method scope
                                ctx.pop_scope();
                            }  // Close if let Some(proto_method)
                        }  // Close if let Some(method_name)
                    }  // Close for method in methods
                }  // Close if let Some(protocol_info)
            }  // Close if let Some(proto_path)


            // Type check all methods in the impl block
            for method in methods {
                check_fn_decl(ctx, method)?;
            }

            Ok(())
        }

        // Constant declaration
        Decl::Const {
            name,
            type_annotation,
            value,
            span,
            visibility: _,
        } => {
            // Type check the value
            let ty_value = super::expr::synth(ctx, value)?;

            // If there's a type annotation, unify with value type
            // TODO: Convert the Type annotation to Ty
            let _ = (ty_value, type_annotation, span);

            // TODO: Bind the constant in the environment
            let _ = name;

            Ok(())
        }

        // Static declaration
        Decl::Static {
            name,
            type_annotation,
            init,
            mutable: _,
            span,
            visibility: _,
        } => {
            // Type check the initializer if present
            let ty_init = if let Some(init_expr) = init {
                Some(super::expr::synth(ctx, init_expr)?)
            } else {
                None
            };

            // If there's a type annotation, unify with initializer type
            // TODO: Convert the Type annotation to Ty
            let _ = (ty_init, type_annotation, span);

            // TODO: Bind the static variable in the environment
            let _ = name;

            Ok(())
        }

        // Type alias
        Decl::TypeAlias {
            name,
            generics,
            target,
            span,
            visibility: _,
        } => {
            // TODO: Process generic parameters
            let _ = generics;

            // TODO: Convert the target type to Ty and register the alias
            let _ = (name, target, span);

            Ok(())
        }
    }
}

/// Type check a function declaration (used in impl blocks and enums).
fn check_fn_decl<'ctx>(ctx: &mut Context<'ctx>, decl: &oxidex_syntax::ast::decl::FnDecl) -> Result<()> {
    // Enter a new scope for the function
    ctx.new_scope();

    // Push generic parameters into scope
    ctx.push_generic_params(&decl.generics);

    // Type check parameters and bind them in the environment
    for param in &decl.params {
        // Convert the Type annotation to Ty
        let ty_param = super::ty::ast_to_ty(ctx, &param.type_annotation)?;

        // Bind the parameter in the environment
        use crate::context::Scheme;
        let scheme = Scheme::mono(ty_param);
        ctx.env.bind(param.name, scheme);
    }

    // Note: FnDecl doesn't have a body, it's just a signature
    // The actual body is in Decl::Fn

    // Pop generic parameters from scope
    ctx.pop_generic_params(&decl.generics);

    // Exit the function scope
    ctx.pop_scope();

    Ok(())
}

/// First pass: collect all function signatures.
///
/// This is used to support mutual recursion and forward references.
pub fn collect_signatures<'ctx>(ctx: &mut Context<'ctx>, decls: &[Decl<'ctx>]) -> Result<()> {
    for decl in decls {
        match decl {
            Decl::Fn {
                name, generics, params, return_type, ..
            } => {
                // TODO: Build the function type
                let _ = (name, generics, params, return_type);
                // For now, just create a fresh type variable
                let ty = Ty::TypeVar(ctx.fresh_var());

                // Bind the function name in the environment
                use crate::context::Scheme;
                let scheme = Scheme::mono(ty);
                let _ = (*name, scheme);
                // ctx.env.bind(*name, scheme);
            }
            _ => {
                // Other declarations don't need signature collection
            }
        }
    }
    Ok(())
}

/// Second pass: check all declaration bodies.
///
/// This runs after signatures are collected, so all declarations are visible.
pub fn check_bodies<'ctx>(ctx: &mut Context<'ctx>, decls: &[Decl<'ctx>]) -> Result<()> {
    for decl in decls {
        check_decl(ctx, decl)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxidex_mem::StringInterner;

    #[test]
    fn test_collect_signatures() {
        let interner = StringInterner::new();
        let mut ctx = Context::new(&interner);

        // TODO: Create test declarations
        // For now, this test just checks that the function exists
        let decls = vec![];
        assert!(collect_signatures(&mut ctx, &decls).is_ok());
    }

    #[test]
    fn test_check_bodies() {
        let interner = StringInterner::new();
        let mut ctx = Context::new(&interner);

        // TODO: Create test declarations
        // For now, this test just checks that the function exists
        let decls = vec![];
        assert!(check_bodies(&mut ctx, &decls).is_ok());
    }
}
