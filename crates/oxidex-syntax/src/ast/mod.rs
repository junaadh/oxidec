//! Abstract Syntax Tree (AST) for the `OxideX` language.
//!
//! This module defines all AST node types used throughout the `OxideX` compiler.
//! Each node preserves source location information via [`Span`] for accurate
//! error reporting.
//!
//! # Design Principles
//!
//! - **Arena Allocation**: All AST nodes are allocated in arenas for zero-overhead performance
//! - **Span Preservation**: Every node tracks its source location for error reporting
//! - **Memory Safety**: Arena lifetimes ensure pointers remain valid
//! - **Ergonomic**: Clear structure matching the `OxideX` language grammar
//!
//! # Modules
//!
//! - [`expr`] - Expression nodes (literals, operators, control flow, etc.)
//! - [`stmt`] - Statement nodes (let, return, control flow)
//! - [`decl`] - Declaration nodes (fn, struct, class, enum, protocol, impl)
//! - [`ty`] - Type annotation nodes
//! - [`pat`] - Pattern matching nodes

pub mod expr;
pub mod stmt;
pub mod ty;
pub mod pat;
pub mod decl;

// Re-exports for convenience
pub use expr::Expr;
pub use stmt::Stmt;
pub use ty::Type;
pub use pat::Pattern;
pub use decl::Decl;
