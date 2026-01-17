//! Type pretty-printing for error messages.
//!
//! This module provides display implementations for types, making them
//! human-readable for error messages and diagnostics.

use crate::types::PrimTy;
use crate::types::Ty;
use oxidex_mem::StringInterner;
use std::fmt;

/// Display a type with an interner for symbol resolution.
///
/// This trait is implemented for `Ty` to allow pretty-printing.
pub trait Display<'a> {
    /// Format the type for display.
    fn display(&self, interner: &'a StringInterner) -> DisplayTy<'a>;
}

/// A wrapper type for displaying types.
///
/// This implements `fmt::Display` for types with symbol resolution.
pub struct DisplayTy<'a> {
    ty: &'a Ty,
    interner: &'a StringInterner,
}

impl<'a> DisplayTy<'a> {
    /// Create a new display wrapper for a type.
    pub fn new(ty: &'a Ty, interner: &'a StringInterner) -> Self {
        Self { ty, interner }
    }
}

impl<'a> fmt::Display for DisplayTy<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.format_type(self.ty, f)
    }
}

impl<'a> DisplayTy<'a> {
    /// Format a type.
    fn format_type(&self, ty: &Ty, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match ty {
            Ty::TypeVar(var) => {
                // Display type variables as '?0', '?1', etc.
                write!(f, "?{}", var)
            }

            Ty::Primitive(prim) => self.format_primitive(prim, f),

            Ty::Struct { name, type_args } => {
                self.format_generic(name, type_args, "Struct", f)
            }

            Ty::Class { name, type_args } => {
                self.format_generic(name, type_args, "Class", f)
            }

            Ty::Enum { name, type_args } => self.format_generic(name, type_args, "Enum", f),

            Ty::Protocol { name, type_args } => {
                self.format_generic(name, type_args, "Protocol", f)
            }

            Ty::Tuple(types) => {
                write!(f, "(")?;
                for (i, ty) in types.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    self.format_type(ty, f)?;
                }
                write!(f, ")")
            }

            Ty::Function {
                params,
                return_type,
                labels,
            } => {
                write!(f, "(")?;
                for (i, param) in params.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }

                    // Add label if present
                    if let Some(Some(label)) = labels.get(i) {
                        let label_str = self.interner.resolve(*label).unwrap_or("_");
                        write!(f, "{}: ", label_str)?;
                    }

                    self.format_type(param, f)?;
                }
                write!(f, ") -> ")?;
                self.format_type(return_type, f)
            }

            Ty::Array(inner) => {
                write!(f, "[")?;
                self.format_type(inner, f)?;
                write!(f, "]")
            }

            Ty::Dict { key, value } => {
                write!(f, "[")?;
                self.format_type(key, f)?;
                write!(f, ": ")?;
                self.format_type(value, f)?;
                write!(f, "]")
            }

            Ty::Optional(inner) => {
                self.format_type(inner, f)?;
                write!(f, "?")
            }

            Ty::Result { ok, error } => {
                write!(f, "Result<")?;
                self.format_type(ok, f)?;
                write!(f, ", ")?;
                self.format_type(error, f)?;
                write!(f, ">")
            }

            Ty::SelfType => write!(f, "Self"),

            Ty::Never => write!(f, "!"),

            Ty::Error => write!(f, "<error>"),
        }
    }

    /// Format a primitive type.
    fn format_primitive(&self, prim: &PrimTy, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match prim {
            PrimTy::Int8 => write!(f, "Int8"),
            PrimTy::Int16 => write!(f, "Int16"),
            PrimTy::Int32 => write!(f, "Int32"),
            PrimTy::Int64 => write!(f, "Int64"),
            PrimTy::Int128 => write!(f, "Int128"),
            PrimTy::UInt8 => write!(f, "UInt8"),
            PrimTy::UInt16 => write!(f, "UInt16"),
            PrimTy::UInt32 => write!(f, "UInt32"),
            PrimTy::UInt64 => write!(f, "UInt64"),
            PrimTy::UInt128 => write!(f, "UInt128"),
            PrimTy::Float32 => write!(f, "Float32"),
            PrimTy::Float64 => write!(f, "Float64"),
            PrimTy::Bool => write!(f, "Bool"),
            PrimTy::String => write!(f, "String"),
            PrimTy::Unit => write!(f, "()"),
            PrimTy::Char => write!(f, "Char"),
        }
    }

    /// Format a generic type with type arguments.
    fn format_generic(
        &self,
        name: &oxidex_mem::Symbol,
        type_args: &[Ty],
        _kind: &str,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        let name_str = self.interner.resolve(*name).unwrap_or("?");

        if type_args.is_empty() {
            write!(f, "{}", name_str)
        } else {
            write!(f, "{}<", name_str)?;
            for (i, arg) in type_args.iter().enumerate() {
                if i > 0 {
                    write!(f, ", ")?;
                }
                self.format_type(arg, f)?;
            }
            write!(f, ">")
        }
    }
}

