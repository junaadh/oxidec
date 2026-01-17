//! Type annotation nodes in the `OxideX` AST.
//!
//! Types represent type annotations and type signatures throughout `OxideX`.

use crate::span::{Span, Spanned};
use oxidex_mem::Symbol;

/// A type annotation in the `OxideX` language.
///
/// Types can appear in many contexts: function signatures, struct fields,
/// variable annotations, and more.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Type {
    /// Simple type identifier: `Int`, `String`, `MyType`
    Simple {
        /// Type name
        name: Symbol,
        /// Source location
        span: Span,
    },

    /// Generic type: `List<T>`, `Map<K, V>`
    Generic {
        /// Base type name
        name: Symbol,
        /// Type parameters
        params: Vec<Type>,
        /// Source location
        span: Span,
    },

    /// Tuple type: `(T1, T2, T3)`
    Tuple {
        /// Element types
        elements: Vec<Type>,
        /// Source location
        span: Span,
    },

    /// Function type: `(T1, T2) -> T3`
    Function {
        /// Parameter types
        params: Vec<Type>,
        /// Return type
        return_type: Box<Type>,
        /// Source location
        span: Span,
    },

    /// Array type: `[T]` or `[T; N]`
    Array {
        /// Element type
        element: Box<Type>,
        /// Optional size (constant expression)
        size: Option<Symbol>,
        /// Source location
        span: Span,
    },

    /// Dictionary type: `[K: V]`
    Dict {
        /// Key type
        key: Box<Type>,
        /// Value type
        value: Box<Type>,
        /// Source location
        span: Span,
    },

    /// Optional type: `T?`
    Optional {
        /// Inner type
        inner: Box<Type>,
        /// Source location
        span: Span,
    },

    /// Reference type: `&T` or `&mut T`
    Reference {
        /// Inner type
        inner: Box<Type>,
        /// Is mutable
        mutable: bool,
        /// Source location
        span: Span,
    },

    /// Inferred type: `_`
    Inferred {
        /// Source location
        span: Span,
    },
}

impl Spanned for Type {
    fn span(&self) -> Span {
        match self {
            Self::Simple { span, .. }
            | Self::Generic { span, .. }
            | Self::Tuple { span, .. }
            | Self::Function { span, .. }
            | Self::Array { span, .. }
            | Self::Dict { span, .. }
            | Self::Optional { span, .. }
            | Self::Reference { span, .. }
            | Self::Inferred { span, .. } => *span,
        }
    }
}
