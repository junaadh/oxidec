//! `Protocol` implementation for the `OxideC` runtime.
//!
//! This module implements **Protocols**, which define interfaces that classes can
//! conform to. Protocols are inspired by Objective-C's protocol system and Rust's
//! trait system.
//!
//! # Architecture
//!
//! Protocols are **globally registered** and never deallocated:
//! - Each protocol name maps to exactly one `Protocol` instance
//! - Protocols have `'static` lifetime (live for program duration)
//! - Immutable after creation (methods added during construction only)
//! - Classes declare protocol conformance without automatic validation
//!
//! # Method Resolution
//!
//! Protocols do **NOT** participate in normal message dispatch:
//! - Protocol methods are found only if class implements them
//! - No automatic protocol method fallback
//! - Protocols are for type checking and validation only
//!
//! # Hybrid Validation
//!
//! OxideC uses a **hybrid validation approach**:
//!
//! ## 1. Declarative (Default)
//! Classes declare protocol conformance without validation:
//! ```rust,ignore
//! class.add_protocol(&protocol).unwrap();  // No validation
//! ```
//!
//! ## 2. Optional Runtime Validation
//! Validate conformance when stricter safety is desired:
//! ```rust,ignore
//! class.validate_protocol_conformance(&protocol)?;  // Explicit validation
//! ```
//!
//! # Thread Safety
//!
//! Protocols are thread-safe and support concurrent access from multiple threads.
//! Uses `RwLock` for method tables and adopted classes tracking.

use crate::error::{Error, Result};
use crate::runtime::{RuntimeString, Selector, get_global_arena};
use std::collections::HashMap;
use std::fmt;
use std::ptr::NonNull;
use std::sync::RwLock;

/// Protocol method with selector and type encoding.
#[repr(C)]
pub(crate) struct ProtocolMethod {
    /// Method selector
    selector: Selector,
    /// Method type encoding
    types: RuntimeString,
}

/// Internal protocol data stored in global arena.
///
/// This struct is allocated in the global arena and never deallocated.
#[repr(C)]
pub(crate) struct ProtocolInner {
    /// Protocol name (e.g., "Copyable", "Iterable")
    name: RuntimeString,
    /// Required methods: selector hash -> ProtocolMethod
    required_methods: RwLock<HashMap<u64, ProtocolMethod>>,
    /// Optional methods: selector hash -> ProtocolMethod
    optional_methods: RwLock<HashMap<u64, ProtocolMethod>>,
    /// Classes that have adopted this protocol
    adopted_classes: RwLock<Vec<NonNull<crate::runtime::class::ClassInner>>>,
    /// Base protocol (for protocol inheritance)
    base_protocol: Option<NonNull<ProtocolInner>>,
    /// Adopted protocols (for protocol composition)
    adopted_protocols: RwLock<Vec<NonNull<ProtocolInner>>>,
}

/// `Protocol` represents an interface that classes can conform to.
///
/// Protocols define required and optional methods that a class must implement
/// to conform to the protocol. Similar to Rust traits or Objective-C protocols.
///
/// # Memory Management
///
/// Protocols use manual memory management:
/// - Allocated in global arena (stable pointers)
/// - Never deallocated (live for program duration)
/// - Methods are stored in the protocol's method tables
///
/// # Thread Safety
///
/// Protocols are `Send + Sync`:
/// - Method tables protected by `RwLock`
/// - Protocol metadata is immutable after construction
/// - Adopted classes list protected by `RwLock`
///
/// # Example
///
/// ```rust,ignore
/// use oxidec::{Class, Protocol, RuntimeString, Selector};
/// use oxidec::runtime::get_global_arena;
/// use std::str::FromStr;
///
/// // Create a protocol
/// let protocol = Protocol::new("Copyable", None).unwrap();
///
/// // Add required methods
/// let sel = Selector::from_str("copy").unwrap();
/// protocol.add_required(sel, "@@:", get_global_arena()).unwrap();
///
/// // Create a class
/// let class = Class::new_root("MyCopyableClass").unwrap();
///
/// // Add protocol conformance (declarative, no validation)
/// class.add_protocol(&protocol).unwrap();
///
/// // Optional: Validate conformance explicitly
/// class.validate_protocol_conformance(&protocol).unwrap();
/// ```
pub struct Protocol {
    /// Pointer to protocol data in global arena.
    /// Never null, valid for entire program lifetime.
    pub(crate) inner: NonNull<ProtocolInner>,
}

