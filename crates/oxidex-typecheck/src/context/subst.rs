//! Substitutions for type unification.
//!
//! This module implements substitutions using a union-find data structure with path compression.
//! Substitutions map type variables to their representative types during unification.
//!
//! # Design
//!
//! - **Union-Find**: Each type variable points to its parent (or None if it's a root)
//! - **Path Compression**: When looking up a variable, we flatten the structure for O(1) future lookups
//! - **Union by Rank**: Not implemented yet (could be added for optimization)
//!
//! # Example
//!
//! ```ignore
//! let mut subst = Subst::new();
//! // Initially: TypeVar(0) is unbound
//!
//! // Unify TypeVar(0) with Int
//! subst.bind(0, Ty::Primitive(PrimTy::Int64));
//!
//! // Lookup: TypeVar(0) => Some(Int)
//! let ty = subst.lookup(0);
//! assert_eq!(ty, Some(&Ty::Primitive(PrimTy::Int64)));
//! ```

use crate::types::Ty;

/// Substitution from type variables to types.
///
/// This implements a union-find structure where each type variable
/// either points to another variable (union) or to a concrete type.
///
/// # Representation
///
/// - `parent[i] = None`: Type variable `i` is unbound (root)
/// - `parent[i] = Some(ty)`: Type variable `i` is bound to `ty`
///
/// # Performance
///
/// With path compression, lookup and union operations are near O(1).
#[derive(Debug, Clone)]
pub struct Subst {
    /// Parent array for union-find.
    ///
    /// `parent[i] = None` means variable `i` is a root (unbound).
    /// `parent[i] = Some(ty)` means variable `i` is bound to `ty`.
    parent: Vec<Option<Ty>>,

    /// Next available type variable index.
    next_var: u32,
}

impl Subst {
    /// Create a new empty substitution.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let subst = Subst::new();
    /// assert_eq!(subst.len(), 0);
    /// ```
    pub fn new() -> Self {
        Self {
            parent: Vec::new(),
            next_var: 0,
        }
    }

    /// Create an empty substitution (alias for `new()`).
    pub fn empty() -> Self {
        Self::new()
    }

    /// Get the number of type variables in this substitution.
    pub fn len(&self) -> usize {
        self.parent.len()
    }

    /// Check if this substitution is empty.
    pub fn is_empty(&self) -> bool {
        self.parent.is_empty()
    }

    /// Allocate a fresh type variable.
    ///
    /// Returns the index of the new variable.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut subst = Subst::new();
    /// let var = subst.fresh_var();
    /// assert_eq!(var, 0);
    ///
    /// let var2 = subst.fresh_var();
    /// assert_eq!(var2, 1);
    /// ```
    pub fn fresh_var(&mut self) -> u32 {
        let var = self.next_var;
        self.parent.push(None);
        self.next_var += 1;
        var
    }

    /// Look up a type variable, following parent pointers.
    ///
    /// This performs path compression to flatten the structure.
    ///
    /// Returns `None` if the variable is unbound.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut subst = Subst::new();
    /// let var = subst.fresh_var();
    ///
    /// // Initially unbound
    /// assert_eq!(subst.lookup(var), None);
    ///
    /// // Bind to Int
    /// subst.bind(var, Ty::Primitive(PrimTy::Int6464));
    /// assert_eq!(subst.lookup(var), Some(&Ty::Primitive(PrimTy::Int6464)));
    /// ```
    pub fn lookup(&mut self, var: u32) -> Option<&Ty> {
        if (var as usize) >= self.parent.len() {
            return None;
        }

        // Follow parent pointers
        let mut current = var;
        let mut path = Vec::new();

        while let Some(ty) = &self.parent[current as usize] {
            match ty {
                Ty::TypeVar(next_var) => {
                    path.push(current);
                    current = *next_var;
                }
                _ => {
                    // Found a concrete type
                    break;
                }
            }
        }

        // Path compression
        for v in path {
            if current as usize >= self.parent.len() {
                break;
            }
            self.parent[v as usize] = self.parent[current as usize].clone();
        }

        self.parent.get(current as usize)?.as_ref()
    }

    /// Look up the representative type of a variable (with mut access for path compression).
    ///
    /// This is similar to `lookup` but returns a mutable reference for internal use.
    pub fn lookup_rep(&mut self, var: u32) -> Result<Ty, String> {
        if var as usize >= self.parent.len() {
            return Err(format!("Type variable {} out of bounds", var));
        }

        let mut current = var;
        let mut path = Vec::new();

        while let Some(ty) = &self.parent[current as usize] {
            match ty {
                Ty::TypeVar(next_var) => {
                    path.push(current);
                    current = *next_var;
                }
                _ => {
                    // Found a concrete type
                    return Ok(ty.clone());
                }
            }
        }

        // Variable is unbound
        Ok(Ty::TypeVar(current))
    }

