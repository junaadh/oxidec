//! Type environment for lexical scoping.
//!
//! This module implements the type environment, which maps symbols to type schemes.
//! The environment is organized as a stack of scopes for lexical scoping.
//!
//! # Design
//!
//! - **Stack of scopes**: Each scope is a map from symbols to type schemes
//! - **Type schemes**: A type with possibly universally quantified type variables
//! - **Lexical scoping**: Inner scopes shadow outer scopes
//!
//! # Example
//!
//! ```ignore
//! let mut env = TypeEnv::new();
//!
//! // Global scope
//! env.bind(symbol!("foo"), Scheme::mono(Ty::Primitive(PrimTy::Int64)));
//!
//! // New scope
//! env.new_scope();
//! env.bind(symbol!("bar"), Scheme::mono(Ty::Primitive(PrimTy::Bool)));
//!
//! // Lookup finds bar in inner scope
//! assert!(env.lookup(symbol!("bar")).is_some());
//!
//! // Pop scope
//! env.pop_scope();
//! assert!(env.lookup(symbol!("bar")).is_none());
//! assert!(env.lookup(symbol!("foo")).is_some());
//! ```

use crate::context::subst::Subst;
use crate::types::Ty;
use oxidex_mem::Symbol;
use std::collections::HashMap;
use std::collections::HashSet;

/// Type scheme: a type with possible universally quantified variables.
///
/// A type scheme represents a polymorphic type where some type variables
/// are universally quantified. For example:
///
/// - `forall a. a -> a` is the identity function type
/// - `Int -> Int` is a monomorphic type (no quantified variables)
///
/// # Example
///
/// ```ignore
/// // Monomorphic type: Int
/// let scheme = Scheme::mono(Ty::Primitive(PrimTy::Int64));
/// assert_eq!(scheme.vars.len(), 0);
///
/// // Polymorphic type: forall a. a -> a
/// let var = 0;
/// let scheme = Scheme {
///     vars: vec![var],
///     ty: Ty::Function {
///         params: vec![Ty::TypeVar(var)],
///         return_type: Box::new(Ty::TypeVar(var)),
///         labels: vec![None],
///     },
/// };
/// ```
#[derive(Debug, Clone)]
pub struct Scheme {
    /// Universally quantified type variables.
    ///
    /// These are the variables that are "bound" by the forall quantifier.
    pub vars: Vec<u32>,

    /// The type itself (may contain references to `vars`).
    pub ty: Ty,
}

impl Scheme {
    /// Create a monomorphic type scheme (no quantified variables).
    ///
    /// # Example
    ///
    /// ```ignore
    /// let scheme = Scheme::mono(Ty::Primitive(PrimTy::Int64));
    /// assert_eq!(scheme.vars.len(), 0);
    /// ```
    pub fn mono(ty: Ty) -> Self {
        Self {
            vars: Vec::new(),
            ty,
        }
    }

    /// Create a polymorphic type scheme.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let scheme = Scheme::poly(vec![0, 1], Ty::Function { ... });
    /// ```
    pub fn poly(vars: Vec<u32>, ty: Ty) -> Self {
        Self { vars, ty }
    }

    /// Instantiate this scheme with fresh type variables.
    ///
    /// This replaces each quantified variable with a fresh type variable.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let scheme = Scheme::poly(vec![0], Ty::TypeVar(0));
    /// let (ty, subst) = scheme.instantiate(&mut ctx);
    /// // ty is now Ty::TypeVar(fresh_var)
    /// ```
    pub fn instantiate(&self, subst: &mut Subst) -> Ty {
        if self.vars.is_empty() {
            // Monomorphic: just clone the type
            return self.ty.clone();
        }

        // Polymorphic: replace each quantified var with a fresh var
        let mapping: HashMap<u32, u32> =
            self.vars.iter().map(|&v| (v, subst.fresh_var())).collect();

        self.instantiate_with_mapping(&mapping, subst)
    }

    /// Instantiate with a specific mapping (for testing).
    fn instantiate_with_mapping(
        &self,
        mapping: &HashMap<u32, u32>,
        subst: &mut Subst,
    ) -> Ty {
        self.replace_vars(&self.ty, mapping, subst)
    }