impl crate::types::Ty {
    /// Create a display wrapper for this type.
    pub fn display<'a>(&'a self, interner: &'a StringInterner) -> DisplayTy<'a> {
        DisplayTy::new(self, interner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxidex_mem::StringInterner;

    #[test]
    fn test_display_primitive() {
        let interner = StringInterner::new();

        let ty = Ty::Primitive(PrimTy::Int64);
        assert_eq!(format!("{}", ty.display(&interner)), "Int64");

        let ty = Ty::Primitive(PrimTy::Bool);
        assert_eq!(format!("{}", ty.display(&interner)), "Bool");

        let ty = Ty::Primitive(PrimTy::Unit);
        assert_eq!(format!("{}", ty.display(&interner)), "()");
    }

    #[test]
    fn test_display_type_var() {
        let interner = StringInterner::new();

        let ty = Ty::TypeVar(0);
        assert_eq!(format!("{}", ty.display(&interner)), "?0");

        let ty = Ty::TypeVar(42);
        assert_eq!(format!("{}", ty.display(&interner)), "?42");
    }

    #[test]
    fn test_display_tuple() {
        let interner = StringInterner::new();

        let ty = Ty::Tuple(vec![
            Ty::Primitive(PrimTy::Int64),
            Ty::Primitive(PrimTy::Bool),
            Ty::Primitive(PrimTy::String),
        ]);

        assert_eq!(format!("{}", ty.display(&interner)), "(Int64, Bool, String)");
    }

    #[test]
    fn test_display_array() {
        let interner = StringInterner::new();

        let ty = Ty::Array(Box::new(Ty::Primitive(PrimTy::Int64)));
        assert_eq!(format!("{}", ty.display(&interner)), "[Int64]");
    }

    #[test]
    fn test_display_dict() {
        let interner = StringInterner::new();

        let ty = Ty::Dict {
            key: Box::new(Ty::Primitive(PrimTy::String)),
            value: Box::new(Ty::Primitive(PrimTy::Int64)),
        };

        assert_eq!(format!("{}", ty.display(&interner)), "[String: Int64]");
    }

    #[test]
    fn test_display_function() {
        let interner = StringInterner::new();

        let ty = Ty::Function {
            params: vec![Ty::Primitive(PrimTy::Int64), Ty::Primitive(PrimTy::Bool)],
            return_type: Box::new(Ty::Primitive(PrimTy::String)),
            labels: vec![None, None],
        };

        assert_eq!(format!("{}", ty.display(&interner)), "(Int64, Bool) -> String");
    }

    #[test]
    fn test_display_optional() {
        let interner = StringInterner::new();

        let ty = Ty::Optional(Box::new(Ty::Primitive(PrimTy::Int64)));
        assert_eq!(format!("{}", ty.display(&interner)), "Int64?");
    }

    #[test]
    fn test_display_result() {
        let interner = StringInterner::new();

        let ty = Ty::Result {
            ok: Box::new(Ty::Primitive(PrimTy::Int64)),
            error: Box::new(Ty::Primitive(PrimTy::String)),
        };

        assert_eq!(format!("{}", ty.display(&interner)), "Result<Int64, String>");
    }

    #[test]
    fn test_display_self() {
        let interner = StringInterner::new();

        let ty = Ty::SelfType;
        assert_eq!(format!("{}", ty.display(&interner)), "Self");

        let ty = Ty::Never;
        assert_eq!(format!("{}", ty.display(&interner)), "!");

        let ty = Ty::Error;
        assert_eq!(format!("{}", ty.display(&interner)), "<error>");
    }
}