unsafe impl Send for Protocol {}
unsafe impl Sync for Protocol {}

impl Protocol {
    /// Creates a new protocol.
    ///
    /// # Arguments
    ///
    /// * `name` - Protocol name (must be unique globally)
    /// * `base_protocol` - Optional base protocol for inheritance
    ///
    /// # Returns
    ///
    /// Returns `Ok(Protocol)` if successful, `Err` if protocol name already
    /// exists globally.
    ///
    /// # Thread Safety
    ///
    /// Multiple threads can create protocols concurrently. The global registry
    /// ensures unique protocol names.
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::Protocol;
    ///
    /// let protocol = Protocol::new("MyProtocol", None).unwrap();
    /// assert_eq!(protocol.name(), "MyProtocol");
    /// ```
    ///
    /// # Errors
    ///
    /// Returns `Err(Error::ProtocolAlreadyExists)` if a protocol with this
    /// name already exists globally.
    pub fn new(name: &str, base_protocol: Option<&Protocol>) -> Result<Self> {
        // Allocate ProtocolInner in global arena
        let arena = get_global_arena();
        let name_str = RuntimeString::new(name, arena);

        let protocol_inner = ProtocolInner {
            name: name_str,
            required_methods: RwLock::new(HashMap::new()),
            optional_methods: RwLock::new(HashMap::new()),
            adopted_classes: RwLock::new(Vec::new()),
            base_protocol: base_protocol.map(|p| p.inner),
            adopted_protocols: RwLock::new(Vec::new()),
        };

        // Allocate in arena
        let ptr = arena.alloc(protocol_inner);
        if ptr.is_null() {
            return Err(Error::OutOfMemory);
        }
        let inner = unsafe { NonNull::new_unchecked(ptr as *mut ProtocolInner) };

        Ok(Protocol { inner })
    }

    /// Adds a required method to this protocol.
    ///
    /// # Arguments
    ///
    /// * `selector` - Method selector
    /// * `types` - Method type encoding string
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if successful, `Err` if method already exists.
    ///
    /// # Thread Safety
    ///
    /// Multiple threads can add methods concurrently.
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::{Protocol, RuntimeString, Selector};
    /// use oxidec::runtime::get_global_arena;
    /// use std::str::FromStr;
    ///
    /// let protocol = Protocol::new("MyProtocol", None).unwrap();
    /// let sel = Selector::from_str("doSomething").unwrap();
    /// protocol.add_required(sel, "v@:", get_global_arena()).unwrap();
    /// ```
    pub fn add_required(&self, selector: Selector, types: &str, arena: &crate::runtime::Arena) -> Result<()> {
        // SAFETY: self.inner points to valid ProtocolInner
        let inner = unsafe { &*self.inner.as_ptr() };

        let hash = selector.hash();
        let method = ProtocolMethod {
            selector,
            types: RuntimeString::new(types, arena),
        };

        // Add to required methods
        let mut required = inner.required_methods.write().unwrap();
        if required.contains_key(&hash) {
            return Err(Error::ProtocolMethodAlreadyRegistered);
        }
        required.insert(hash, method);

        Ok(())
    }

    /// Adds an optional method to this protocol.
    ///
    /// # Arguments
    ///
    /// * `selector` - Method selector
    /// * `types` - Method type encoding string
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if successful, `Err` if method already exists.
    ///
    /// # Thread Safety
    ///
    /// Multiple threads can add methods concurrently.
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::{Protocol, RuntimeString, Selector};
    /// use oxidec::runtime::get_global_arena;
    /// use std::str::FromStr;
    ///
    /// let protocol = Protocol::new("MyProtocol", None).unwrap();
    /// let sel = Selector::from_str("optionalMethod").unwrap();
    /// protocol.add_optional(sel, "v@:", get_global_arena()).unwrap();
    /// ```
    pub fn add_optional(&self, selector: Selector, types: &str, arena: &crate::runtime::Arena) -> Result<()> {
        // SAFETY: self.inner points to valid ProtocolInner
        let inner = unsafe { &*self.inner.as_ptr() };

        let hash = selector.hash();
        let method = ProtocolMethod {
            selector,
            types: RuntimeString::new(types, arena),
        };

        // Add to optional methods
        let mut optional = inner.optional_methods.write().unwrap();
        if optional.contains_key(&hash) {
            return Err(Error::ProtocolMethodAlreadyRegistered);
        }
        optional.insert(hash, method);

        Ok(())
    }