    /// Replace type variables according to a mapping.
    fn replace_vars(
        &self,
        ty: &Ty,
        mapping: &HashMap<u32, u32>,
        subst: &mut Subst,
    ) -> Ty {
        match ty {
            Ty::TypeVar(v) => {
                if let Some(&new_var) = mapping.get(v) {
                    Ty::TypeVar(new_var)
                } else {
                    Ty::TypeVar(*v)
                }
            }

            Ty::Struct { name, type_args } => Ty::Struct {
                name: *name,
                type_args: type_args
                    .iter()
                    .map(|t| self.replace_vars(t, mapping, subst))
                    .collect(),
            },

            Ty::Class { name, type_args } => Ty::Class {
                name: *name,
                type_args: type_args
                    .iter()
                    .map(|t| self.replace_vars(t, mapping, subst))
                    .collect(),
            },

            Ty::Enum { name, type_args } => Ty::Enum {
                name: *name,
                type_args: type_args
                    .iter()
                    .map(|t| self.replace_vars(t, mapping, subst))
                    .collect(),
            },

            Ty::Protocol { name, type_args } => Ty::Protocol {
                name: *name,
                type_args: type_args
                    .iter()
                    .map(|t| self.replace_vars(t, mapping, subst))
                    .collect(),
            },

            Ty::Tuple(types) => Ty::Tuple(
                types
                    .iter()
                    .map(|t| self.replace_vars(t, mapping, subst))
                    .collect(),
            ),

            Ty::Function {
                params,
                return_type,
                labels,
            } => Ty::Function {
                params: params
                    .iter()
                    .map(|p| self.replace_vars(p, mapping, subst))
                    .collect(),
                return_type: Box::new(self.replace_vars(
                    return_type,
                    mapping,
                    subst,
                )),
                labels: labels.clone(),
            },

            Ty::Array(inner) => {
                Ty::Array(Box::new(self.replace_vars(inner, mapping, subst)))
            }

            Ty::Dict { key, value } => Ty::Dict {
                key: Box::new(self.replace_vars(key, mapping, subst)),
                value: Box::new(self.replace_vars(value, mapping, subst)),
            },

            Ty::Optional(inner) => {
                Ty::Optional(Box::new(self.replace_vars(inner, mapping, subst)))
            }

            Ty::Result { ok, error } => Ty::Result {
                ok: Box::new(self.replace_vars(ok, mapping, subst)),
                error: Box::new(self.replace_vars(error, mapping, subst)),
            },

            // These types don't contain other types
            Ty::Primitive(_) | Ty::SelfType | Ty::Never | Ty::Error => {
                ty.clone()
            }
        }
    }

    /// Apply a substitution to this scheme.
    pub fn apply(&mut self, subst: &mut Subst) {
        self.ty = subst.apply_ty(&self.ty);
    }

    /// Get all free type variables in this scheme.
    ///
    /// Free variables are those that appear in `ty` but are NOT in `vars`.
    pub fn free_vars(&self) -> HashSet<u32> {
        let ty_vars = self.ty.free_vars();
        let bound: HashSet<u32> = self.vars.iter().copied().collect();
        ty_vars.difference(&bound).copied().collect()
    }
}

/// Type environment mapping symbols to type schemes.
///
/// The environment is organized as a stack of scopes for lexical scoping.
/// Each scope is a map from symbols to type schemes.
///
/// # Design
///
/// - **Stack of scopes**: Inner scopes shadow outer scopes
/// - **Substitution**: Accumulated during type checking
/// - **Polymorphism level**: For let-polymorphism (not used in Phase 6.1)
/// - **Mutability tracking**: Each scope tracks which bindings are mutable
#[derive(Debug, Clone)]
pub struct TypeEnv {
    /// Stack of scopes (each scope is a map from symbol to scheme).
    scopes: Vec<HashMap<Symbol, Scheme>>,

    /// Stack of mutability tracking (parallel to scopes).
    /// Each scope tracks which symbols are mutable.
    mutable: Vec<HashMap<Symbol, bool>>,

    /// Substitution accumulated during type checking.
    pub subst: Subst,

    /// Current polymorphism level (for let-polymorphism).
    level: u32,
}

