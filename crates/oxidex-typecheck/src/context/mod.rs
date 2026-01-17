//! Type checking context and environment management.
//!
//! This module provides the core context for type checking:
//!
//! - **Subst**: Substitutions with union-find for unification
//! - **TypeEnv**: Type environment with lexical scoping
//! - **TypeRegistry**: Registry for struct/enum definitions

pub mod env;
pub mod registry;
pub mod subst;

pub use env::{Scheme, TypeEnv};
pub use registry::{ClassInfo, EnumInfo, EnumVariantInfo, FieldInfo, MethodInfo, ProtocolInfo, ProtocolMethodInfo, StructInfo, TypeRegistry};
pub use subst::Subst;