    /// Returns the protocol name.
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::Protocol;
    ///
    /// let protocol = Protocol::new("MyProtocol", None).unwrap();
    /// assert_eq!(protocol.name(), "MyProtocol");
    /// ```
    #[must_use]
    pub fn name(&self) -> &str {
        // SAFETY: self.inner points to valid ProtocolInner
        let inner = unsafe { &*self.inner.as_ptr() };
        inner.name.as_str().unwrap_or("<invalid>")
    }

    /// Returns all required methods declared in this protocol.
    ///
    /// Does not include methods from base protocols.
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::{Protocol, Selector};
    /// use oxidec::runtime::get_global_arena;
    /// use std::str::FromStr;
    ///
    /// let protocol = Protocol::new("MyProtocol", None).unwrap();
    /// let sel = Selector::from_str("requiredMethod").unwrap();
    /// protocol.add_required(sel.clone(), "v@:", get_global_arena()).unwrap();
    ///
    /// let required = protocol.required();
    /// assert_eq!(required.len(), 1);
    /// assert_eq!(required[0].name(), "requiredMethod");
    /// ```
    #[must_use]
    pub fn required(&self) -> Vec<Selector> {
        // SAFETY: self.inner points to valid ProtocolInner
        let inner = unsafe { &*self.inner.as_ptr() };
        let required = inner.required_methods.read().unwrap();
        required.values().map(|m| m.selector.clone()).collect()
    }

    /// Returns all optional methods declared in this protocol.
    ///
    /// Does not include methods from base protocols.
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::{Protocol, Selector};
    /// use oxidec::runtime::get_global_arena;
    /// use std::str::FromStr;
    ///
    /// let protocol = Protocol::new("MyProtocol", None).unwrap();
    /// let sel = Selector::from_str("optionalMethod").unwrap();
    /// protocol.add_optional(sel.clone(), "v@:", get_global_arena()).unwrap();
    ///
    /// let optional = protocol.optional();
    /// assert_eq!(optional.len(), 1);
    /// assert_eq!(optional[0].name(), "optionalMethod");
    /// ```
    #[must_use]
    pub fn optional(&self) -> Vec<Selector> {
        // SAFETY: self.inner points to valid ProtocolInner
        let inner = unsafe { &*self.inner.as_ptr() };
        let optional = inner.optional_methods.read().unwrap();
        optional.values().map(|m| m.selector.clone()).collect()
    }

    /// Returns the base protocol if this protocol inherits from another.
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::Protocol;
    ///
    /// let base = Protocol::new("BaseProtocol", None).unwrap();
    /// let derived = Protocol::new("DerivedProtocol", Some(&base)).unwrap();
    ///
    /// assert_eq!(derived.base_protocol().unwrap().name(), "BaseProtocol");
    /// ```
    #[must_use]
    pub fn base_protocol(&self) -> Option<Protocol> {
        // SAFETY: self.inner points to valid ProtocolInner
        let inner = unsafe { &*self.inner.as_ptr() };
        inner.base_protocol.map(|inner| Protocol { inner })
    }

    /// Returns all protocols adopted by this protocol (protocol composition).
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::Protocol;
    ///
    /// let proto1 = Protocol::new("Protocol1", None).unwrap();
    /// let proto2 = Protocol::new("Protocol2", None).unwrap();
    /// // Assume proto2 adopts proto1
    /// // let adopted = proto2.adopted_protocols();
    /// // assert_eq!(adopted.len(), 1);
    /// ```
    #[must_use]
    pub fn adopted_protocols(&self) -> Vec<Protocol> {
        // SAFETY: self.inner points to valid ProtocolInner
        let inner = unsafe { &*self.inner.as_ptr() };
        let adopted = inner.adopted_protocols.read().unwrap();
        adopted.iter().map(|&inner| Protocol { inner }).collect()
    }

