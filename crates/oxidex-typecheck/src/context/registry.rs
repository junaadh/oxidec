//! Type registry for struct/enum definitions.
//!
//! This module stores type definitions (structs, enums, classes) with their
//! field information, enabling type checking of construction and field access.

use crate::types::Ty;
use oxidex_mem::Symbol;
use std::collections::HashMap;

/// Information about a protocol method.
#[derive(Debug, Clone)]
pub struct ProtocolMethodInfo {
    /// Method name
    pub name: Symbol,
    /// Parameter types
    pub params: Vec<Ty>,
    /// Return type
    pub return_type: Ty,
}

/// Information about a method.
#[derive(Debug, Clone)]
pub struct MethodInfo {
    /// Method name
    pub name: Symbol,
    /// Parameter types
    pub params: Vec<Ty>,
    /// Return type
    pub return_type: Ty,
    /// Is this a mutable method?
    pub is_mut: bool,
    /// Is this a static method?
    pub is_static: bool,
}

/// Information about a struct field.
#[derive(Debug, Clone)]
pub struct FieldInfo {
    /// Field name
    pub name: Symbol,
    /// Field type
    pub ty: Ty,
}

/// Information about a struct definition.
#[derive(Debug, Clone)]
pub struct StructInfo {
    /// Struct name
    pub name: Symbol,
    /// Fields
    pub fields: Vec<FieldInfo>,
    /// Methods
    pub methods: Vec<MethodInfo>,
    /// Generic type parameters
    pub generics: Vec<Symbol>,
}

/// Information about an enum variant.
#[derive(Debug, Clone)]
pub struct EnumVariantInfo {
    /// Variant name
    pub name: Symbol,
    /// Payload type (None for no payload, Some for typed payload)
    pub payload: Option<Ty>,
}

/// Information about an enum definition.
#[derive(Debug, Clone)]
pub struct EnumInfo {
    /// Enum name
    pub name: Symbol,
    /// Variants
    pub variants: Vec<EnumVariantInfo>,
    /// Methods
    pub methods: Vec<MethodInfo>,
    /// Generic type parameters
    pub generics: Vec<Symbol>,
}

/// Information about a class definition.
#[derive(Debug, Clone)]
pub struct ClassInfo {
    /// Class name
    pub name: Symbol,
    /// Superclass (if any)
    pub superclass: Option<Symbol>,
    /// Fields
    pub fields: Vec<FieldInfo>,
    /// Methods
    pub methods: Vec<MethodInfo>,
    /// Generic type parameters
    pub generics: Vec<Symbol>,
}

/// Information about a protocol definition.
#[derive(Debug, Clone)]
pub struct ProtocolInfo {
    /// Protocol name
    pub name: Symbol,
    /// Required methods
    pub methods: Vec<ProtocolMethodInfo>,
    /// Generic type parameters
    pub generics: Vec<Symbol>,
}

/// Type registry storing all struct/enum/protocol definitions.
#[derive(Debug, Clone)]
pub struct TypeRegistry {
    /// Struct definitions
    structs: HashMap<Symbol, StructInfo>,

    /// Enum definitions
    enums: HashMap<Symbol, EnumInfo>,

    /// Class definitions
    classes: HashMap<Symbol, ClassInfo>,

    /// Protocol definitions
    protocols: HashMap<Symbol, ProtocolInfo>,
}

impl TypeRegistry {
    /// Create a new empty type registry.
    pub fn new() -> Self {
        Self {
            structs: HashMap::new(),
            enums: HashMap::new(),
            classes: HashMap::new(),
            protocols: HashMap::new(),
        }
    }

    /// Register a struct definition.
    pub fn register_struct(&mut self, info: StructInfo) {
        self.structs.insert(info.name, info);
    }

    /// Register an enum definition.
    pub fn register_enum(&mut self, info: EnumInfo) {
        self.enums.insert(info.name, info);
    }

    /// Register a class definition.
    pub fn register_class(&mut self, info: ClassInfo) {
        self.classes.insert(info.name, info);
    }

    /// Register a protocol definition.
    pub fn register_protocol(&mut self, info: ProtocolInfo) {
        self.protocols.insert(info.name, info);
    }

    /// Look up a struct definition.
    pub fn lookup_struct(&self, name: Symbol) -> Option<&StructInfo> {
        self.structs.get(&name)
    }

    /// Look up an enum definition.
    pub fn lookup_enum(&self, name: Symbol) -> Option<&EnumInfo> {
        self.enums.get(&name)
    }

    /// Look up a class definition.
    pub fn lookup_class(&self, name: Symbol) -> Option<&ClassInfo> {
        self.classes.get(&name)
    }

    /// Look up a protocol definition.
    pub fn lookup_protocol(&self, name: Symbol) -> Option<&ProtocolInfo> {
        self.protocols.get(&name)
    }

    /// Check if a struct exists.
    pub fn has_struct(&self, name: Symbol) -> bool {
        self.structs.contains_key(&name)
    }

    /// Check if an enum exists.
    pub fn has_enum(&self, name: Symbol) -> bool {
        self.enums.contains_key(&name)
    }

    /// Check if a class exists.
    pub fn has_class(&self, name: Symbol) -> bool {
        self.classes.contains_key(&name)
    }

    /// Check if a protocol exists.
    pub fn has_protocol(&self, name: Symbol) -> bool {
        self.protocols.contains_key(&name)
    }
}

impl Default for TypeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::PrimTy;

    #[test]
    fn test_registry_new() {
        let registry = TypeRegistry::new();
        assert!(!registry.has_struct(Symbol::new(0)));
        assert!(!registry.has_enum(Symbol::new(0)));
    }

    #[test]
    fn test_register_struct() {
        let mut registry = TypeRegistry::new();
        let name = Symbol::new(0);

        let info = StructInfo {
            name,
            fields: vec![
                FieldInfo {
                    name: Symbol::new(1),
                    ty: Ty::Primitive(PrimTy::Int64),
                },
            ],
            methods: vec![],
            generics: vec![],
        };

        let mut registry = TypeRegistry::new();
        registry.register_struct(info);
        assert!(registry.has_struct(name));

        let lookup = registry.lookup_struct(name).unwrap();
        assert_eq!(lookup.fields.len(), 1);
    }

    #[test]
    fn test_register_enum() {
        let mut registry = TypeRegistry::new();
        let name = Symbol::new(0);

        let info = EnumInfo {
            name,
            variants: vec![
                EnumVariantInfo {
                    name: Symbol::new(1),
                    payload: Some(Ty::Primitive(PrimTy::Int64)),
                },
            ],
            methods: vec![],
            generics: vec![],
        };

        registry.register_enum(info);
        assert!(registry.has_enum(name));

        let lookup = registry.lookup_enum(name).unwrap();
        assert_eq!(lookup.variants.len(), 1);
    }
}
