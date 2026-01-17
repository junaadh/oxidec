//! Type checking validators.
//!
//! This module provides type checking for AST nodes:
//! - Expressions (bidirectional checking)
//! - Statements
//! - Declarations
//! - Type annotation conversion
//! - Pattern type checking

pub mod decl;
pub mod expr;
pub mod pat;
pub mod stmt;
pub mod ty;

pub use decl::{check_decl, check_bodies, collect_signatures};
pub use expr::{check, synth};
pub use pat::check_pat;
pub use stmt::check_stmt;
pub use ty::ast_to_ty;