    /// Gets all required methods (including from base protocols).
    ///
    /// This is used internally by `validate_protocol_conformance`.
    pub(crate) fn all_required(&self) -> Vec<(u64, Selector)> {
        let mut methods = Vec::new();

        // Add from base protocol first
        if let Some(base) = self.base_protocol() {
            methods.extend(base.all_required());
        }

        // Add from this protocol (overriding base if needed)
        let inner = unsafe { &*self.inner.as_ptr() };
        let required = inner.required_methods.read().unwrap();
        for (hash, method) in required.iter() {
            methods.push((*hash, method.selector.clone()));
        }

        methods
    }
}

impl fmt::Debug for Protocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // SAFETY: self.inner points to valid ProtocolInner
        let inner = unsafe { &*self.inner.as_ptr() };
        let required = inner.required_methods.read().unwrap();
        let optional = inner.optional_methods.read().unwrap();

        f.debug_struct("Protocol")
            .field("name", &inner.name.as_str().unwrap_or("<invalid>"))
            .field("required_count", &required.len())
            .field("optional_count", &optional.len())
            .field("base_protocol", &inner.base_protocol.map(|p| unsafe {
                (&*p.as_ptr()).name.as_str().unwrap_or("<invalid>").to_string()
            }))
            .finish()
    }
}