    /// Bind a type variable to a type.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut subst = Subst::new();
    /// let var = subst.fresh_var();
    ///
    /// subst.bind(var, Ty::Primitive(PrimTy::Int64));
    /// assert_eq!(subst.lookup(var), Some(&Ty::Primitive(PrimTy::Int64)));
    /// ```
    pub fn bind(&mut self, var: u32, ty: Ty) {
        if (var as usize) < self.parent.len() {
            self.parent[var as usize] = Some(ty);
        }
    }

    /// Union two type variables.
    ///
    /// Makes `var1` point to `var2`.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut subst = Subst::new();
    /// let var1 = subst.fresh_var();
    /// let var2 = subst.fresh_var();
    ///
    /// subst.bind(var2, Ty::Primitive(PrimTy::Int6464));
    /// subst.union(var1, var2);
    ///
    /// // Now var1 also points to Int64
    /// assert_eq!(subst.lookup(var1), Some(&Ty::Primitive(PrimTy::Int6464)));
    /// ```
    pub fn union(&mut self, var1: u32, var2: u32) {
        if (var1 as usize) < self.parent.len() && (var2 as usize) < self.parent.len() {
            self.parent[var1 as usize] = Some(Ty::TypeVar(var2));
        }
    }

    /// Create a substitution that binds a single variable.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let subst = Subst::single(0, Ty::Primitive(PrimTy::Int64));
    /// assert_eq!(subst.lookup(0), Some(&Ty::Primitive(PrimTy::Int64)));
    /// ```
    pub fn single(var: u32, ty: Ty) -> Self {
        let mut subst = Self::new();
        // Ensure the variable exists
        while subst.next_var <= var {
            subst.parent.push(None);
            subst.next_var += 1;
        }
        subst.bind(var, ty);
        subst
    }

    /// Compose two substitutions.
    ///
    /// `(s1 ∘ s2)(x) = s1(s2(x))`
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut s1 = Subst::new();
    /// let mut s2 = Subst::new();
    ///
    /// let v1 = s1.fresh_var();
    /// let v2 = s2.fresh_var();
    ///
    /// s1.bind(v1, Ty::Primitive(PrimTy::Int64));
    /// s2.bind(v2, Ty::Primitive(PrimTy::Bool));
    ///
    /// let s3 = s1.compose(s2);
    /// ```
    pub fn compose(mut self, other: Subst) -> Subst {
        // For now, just merge the two substitutions
        // This is a simplified version; a proper implementation would
        // apply `self` to all types in `other`
        for (i, ty) in other.parent.into_iter().enumerate() {
            if i >= self.parent.len() {
                self.parent.push(ty);
            } else if self.parent[i].is_none() && ty.is_some() {
                self.parent[i] = ty;
            }
        }

        self
    }

    /// Apply a substitution to a type.
    ///
    /// This replaces all type variables in `ty` with their bindings.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut subst = Subst::new();
    /// let var = subst.fresh_var();
    /// subst.bind(var, Ty::Primitive(PrimTy::Int64));
    ///
    /// let ty = Ty::Array(Box::new(Ty::TypeVar(var)));
    /// let result = subst.apply_ty(&ty);
    /// assert_eq!(result, Ty::Array(Box::new(Ty::Primitive(PrimTy::Int64))));
    /// ```
    pub fn apply_ty(&mut self, ty: &Ty) -> Ty {
        match ty {
            Ty::TypeVar(var) => {
                match self.lookup(*var) {
                    Some(binding) => binding.clone(),
                    None => Ty::TypeVar(*var),
                }
            }

            Ty::Struct { name, type_args } => Ty::Struct {
                name: *name,
                type_args: type_args.iter().map(|t| self.apply_ty(t)).collect(),
            },

            Ty::Class { name, type_args } => Ty::Class {
                name: *name,
                type_args: type_args.iter().map(|t| self.apply_ty(t)).collect(),
            },

            Ty::Enum { name, type_args } => Ty::Enum {
                name: *name,
                type_args: type_args.iter().map(|t| self.apply_ty(t)).collect(),
            },

            Ty::Protocol { name, type_args } => Ty::Protocol {
                name: *name,
                type_args: type_args.iter().map(|t| self.apply_ty(t)).collect(),
            },

            Ty::Tuple(types) => Ty::Tuple(types.iter().map(|t| self.apply_ty(t)).collect()),

            Ty::Function {
                params,
                return_type,
                labels,
            } => Ty::Function {
                params: params.iter().map(|p| self.apply_ty(p)).collect(),
                return_type: Box::new(self.apply_ty(return_type)),
                labels: labels.clone(),
            },

            Ty::Array(inner) => Ty::Array(Box::new(self.apply_ty(inner))),

            Ty::Dict { key, value } => Ty::Dict {
                key: Box::new(self.apply_ty(key)),
                value: Box::new(self.apply_ty(value)),
            },

            Ty::Optional(inner) => Ty::Optional(Box::new(self.apply_ty(inner))),

            Ty::Result { ok, error } => Ty::Result {
                ok: Box::new(self.apply_ty(ok)),
                error: Box::new(self.apply_ty(error)),
            },

            // These types don't contain other types
            Ty::Primitive(_) | Ty::SelfType | Ty::Never | Ty::Error => ty.clone(),
        }
    }

