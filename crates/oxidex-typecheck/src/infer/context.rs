//! Type checking context for inference.
//!
//! This module provides the main context for type checking, combining
//! the type environment, substitution, and symbol interner.

use crate::context::{Scheme, Subst, TypeEnv, TypeRegistry};
use crate::error::Result;
use crate::infer::Unifier;
use crate::types::Ty;
use oxidex_mem::StringInterner;
use oxidex_syntax::Span;

/// Main type checking context.
pub struct Context<'ctx> {
    /// String interner (shared with parser)
    pub interner: &'ctx StringInterner,

    /// Type environment (symbol table)
    pub env: TypeEnv,

    /// Type registry (struct/enum definitions)
    pub types: TypeRegistry,

    /// Unifier for type unification
    pub unifier: Unifier<'ctx>,

    /// Current class/struct being checked (for Self type)
    pub current_self: Option<CurrentSelf>,

    /// Expected return type for current function (None if not in function)
    pub return_type: Option<Ty>,

    /// Generic parameters in scope (mapping from name to type variable)
    pub generic_params: std::collections::HashMap<oxidex_mem::Symbol, u32>,
}

/// Information about the current Self type.
#[derive(Debug, Clone)]
pub struct CurrentSelf {
    /// Name of the current type
    pub name: String,
    /// Type parameters if generic
    pub type_params: Vec<String>,
}

impl<'ctx> Context<'ctx> {
    /// Create a new type checking context.
    pub fn new(interner: &'ctx StringInterner) -> Self {
        let subst = Subst::new();
        let unifier = Unifier::new(subst);

        Self {
            interner,
            env: TypeEnv::new(),
            types: TypeRegistry::new(),
            unifier,
            current_self: None,
            return_type: None,
            generic_params: std::collections::HashMap::new(),
        }
    }

    /// Set the expected return type for the current function.
    pub fn set_return_type(&mut self, ty: Ty) {
        self.return_type = Some(ty);
    }

    /// Get the expected return type (if in a function).
    pub fn get_return_type(&self) -> Option<&Ty> {
        self.return_type.as_ref()
    }

    /// Clear the return type (when exiting a function).
    pub fn clear_return_type(&mut self) {
        self.return_type = None;
    }

    /// Push generic parameters into scope.
    ///
    /// This should be called when entering a generic function/struct/enum.
    /// Each generic parameter is mapped to a fresh type variable.
    pub fn push_generic_params(&mut self, params: &[oxidex_mem::Symbol]) {
        for &param in params {
            let type_var = self.fresh_var();
            self.generic_params.insert(param, type_var);
        }
    }

    /// Pop generic parameters from scope.
    ///
    /// This should be called when exiting a generic declaration.
    pub fn pop_generic_params(&mut self, params: &[oxidex_mem::Symbol]) {
        for &param in params {
            self.generic_params.remove(&param);
        }
    }

    /// Look up a generic parameter.
    ///
    /// Returns the type variable if this is a generic parameter in scope.
    pub fn lookup_generic_param(&self, name: oxidex_mem::Symbol) -> Option<u32> {
        self.generic_params.get(&name).copied()
    }

    /// Check if a symbol is a generic parameter.
    pub fn is_generic_param(&self, name: oxidex_mem::Symbol) -> bool {
        self.generic_params.contains_key(&name)
    }

    /// Enter a new scope.
    pub fn new_scope(&mut self) {
        self.env.new_scope();
    }

    /// Exit the current scope.
    pub fn pop_scope(&mut self) {
        self.env.pop_scope();
    }

    /// Get the current substitution.
    pub fn subst(&mut self) -> &mut Subst {
        &mut self.unifier.subst
    }

    /// Unify two types.
    pub fn unify(&mut self, ty1: &Ty, ty2: &Ty, span: Span) -> Result<()> {
        self.unifier.unify(ty1, ty2, span)
    }

    /// Create a fresh type variable.
    pub fn fresh_var(&mut self) -> u32 {
        self.env.fresh_var()
    }

    /// Look up a symbol in the environment.
    pub fn lookup(&self, name: &str) -> Option<&Scheme> {
        use oxidex_mem::Symbol;
        // For now, we'll use a simple hash-based approach
        // In a real implementation, we'd look up the symbol in the interner
        // This is a placeholder for the actual symbol lookup
        self.env.lookup(Symbol::new(0))
    }
}

impl<'ctx> AsMut<TypeEnv> for Context<'ctx> {
    fn as_mut(&mut self) -> &mut TypeEnv {
        &mut self.env
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_creation() {
        let interner = StringInterner::new();
        let ctx = Context::new(&interner);
        assert_eq!(ctx.env.depth(), 1);
    }

    #[test]
    fn test_scope_management() {
        let interner = StringInterner::new();
        let mut ctx = Context::new(&interner);

        assert_eq!(ctx.env.depth(), 1);

        ctx.new_scope();
        assert_eq!(ctx.env.depth(), 2);

        ctx.pop_scope();
        assert_eq!(ctx.env.depth(), 1);
    }

    #[test]
    fn test_fresh_var() {
        let interner = StringInterner::new();
        let mut ctx = Context::new(&interner);

        let v1 = ctx.fresh_var();
        let v2 = ctx.fresh_var();

        assert_eq!(v1, 0);
        assert_eq!(v2, 1);
    }
}
