//! Type inference engine.
//!
//! This module implements Hindley-Milner type inference with bidirectional checking.

pub mod context;
pub mod unify;

pub use context::Context;
pub use unify::Unifier;