impl TypeEnv {
    /// Create a new empty type environment.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let env = TypeEnv::new();
    /// assert_eq!(env.depth(), 1); // Starts with one global scope
    /// ```
    pub fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
            mutable: vec![HashMap::new()],
            subst: Subst::new(),
            level: 0,
        }
    }

    /// Get the current nesting depth (number of scopes).
    pub fn depth(&self) -> usize {
        self.scopes.len()
    }

    /// Get the current polymorphism level.
    pub fn level(&self) -> u32 {
        self.level
    }

    /// Enter a new scope.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut env = TypeEnv::new();
    /// assert_eq!(env.depth(), 1);
    ///
    /// env.new_scope();
    /// assert_eq!(env.depth(), 2);
    /// ```
    pub fn new_scope(&mut self) {
        self.scopes.push(HashMap::new());
        self.mutable.push(HashMap::new());
    }

    /// Exit the current scope.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut env = TypeEnv::new();
    /// env.new_scope();
    /// assert_eq!(env.depth(), 2);
    ///
    /// env.pop_scope();
    /// assert_eq!(env.depth(), 1);
    /// ```
    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
            self.mutable.pop();
        }
    }

    /// Bind a symbol to a type scheme in the current scope.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut env = TypeEnv::new();
    /// let sym = symbol!("x");
    ///
    /// env.bind(sym, Scheme::mono(Ty::Primitive(PrimTy::Int64)));
    /// assert!(env.lookup(sym).is_some());
    /// ```
    pub fn bind(&mut self, sym: Symbol, scheme: Scheme) {
        if let Some(scope) = self.scopes.last_mut() {
            std::collections::HashMap::insert(scope, sym, scheme);
        }
    }

    /// Bind a symbol with mutability tracking.
    pub fn bind_mut(&mut self, sym: Symbol, scheme: Scheme, is_mut: bool) {
        self.bind(sym, scheme);
        if let Some(mutable) = self.mutable.last_mut() {
            std::collections::HashMap::insert(mutable, sym, is_mut);
        }
    }

    /// Check if a symbol is mutable.
    ///
    /// Returns true if the symbol exists and is mutable, false otherwise.
    pub fn is_mutable(&self, sym: Symbol) -> bool {
        // Search from innermost to outermost
        for scope in self.mutable.iter().rev() {
            if let Some(&is_mutable) = std::collections::HashMap::get(scope, &sym) {
                return is_mutable;
            }
        }
        false
    }

    /// Look up a symbol in the environment.
    ///
    /// This searches from innermost to outermost scope.
    ///
    /// Returns `None` if the symbol is not found.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut env = TypeEnv::new();
    /// let sym = symbol!("x");
    ///
    /// assert!(env.lookup(sym).is_none());
    ///
    /// env.bind(sym, Scheme::mono(Ty::Primitive(PrimTy::Int64)));
    /// assert!(env.lookup(sym).is_some());
    /// ```
    pub fn lookup(&self, sym: Symbol) -> Option<&Scheme> {
        // Search from innermost to outermost
        for scope in self.scopes.iter().rev() {
            if let Some(scheme) = std::collections::HashMap::get(scope, &sym) {
                return Some(scheme);
            }
        }
        None
    }

    /// Look up a mutable reference to a symbol.
    pub fn lookup_mut(&mut self, sym: Symbol) -> Option<&mut Scheme> {
        for scope in self.scopes.iter_mut().rev() {
            if std::collections::HashMap::contains_key(scope, &sym) {
                return std::collections::HashMap::get_mut(scope, &sym);
            }
        }
        None
    }

    /// Generalize a type over free variables not in the environment.
    ///
    /// This is the heart of let-polymorphism:
    ///
    /// ```ignore
    /// let id = fn(x) { x }
    ///
    /// // id has type forall a. a -> a
    /// // because the type variable 'a' is not in the environment
    /// ```
    pub fn generalize(&self, ty: &Ty) -> Scheme {
        let free_in_ty = ty.free_vars();
        let free_in_env = self.free_vars();

        // Generalize over variables free in ty but not in env
        let vars: Vec<u32> =
            free_in_ty.difference(&free_in_env).copied().collect();

        Scheme {
            vars,
            ty: ty.clone(),
        }
    }

    /// Get all free type variables in the environment.
    ///
    /// This includes variables from all scopes.
    fn free_vars(&self) -> HashSet<u32> {
        let mut vars = HashSet::new();
        for scope in &self.scopes {
            for scheme in scope.values() {
                vars.extend(scheme.vars.iter().copied());
                vars.extend(scheme.ty.free_vars().iter().copied());
            }
        }
        vars
    }

    /// Apply a substitution to the entire environment.
    ///
    /// This updates all schemes in all scopes.
    pub fn apply_subst(&mut self, subst: &mut Subst) {
        for scope in &mut self.scopes {
            for (_key, scheme) in std::collections::HashMap::iter_mut(scope) {
                scheme.apply(subst);
            }
        }
    }

    /// Increment the polymorphism level.
    pub fn inc_level(&mut self) {
        self.level += 1;
    }

    /// Decrement the polymorphism level.
    pub fn dec_level(&mut self) {
        if self.level > 0 {
            self.level -= 1;
        }
    }

    /// Create a fresh type variable.
    pub fn fresh_var(&mut self) -> u32 {
        self.subst.fresh_var()
    }
}