    /// Get all unbound type variables.
    ///
    /// Returns indices of variables that are still free (not bound to a concrete type).
    pub fn unbound_vars(&self) -> Vec<u32> {
        let mut vars = Vec::new();
        for (i, binding) in self.parent.iter().enumerate() {
            if binding.is_none() {
                vars.push(i as u32);
            }
        }
        vars
    }

    /// Clone only the bindings (not the entire structure).
    ///
    /// This is more efficient than `clone()` when you only need the bindings.
    pub fn clone_bindings(&self) -> Vec<Option<Ty>> {
        self.parent.clone()
    }
}

impl Default for Subst {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::PrimTy;

    #[test]
    fn test_empty_subst() {
        let subst = Subst::new();
        assert!(subst.is_empty());
        assert_eq!(subst.len(), 0);
    }

    #[test]
    fn test_fresh_var() {
        let mut subst = Subst::new();
        let v1 = subst.fresh_var();
        let v2 = subst.fresh_var();

        assert_eq!(v1, 0);
        assert_eq!(v2, 1);
        assert_eq!(subst.len(), 2);
    }

    #[test]
    fn test_bind_and_lookup() {
        let mut subst = Subst::new();
        let var = subst.fresh_var();

        assert_eq!(subst.lookup(var), None);

        subst.bind(var, Ty::Primitive(PrimTy::Int64));
        assert_eq!(subst.lookup(var), Some(&Ty::Primitive(PrimTy::Int64)));
    }

    #[test]
    fn test_union() {
        let mut subst = Subst::new();
        let var1 = subst.fresh_var();
        let var2 = subst.fresh_var();

        subst.bind(var2, Ty::Primitive(PrimTy::Int64));
        subst.union(var1, var2);

        // var1 should now point to var2 which points to Int
        let lookup = subst.lookup(var1);
        assert_eq!(lookup, Some(&Ty::Primitive(PrimTy::Int64)));
    }

    #[test]
    fn test_single() {
        let mut subst = Subst::single(0, Ty::Primitive(PrimTy::Bool));
        assert_eq!(subst.lookup(0), Some(&Ty::Primitive(PrimTy::Bool)));
    }

    #[test]
    fn test_apply_ty() {
        let mut subst = Subst::new();
        let var = subst.fresh_var();

        subst.bind(var, Ty::Primitive(PrimTy::Int64));

        let ty = Ty::Array(Box::new(Ty::TypeVar(var)));
        let result = subst.apply_ty(&ty);

        assert_eq!(
            result,
            Ty::Array(Box::new(Ty::Primitive(PrimTy::Int64)))
        );
    }

    #[test]
    fn test_apply_ty_nested() {
        let mut subst = Subst::new();
        let var = subst.fresh_var();

        subst.bind(var, Ty::Primitive(PrimTy::String));

        let ty = Ty::Tuple(vec![
            Ty::TypeVar(var),
            Ty::Primitive(PrimTy::Int64),
            Ty::Array(Box::new(Ty::TypeVar(var))),
        ]);

        let result = subst.apply_ty(&ty);

        assert_eq!(
            result,
            Ty::Tuple(vec![
                Ty::Primitive(PrimTy::String),
                Ty::Primitive(PrimTy::Int64),
                Ty::Array(Box::new(Ty::Primitive(PrimTy::String))),
            ])
        );
    }

    #[test]
    fn test_unbound_vars() {
        let mut subst = Subst::new();
        let v1 = subst.fresh_var();
        let v2 = subst.fresh_var();
        let v3 = subst.fresh_var();

        subst.bind(v2, Ty::Primitive(PrimTy::Int64));

        let unbound = subst.unbound_vars();
        assert_eq!(unbound.len(), 2);
        assert!(unbound.contains(&v1));
        assert!(unbound.contains(&v3));
        assert!(!unbound.contains(&v2));
    }

    #[test]
    fn test_path_compression() {
        let mut subst = Subst::new();
        let v1 = subst.fresh_var();
        let v2 = subst.fresh_var();
        let v3 = subst.fresh_var();

        // Create a chain: v1 -> v2 -> v3 -> Int
        subst.bind(v3, Ty::Primitive(PrimTy::Int64));
        subst.union(v2, v3);
        subst.union(v1, v2);

        // First lookup should compress the path
        let _ = subst.lookup(v1);

        // Subsequent lookups should be O(1)
        let lookup = subst.lookup(v1);
        assert_eq!(lookup, Some(&Ty::Primitive(PrimTy::Int64)));
    }
}
