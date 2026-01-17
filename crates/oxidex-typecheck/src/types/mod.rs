//! Type representation and operations.
//!
//! This module defines the core type system used during type checking:
//!
//! - **Ty**: Internal type representation with unification variables
//! - **Operations**: Type equality, free variables, structural comparison
//! - **Display**: Pretty-printing for error messages

pub mod display;
pub mod ty;

pub use ty::{PrimTy, Ty};