impl Default for TypeEnv {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::PrimTy;
    use oxidex_mem::Symbol;

    #[test]
    fn test_scheme_mono() {
        let scheme = Scheme::mono(Ty::Primitive(PrimTy::Int64));
        assert_eq!(scheme.vars.len(), 0);
        assert!(matches!(scheme.ty, Ty::Primitive(PrimTy::Int64)));
    }

    #[test]
    fn test_scheme_poly() {
        let scheme = Scheme::poly(vec![0, 1], Ty::TypeVar(0));
        assert_eq!(scheme.vars.len(), 2);
        assert_eq!(scheme.vars[0], 0);
        assert_eq!(scheme.vars[1], 1);
    }

    #[test]
    fn test_env_new() {
        let env = TypeEnv::new();
        assert_eq!(env.depth(), 1);
    }

    #[test]
    fn test_env_scopes() {
        let mut env = TypeEnv::new();
        assert_eq!(env.depth(), 1);

        env.new_scope();
        assert_eq!(env.depth(), 2);

        env.pop_scope();
        assert_eq!(env.depth(), 1);
    }

    #[test]
    fn test_env_bind_lookup() {
        let mut env = TypeEnv::new();
        let sym = Symbol::new(0);

        assert!(env.lookup(sym).is_none());

        env.bind(sym, Scheme::mono(Ty::Primitive(PrimTy::Int64)));
        assert!(env.lookup(sym).is_some());
    }

    #[test]
    fn test_env_shadowing() {
        let mut env = TypeEnv::new();
        let sym = Symbol::new(0);

        // Bind in outer scope
        env.bind(sym, Scheme::mono(Ty::Primitive(PrimTy::Int64)));

        // New scope
        env.new_scope();

        // Shadow in inner scope
        env.bind(sym, Scheme::mono(Ty::Primitive(PrimTy::Bool)));

        let scheme = env.lookup(sym).unwrap();
        assert!(matches!(scheme.ty, Ty::Primitive(PrimTy::Bool)));

        // Pop and check outer binding
        env.pop_scope();
        let scheme = env.lookup(sym).unwrap();
        assert!(matches!(scheme.ty, Ty::Primitive(PrimTy::Int64)));
    }

    #[test]
    fn test_generalize() {
        let env = TypeEnv::new();

        // Type with free variable 0
        let ty = Ty::Function {
            params: vec![Ty::TypeVar(0)],
            return_type: Box::new(Ty::TypeVar(0)),
            labels: vec![None],
        };

        let scheme = env.generalize(&ty);

        // Variable 0 should be quantified (not in env)
        assert_eq!(scheme.vars.len(), 1);
        assert_eq!(scheme.vars[0], 0);
    }

    #[test]
    fn test_generalize_with_env() {
        let mut env = TypeEnv::new();
        let sym = Symbol::new(0);

        // Add variable to environment
        env.bind(sym, Scheme::mono(Ty::TypeVar(0)));

        // Now generalize a type with var 0
        let ty = Ty::TypeVar(0);
        let scheme = env.generalize(&ty);

        // Variable 0 should NOT be quantified (it's in the env)
        assert_eq!(scheme.vars.len(), 0);
    }

    #[test]
    fn test_instantiate() {
        let mut subst = Subst::new();

        // Create a polymorphic scheme
        let scheme = Scheme::poly(
            vec![0],
            Ty::Function {
                params: vec![Ty::TypeVar(0)],
                return_type: Box::new(Ty::TypeVar(0)),
                labels: vec![None],
            },
        );

        // Instantiate with fresh variables
        let ty = scheme.instantiate(&mut subst);

        // Should be a function with the same variable in params and return
        match &ty {
            Ty::Function {
                params,
                return_type,
                ..
            } => {
                if let (Ty::TypeVar(p), Ty::TypeVar(r)) =
                    (&params[0], return_type.as_ref())
                {
                    assert_eq!(p, r); // Same variable in params and return
                } else {
                    panic!("Expected type variables");
                }
            }
            _ => panic!("Expected function type"),
        }
    }

    #[test]
    fn test_fresh_var() {
        let mut env = TypeEnv::new();

        let v1 = env.fresh_var();
        let v2 = env.fresh_var();

        assert_eq!(v1, 0);
        assert_eq!(v2, 1);
    }

    #[test]
    fn test_level() {
        let mut env = TypeEnv::new();
        assert_eq!(env.level(), 0);

        env.inc_level();
        assert_eq!(env.level(), 1);

        env.inc_level();
        assert_eq!(env.level(), 2);

        env.dec_level();
        assert_eq!(env.level(), 1);
    }
}
