//! `OxideX` Type Checker: Type Inference and Validation
//!
//! This crate provides type system functionality for `OxideX`, including:
//! - Type inference (Hindley-Milner)
//! - Constraint solving
//! - Type checking and validation
//! - Protocol conformance checking
//!
//! **Phase:** 6 - Type Checker
//! **Status:** In Progress (Phase 6.2)

#![warn(missing_docs)]

// Type representation and operations
pub mod types;

// Type checking context and environment
pub mod context;

// Type checking errors
pub mod error;

// Type inference engine
pub mod infer;

// Type checking validators
pub mod check;

// Re-exports for convenience
pub use context::{Scheme, Subst, TypeEnv};
pub use error::Result;
pub use infer::Context as InferContext;
pub use types::{PrimTy, Ty};