impl Clone for Protocol {
    fn clone(&self) -> Self {
        Protocol { inner: self.inner }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::{get_global_arena, Class, Category, Method};
    use std::str::FromStr;

    #[test]
    fn test_protocol_creation() {
        let protocol = Protocol::new("TestProtocol", None).unwrap();
        assert_eq!(protocol.name(), "TestProtocol");
    }

    #[test]
    fn test_protocol_with_base_protocol() {
        let base = Protocol::new("BaseProtocol", None).unwrap();
        let derived = Protocol::new("DerivedProtocol", Some(&base)).unwrap();

        assert_eq!(derived.base_protocol().unwrap().name(), "BaseProtocol");
    }

    #[test]
    fn test_add_required_method() {
        let protocol = Protocol::new("RequiredProtocol", None).unwrap();
        let sel = Selector::from_str("requiredMethod:").unwrap();

        protocol
            .add_required(sel.clone(), "v@:i", get_global_arena())
            .unwrap();

        let required = protocol.required();
        assert_eq!(required.len(), 1);
        assert_eq!(required[0].name(), "requiredMethod:");
    }

    #[test]
    fn test_add_optional_method() {
        let protocol = Protocol::new("OptionalProtocol", None).unwrap();
        let sel = Selector::from_str("optionalMethod").unwrap();

        protocol
            .add_optional(sel.clone(), "v@:", get_global_arena())
            .unwrap();

        let optional = protocol.optional();
        assert_eq!(optional.len(), 1);
        assert_eq!(optional[0].name(), "optionalMethod");
    }

    #[test]
    fn test_protocol_inheritance_methods() {
        let base = Protocol::new("BaseProtocol", None).unwrap();
        let sel1 = Selector::from_str("baseMethod").unwrap();
        base.add_required(sel1, "v@:", get_global_arena())
            .unwrap();

        let derived = Protocol::new("DerivedProtocol", Some(&base)).unwrap();
        let sel2 = Selector::from_str("derivedMethod").unwrap();
        derived.add_required(sel2, "v@:", get_global_arena())
            .unwrap();

        // all_required should include both base and derived methods
        let all_req = derived.all_required();
        assert_eq!(all_req.len(), 2);
    }

    #[test]
    fn test_protocol_debug() {
        let protocol = Protocol::new("DebugProtocol", None).unwrap();
        let debug_str = format!("{:?}", protocol);
        assert!(debug_str.contains("DebugProtocol"));
    }

    #[test]
    fn test_protocol_clone() {
        let protocol1 = Protocol::new("CloneProtocol", None).unwrap();
        let protocol2 = protocol1.clone();
        assert_eq!(protocol1.name(), protocol2.name());
    }

    #[test]
    fn test_protocol_adopted_by_class() {
        use crate::runtime::Class;

        let protocol = Protocol::new("AdoptableProtocol", None).unwrap();
        let class = Class::new_root("AdoptableClass").unwrap();

        // Add protocol to class
        class.add_protocol(&protocol).unwrap();

        // Check conformance
        assert!(class.conforms_to(&protocol));

        // Check protocols() returns the protocol
        let protocols = class.protocols();
        assert_eq!(protocols.len(), 1);
        assert_eq!(protocols[0].name(), "AdoptableProtocol");
    }

    #[test]
    fn test_duplicate_protocol_adoption() {
        use crate::runtime::Class;

        let protocol = Protocol::new("DuplicateProtocol", None).unwrap();
        let class = Class::new_root("DuplicateClass").unwrap();

        // Add protocol once
        class.add_protocol(&protocol).unwrap();

        // Try to add again - should fail
        let result = class.add_protocol(&protocol);
        assert!(result.is_err());
    }

    #[test]
    fn test_protocol_conformance_inheritance() {
        use crate::runtime::Class;

        let protocol = Protocol::new("InheritableProtocol", None).unwrap();
        let parent = Class::new_root("ParentClass").unwrap();
        let child = Class::new("ChildClass", &parent).unwrap();

        // Parent adopts protocol
        parent.add_protocol(&protocol).unwrap();

        // Child should conform through inheritance
        assert!(child.conforms_to(&protocol));
    }

    #[test]
    fn test_protocols_collected_with_inheritance() {
        use crate::runtime::Class;

        let proto1 = Protocol::new("Proto1", None).unwrap();
        let proto2 = Protocol::new("Proto2", None).unwrap();

        let parent = Class::new_root("ProtocolInheritParent").unwrap();
        let child = Class::new("ProtocolInheritChild", &parent).unwrap();

        // Parent adopts proto1
        parent.add_protocol(&proto1).unwrap();

        // Child adopts proto2
        child.add_protocol(&proto2).unwrap();

        // Child should have both protocols
        let protocols = child.protocols();
        assert_eq!(protocols.len(), 2);

        let protocol_names: Vec<&str> = protocols.iter().map(|p| p.name()).collect();
        assert!(protocol_names.contains(&"Proto1"));
        assert!(protocol_names.contains(&"Proto2"));
    }

    #[test]
    fn test_validate_protocol_conformance_valid() {
        use crate::runtime::{Class, Method};
        use std::str::FromStr;

        let protocol = Protocol::new("ValidProtocol", None).unwrap();
        let sel = Selector::from_str("requiredMethod").unwrap();
        protocol
            .add_required(sel.clone(), "v@:", get_global_arena())
            .unwrap();

        let class = Class::new_root("ValidClass").unwrap();

        // Add protocol to class
        class.add_protocol(&protocol).unwrap();

        // Validation should fail (method not implemented)
        assert!(class.validate_protocol_conformance(&protocol).is_err());

        // Add the required method
        let method = Method {
            selector: sel.clone(),
            imp: test_method_impl,
            types: RuntimeString::new("v@:", get_global_arena()),
        };
        class.add_method(method).unwrap();

        // Now validation should pass
        class.validate_protocol_conformance(&protocol).unwrap();
    }

    #[test]
    fn test_validate_protocol_conformance_missing_method() {
        use crate::runtime::Class;
        use std::str::FromStr;

        let protocol = Protocol::new("InvalidProtocol", None).unwrap();
        let sel = Selector::from_str("missingMethod").unwrap();
        protocol
            .add_required(sel.clone(), "v@:", get_global_arena())
            .unwrap();

        let class = Class::new_root("InvalidClass").unwrap();
        class.add_protocol(&protocol).unwrap();

        // Validation should fail
        let result = class.validate_protocol_conformance(&protocol);
        assert!(result.is_err());

        if let Err(Error::MissingProtocolMethod { selector }) = result {
            assert_eq!(selector, "missingMethod");
        } else {
            panic!("Expected MissingProtocolMethod error");
        }
    }

    #[test]
    fn test_validate_with_category_methods() {
        use crate::runtime::{Category, Class, Method};
        use std::str::FromStr;

        let protocol = Protocol::new("CategoryProtocol", None).unwrap();
        let sel = Selector::from_str("categoryMethod").unwrap();
        protocol
            .add_required(sel.clone(), "v@:", get_global_arena())
            .unwrap();

        let class = Class::new_root("CategoryClass").unwrap();
        class.add_protocol(&protocol).unwrap();

        // Add method via category
        let category = Category::new("MethodCategory", &class).unwrap();
        let method = Method {
            selector: sel.clone(),
            imp: test_method_impl,
            types: RuntimeString::new("v@:", get_global_arena()),
        };
        category.add_method(method).unwrap();

        // Validation should pass (method found in category)
        class.validate_protocol_conformance(&protocol).unwrap();
    }

    #[test]
    fn test_validate_with_inheritance() {
        use crate::runtime::{Class, Method};
        use std::str::FromStr;

        let protocol = Protocol::new("InheritedProtocol", None).unwrap();
        let sel = Selector::from_str("inheritedMethod").unwrap();
        protocol
            .add_required(sel.clone(), "v@:", get_global_arena())
            .unwrap();

        let parent = Class::new_root("ValidateInheritParent").unwrap();
        let child = Class::new("ValidateInheritChild", &parent).unwrap();

        // Add method to parent
        let method = Method {
            selector: sel.clone(),
            imp: test_method_impl,
            types: RuntimeString::new("v@:", get_global_arena()),
        };
        parent.add_method(method).unwrap();

        // Child adopts protocol
        child.add_protocol(&protocol).unwrap();

        // Validation should pass (method found in parent)
        child.validate_protocol_conformance(&protocol).unwrap();
    }

    #[test]
    fn test_protocol_inheritance_validation() {
        use crate::runtime::Class;

        // Create base protocol with required methods
        let base = Protocol::new("BaseProtocol", None).unwrap();
        let sel1 = Selector::from_str("baseMethod").unwrap();
        base.add_required(sel1, "v@:", get_global_arena())
            .unwrap();

        // Create derived protocol that inherits from base
        let derived = Protocol::new("DerivedProtocol", Some(&base)).unwrap();
        let sel2 = Selector::from_str("derivedMethod").unwrap();
        derived.add_required(sel2, "v@:", get_global_arena())
            .unwrap();

        let class = Class::new_root("InheritanceClass").unwrap();
        class.add_protocol(&derived).unwrap();

        // Validation should fail (both methods missing)
        assert!(class.validate_protocol_conformance(&derived).is_err());
    }

    #[test]
    fn test_multiple_protocols_on_class() {
        use crate::runtime::Class;

        let proto1 = Protocol::new("Protocol1", None).unwrap();
        let proto2 = Protocol::new("Protocol2", None).unwrap();
        let proto3 = Protocol::new("Protocol3", None).unwrap();

        let class = Class::new_root("MultiProtocolClass").unwrap();

        // Add all protocols
        class.add_protocol(&proto1).unwrap();
        class.add_protocol(&proto2).unwrap();
        class.add_protocol(&proto3).unwrap();

        // Check conformance
        assert!(class.conforms_to(&proto1));
        assert!(class.conforms_to(&proto2));
        assert!(class.conforms_to(&proto3));

        // Check protocols() returns all
        let protocols = class.protocols();
        assert_eq!(protocols.len(), 3);
    }

    #[test]
    fn test_protocol_with_optional_methods() {
        use crate::runtime::Class;
        use std::str::FromStr;

        let protocol = Protocol::new("OptionalProtocol", None).unwrap();
        let req_sel = Selector::from_str("requiredMethod").unwrap();
        let opt_sel = Selector::from_str("optionalMethod").unwrap();

        protocol
            .add_required(req_sel.clone(), "v@:", get_global_arena())
            .unwrap();
        protocol
            .add_optional(opt_sel.clone(), "v@:", get_global_arena())
            .unwrap();

        let class = Class::new_root("OptionalClass").unwrap();
        class.add_protocol(&protocol).unwrap();

        // Add only the required method
        let method = Method {
            selector: req_sel.clone(),
            imp: test_method_impl,
            types: RuntimeString::new("v@:", get_global_arena()),
        };
        class.add_method(method).unwrap();

        // Validation should pass (optional methods don't need to be implemented)
        class.validate_protocol_conformance(&protocol).unwrap();
    }

    // Test method implementation
    unsafe extern "C" fn test_method_impl(
        _self: crate::runtime::object::ObjectPtr,
        _cmd: crate::runtime::selector::SelectorHandle,
        _args: *const *mut u8,
        _ret: *mut u8,
    ) {
    }
}
