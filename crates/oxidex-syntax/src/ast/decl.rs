//! Declaration nodes in the `OxideX` AST.
//!
//! Declarations represent top-level items in `OxideX` programs: functions,
//! structs, classes, enums, protocols, and more.

use crate::span::{Span, Spanned};
use oxidex_mem::Symbol;

/// A top-level declaration in the `OxideX` language.
///
/// Declarations are items that appear at module scope: functions, types,
/// constants, etc.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Decl<'arena> {
    /// Function declaration: `fn foo<T>(x: T) -> T { body }` or `init(x: T) { body }`
    Fn {
        /// Is this a mutable method (`mut fn`)?
        is_mut: bool,
        /// Is this an initializer (`init`)?
        is_init: bool,
        /// Is this a static method (`static fn`)?
        is_static: bool,
        /// Function name (for init, this will be "init")
        name: Symbol,
        /// Generic type parameters
        generics: Vec<Symbol>,
        /// Parameters
        params: Vec<FnParam>,
        /// Return type
        return_type: Option<crate::ast::ty::Type>,
        /// Function body
        body: &'arena super::expr::Expr<'arena>,
        /// Visibility
        visibility: Visibility,
        /// Source location
        span: Span,
    },

    /// Struct declaration: `struct Point<T> { x: T, y: T }`
    Struct {
        /// Struct name
        name: Symbol,
        /// Generic type parameters
        generics: Vec<Symbol>,
        /// Fields
        fields: Vec<StructField>,
        /// Protocol conformances
        protocols: Vec<Vec<Symbol>>,
        /// Visibility
        visibility: Visibility,
        /// Source location
        span: Span,
    },

    /// Class declaration: `class MyClass { ... }`
    Class {
        /// Class name
        name: Symbol,
        /// Generic type parameters
        generics: Vec<Symbol>,
        /// Optional superclass
        superclass: Option<Vec<Symbol>>,
        /// Fields
        fields: Vec<StructField>,
        /// Protocol conformances
        protocols: Vec<Vec<Symbol>>,
        /// Visibility
        visibility: Visibility,
        /// Source location
        span: Span,
    },

    /// Enum declaration: `enum Option<T> { case some(T), case none }`
    Enum {
        /// Enum name
        name: Symbol,
        /// Generic type parameters
        generics: Vec<Symbol>,
        /// Variants
        variants: Vec<EnumVariant>,
        /// Methods (can be defined directly in enum body)
        methods: Vec<FnDecl>,
        /// Protocol conformances
        protocols: Vec<Vec<Symbol>>,
        /// Visibility
        visibility: Visibility,
        /// Source location
        span: Span,
    },

    /// Protocol declaration: `protocol MyProtocol { fn method(); }`
    Protocol {
        /// Protocol name
        name: Symbol,
        /// Generic type parameters
        generics: Vec<Symbol>,
        /// Method signatures
        methods: Vec<ProtocolMethod>,
        /// Visibility
        visibility: Visibility,
        /// Source location
        span: Span,
    },

    /// Implementation block: `impl Type { ... }` or `impl Protocol for Type { ... }`
    Impl {
        /// Type being implemented
        type_path: Vec<Symbol>,
        /// Optional protocol being implemented
        protocol: Option<Vec<Symbol>>,
        /// Methods
        methods: Vec<FnDecl>,
        /// Source location
        span: Span,
    },

    /// Constant declaration: `const MAX: Int = 100;`
    Const {
        /// Constant name
        name: Symbol,
        /// Type annotation
        type_annotation: crate::ast::ty::Type,
        /// Value
        value: &'arena super::expr::Expr<'arena>,
        /// Visibility
        visibility: Visibility,
        /// Source location
        span: Span,
    },

    /// Static declaration: `static COUNTER: Int = 0;`
    Static {
        /// Static variable name
        name: Symbol,
        /// Type annotation
        type_annotation: crate::ast::ty::Type,
        /// Optional initializer
        init: Option<&'arena super::expr::Expr<'arena>>,
        /// Is mutable
        mutable: bool,
        /// Visibility
        visibility: Visibility,
        /// Source location
        span: Span,
    },

    /// Type alias: `type Result<T> = Option<T>;`
    TypeAlias {
        /// Alias name
        name: Symbol,
        /// Generic type parameters
        generics: Vec<Symbol>,
        /// Target type
        target: crate::ast::ty::Type,
        /// Visibility
        visibility: Visibility,
        /// Source location
        span: Span,
    },
}

impl Spanned for Decl<'_> {
    fn span(&self) -> Span {
        match self {
            Self::Fn { span, .. }
            | Self::Struct { span, .. }
            | Self::Class { span, .. }
            | Self::Enum { span, .. }
            | Self::Protocol { span, .. }
            | Self::Impl { span, .. }
            | Self::Const { span, .. }
            | Self::Static { span, .. }
            | Self::TypeAlias { span, .. } => *span,
        }
    }
}

/// Visibility modifier for declarations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Visibility {
    /// Public: `pub`
    Public,
    /// Private (file-private): `prv` or implicit
    Private,
}

/// A function parameter: `x: Type` or `label: Type` or `external internal: Type`
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FnParam {
    /// External label (for calling) - None means use internal name or omitted with `_`
    pub label: Option<Symbol>,
    /// Internal parameter name (used in function body)
    pub name: Symbol,
    /// Parameter type
    pub type_annotation: crate::ast::ty::Type,
    /// Source location
    pub span: Span,
}

/// A struct field: `name: Type`
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StructField {
    /// Field name
    pub name: Symbol,
    /// Field type
    pub type_annotation: crate::ast::ty::Type,
    /// Source location
    pub span: Span,
}

/// An enum variant: `VariantName` or `VariantName(Type1, Type2)` or `VariantName { fields }`
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EnumVariant {
    /// Unit variant: `None`
    Unit {
        /// Variant name
        name: Symbol,
        /// Source location
        span: Span,
    },
    /// Tuple variant: `Some(T)`
    Tuple {
        /// Variant name
        name: Symbol,
        /// Payload types
        fields: Vec<crate::ast::ty::Type>,
        /// Source location
        span: Span,
    },
    /// Struct variant: `Point { x: T, y: T }`
    Struct {
        /// Variant name
        name: Symbol,
        /// Fields
        fields: Vec<StructField>,
        /// Source location
        span: Span,
    },
}

/// A protocol method signature.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProtocolMethod {
    /// Method name
    pub name: Symbol,
    /// Parameters
    pub params: Vec<FnParam>,
    /// Return type
    pub return_type: Option<crate::ast::ty::Type>,
    /// Source location
    pub span: Span,
}

/// A function declaration (standalone, for impl blocks and protocols).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FnDecl {
    /// Is this a mutable method (`mut fn`)?
    pub is_mut: bool,
    /// Is this an initializer (`init`)?
    pub is_init: bool,
    /// Is this a static method (`static fn`)?
    pub is_static: bool,
    /// Function name
    pub name: Option<Symbol>,
    /// Generic type parameters
    pub generics: Vec<Symbol>,
    /// Parameters
    pub params: Vec<FnParam>,
    /// Return type
    pub return_type: Option<crate::ast::ty::Type>,
    /// Visibility (resolved to most restrictive of parent and method during semantic analysis)
    pub visibility: Visibility,
    /// Source location
    pub span: Span,
}
