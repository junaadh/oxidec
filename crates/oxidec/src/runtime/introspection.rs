//! Runtime introspection APIs for dynamic runtime inspection.
//!
//! This module provides APIs for inspecting and querying the runtime state,
//! including:
//!
//! - **Class enumeration** - List all registered classes, find by name
//! - **Method enumeration** - List methods for a class, find implementations
//! - **Protocol inspection** - List protocols, check conformance
//! - **Dynamic class creation** - Build classes at runtime
//!
//! # Example
//!
//! ```rust
//! use oxidec::runtime::introspection::*;
//! use oxidec::runtime::Class;
//!
//! // Enumerate all classes
//! let classes = all_classes();
//! println!("Total classes: {}", classes.len());
//!
//! // Get specific class
//! if let Some(class) = class_from_name("MyClass") {
//!     // List methods
//!     let methods = instance_methods(&class);
//!     for method in methods {
//!         println!("Method: {:?}", method.selector.name());
//!     }
//! }
//! ```

use crate::runtime::{Class, Method, Selector, Protocol, Object};
use crate::error::Result;
use std::sync::RwLock;
use std::collections::HashMap;

use super::get_global_arena;

// ============================================================================
// Class Registry
// ============================================================================

/// Global class registry for tracking all created classes.
///
/// This registry maintains weak references to all classes to allow
/// enumeration without preventing garbage collection.
static CLASS_REGISTRY: std::sync::OnceLock<std::sync::RwLock<HashMap<String, Class>>> =
    std::sync::OnceLock::new();

/// Register a class in the global registry.
///
/// This is called automatically by `Class::new_root` and `Class::new`.
pub(crate) fn register_class(class: &Class) {
    let name = class.name().to_string();
    CLASS_REGISTRY
        .get_or_init(|| RwLock::new(HashMap::new()))
        .write()
        .unwrap()
        .insert(name, class.clone());
}

/// Enumerate all registered classes.
///
/// Returns a vector of all classes currently registered in the runtime.
/// Classes that have been dropped will not appear in the list.
///
/// # Returns
///
/// A vector of `Class` instances.
///
/// # Example
///
/// ```rust
/// use oxidec::runtime::introspection::all_classes;
///
/// let classes = all_classes();
/// println!("Total classes: {}", classes.len());
/// ```
#[must_use]
pub fn all_classes() -> Vec<Class> {
    CLASS_REGISTRY
        .get_or_init(|| RwLock::new(HashMap::new()))
        .read()
        .unwrap()
        .values()
        .cloned()
        .collect()
}

/// Get a class by name.
///
/// Searches the global registry for a class with the given name.
///
/// # Arguments
///
/// * `name` - The name of the class to find
///
/// # Returns
///
/// `Some(Class)` if found, `None` otherwise.
///
/// # Example
///
/// ```rust
/// use oxidec::runtime::introspection::class_from_name;
///
/// if let Some(class) = class_from_name("MyClass") {
///     println!("Found class: {}", class.name());
/// }
/// ```
#[must_use]
pub fn class_from_name(name: &str) -> Option<Class> {
    CLASS_REGISTRY
        .get_or_init(|| RwLock::new(HashMap::new()))
        .read()
        .unwrap()
        .get(name)
        .cloned()
}

/// Get the class hierarchy from a class to root.
///
/// Returns a vector representing the inheritance chain, starting with
/// the given class and ending with the root class.
///
/// # Arguments
///
/// * `class` - The class to get the hierarchy for
///
/// # Returns
///
/// A vector of classes in inheritance order.
///
/// # Example
///
/// ```rust
/// use oxidec::runtime::{Class, introspection::class_hierarchy};
///
/// let class = Class::new_root("MyClass").unwrap();
/// let hierarchy = class_hierarchy(&class);
/// println!("Inheritance depth: {}", hierarchy.len());
/// ```
#[must_use]
pub fn class_hierarchy(class: &Class) -> Vec<Class> {
    let mut hierarchy = Vec::new();
    let mut current = Some(class.clone());

    while let Some(cls) = current {
        hierarchy.push(cls.clone());
        current = cls.super_class();
    }

    hierarchy
}

/// Check if a child class is a subclass of a parent class.
///
/// # Arguments
///
/// * `child` - The potential child class
/// * `parent` - The potential parent class
///
/// # Returns
///
/// `true` if `child` is a subclass of `parent`, `false` otherwise.
///
/// # Example
///
/// ```rust
/// use oxidec::runtime::{Class, introspection::is_subclass};
///
/// let parent = Class::new_root("Parent").unwrap();
/// let child = Class::new("Child", &parent).unwrap();
///
/// assert!(is_subclass(&child, &parent));
/// assert!(!is_subclass(&parent, &child));
/// ```
#[must_use]
pub fn is_subclass(child: &Class, parent: &Class) -> bool {
    class_hierarchy(child)
        .iter()
        .any(|c| c.name() == parent.name())
}

// ============================================================================
// Method Introspection
// ============================================================================

/// Enumerate all instance methods for a class.
///
/// Collects all instance methods from the class and its superclasses,
/// with subclass methods overriding superclass methods.
///
/// # Arguments
///
/// * `class` - The class to enumerate methods for
///
/// # Returns
///
/// A vector of methods, with subclass methods appearing before
/// superclass methods of the same selector.
///
/// # Example
///
/// ```rust
/// use oxidec::runtime::{Class, introspection::instance_methods};
///
/// let class = Class::new_root("MyClass").unwrap();
/// let methods = instance_methods(&class);
/// println!("Instance methods: {}", methods.len());
/// ```
#[must_use]
pub fn instance_methods(class: &Class) -> Vec<Method> {
    let mut methods = Vec::new();
    let mut current = Some(class.clone());

    // Walk the class hierarchy from subclass to superclass
    while let Some(cls) = current {
        // Get all methods from this class
        let class_methods = cls.get_all_methods();

        // Add all methods from this class
        methods.extend(class_methods);

        current = cls.super_class();
    }

    // Deduplicate by selector name (keep first occurrence, which is from subclass)
    let mut seen = std::collections::HashSet::new();
    methods.retain(|method| seen.insert(method.selector.name().to_string()));

    methods
}

/// Enumerate all class methods for a class.
///
/// Note: This is a placeholder for future class method support.
/// Currently returns an empty vector.
///
/// # Arguments
///
/// * `_class` - The class to enumerate class methods for
///
/// # Returns
///
/// An empty vector (placeholder).
#[must_use]
pub fn class_methods(_class: &Class) -> Vec<Method> {
    // TODO: Implement class methods when they're added to the runtime
    Vec::new()
}

/// Check if a class responds to a selector.
///
/// Searches the class hierarchy for a method matching the selector.
///
/// # Arguments
///
/// * `class` - The class to check
/// * `selector` - The selector to look for
///
/// # Returns
///
/// `true` if the class or any superclass implements the selector.
///
/// # Example
///
/// ```rust
/// use oxidec::runtime::{Class, Selector, introspection::has_method};
/// use std::str::FromStr;
///
/// let class = Class::new_root("MyClass").unwrap();
/// let selector = Selector::from_str("doSomething:").unwrap();
///
/// if has_method(&class, &selector) {
///     println!("Class responds to doSomething:");
/// }
/// ```
#[must_use]
pub fn has_method(class: &Class, selector: &Selector) -> bool {
    class.lookup_method(selector).is_some()
}

/// Find which class in the hierarchy provides a method implementation.
///
/// Searches the class hierarchy to find which class actually implements
/// the given selector.
///
/// # Arguments
///
/// * `class` - The class to start searching from
/// * `selector` - The selector to find
///
/// # Returns
///
/// `Some(Class)` if the method is found, `None` otherwise.
///
/// # Example
///
/// ```rust
/// use oxidec::runtime::{Class, Selector, introspection::method_provider};
/// use std::str::FromStr;
///
/// let class = Class::new_root("MyClass").unwrap();
/// let selector = Selector::from_str("doSomething:").unwrap();
///
/// if let Some(provider) = method_provider(&class, &selector) {
///     println!("Method implemented by: {}", provider.name());
/// }
/// ```
#[must_use]
pub fn method_provider(class: &Class, selector: &Selector) -> Option<Class> {
    let mut current = Some(class.clone());

    while let Some(cls) = current {
        // Check if this class directly implements the method (not inherited)
        let methods = cls.get_all_methods();
        if methods.iter().any(|m| m.selector.name() == selector.name()) {
            return Some(cls);
        }
        current = cls.super_class();
    }

    None
}

/// Get all subclasses of a given class.
///
/// Searches the class registry for all classes that inherit from
/// the given class.
///
/// # Arguments
///
/// * `parent` - The parent class
///
/// # Returns
///
/// A vector of classes that are subclasses of `parent`.
///
/// # Example
///
/// ```rust
/// use oxidec::runtime::{Class, introspection::subclasses};
///
/// let parent = Class::new_root("Parent").unwrap();
/// let _child1 = Class::new("Child1", &parent).unwrap();
/// let _child2 = Class::new("Child2", &parent).unwrap();
///
/// let children = subclasses(&parent);
/// println!("Subclasses: {}", children.len());
/// ```
#[must_use]
pub fn subclasses(parent: &Class) -> Vec<Class> {
    all_classes()
        .into_iter()
        .filter(|class| {
            // Compare by name since we can't directly compare Class instances
            class.name() != parent.name() && is_subclass(class, parent)
        })
        .collect()
}

// ============================================================================
// Protocol Introspection
// ============================================================================

/// Enumerate all protocols.
///
/// Returns a vector of all protocols currently registered in the runtime.
///
/// # Returns
///
/// A vector of protocols.
///
/// # Example
///
/// ```rust
/// use oxidec::runtime::introspection::all_protocols;
///
/// let protocols = all_protocols();
/// println!("Total protocols: {}", protocols.len());
/// ```
#[must_use]
pub fn all_protocols() -> Vec<Protocol> {
    // TODO: Implement global protocol registry
    // For now, return empty vector
    Vec::new()
}

/// Get protocols adopted by a class.
///
/// Returns all protocols that the class conforms to, including inherited
/// protocols.
///
/// # Arguments
///
/// * `class` - The class to query
///
/// # Returns
///
/// A vector of protocols.
///
/// # Example
///
/// ```rust
/// use oxidec::runtime::{Class, introspection::adopted_protocols};
///
/// let class = Class::new_root("MyClass").unwrap();
/// let protocols = adopted_protocols(&class);
/// println!("Adopted protocols: {}", protocols.len());
/// ```
#[must_use]
pub fn adopted_protocols(class: &Class) -> Vec<Protocol> {
    class.protocols()
}

/// Check if a class conforms to a protocol.
///
/// Searches the class hierarchy for protocol conformance.
///
/// # Arguments
///
/// * `class` - The class to check
/// * `protocol` - The protocol to check for
///
/// # Returns
///
/// `true` if the class conforms to the protocol.
///
/// # Example
///
/// ```rust
/// use oxidec::runtime::{Class, Protocol, introspection::conforms_to};
///
/// let class = Class::new_root("MyClass").unwrap();
/// let protocol = Protocol::new("MyProtocol", None).unwrap();
///
/// if conforms_to(&class, &protocol) {
///     println!("Class conforms to protocol");
/// }
/// ```
#[must_use]
pub fn conforms_to(class: &Class, protocol: &Protocol) -> bool {
    let mut current = Some(class.clone());

    while let Some(cls) = current {
        for adopted in cls.protocols() {
            if protocol_matches(&adopted, protocol) {
                return true;
            }
        }
        current = cls.super_class();
    }

    false
}

/// Check if two protocols match (by name or equality).
fn protocol_matches(a: &Protocol, b: &Protocol) -> bool {
    a.name() == b.name() || std::ptr::eq(a, b)
}

// ============================================================================
// Dynamic Class Creation
// ============================================================================

/// Builder for dynamically creating classes at runtime.
///
/// Provides a fluent API for creating classes with methods, instance
/// variables, and protocols.
///
/// # Example
///
/// ```rust
/// use oxidec::runtime::{Class, introspection::ClassBuilder};
///
/// let builder = ClassBuilder::new("DynamicClass", None);
/// // Add methods and protocols...
/// let class = builder.register().unwrap();
/// ```
pub struct ClassBuilder {
    name: String,
    superclass: Option<Class>,
    methods: Vec<(Selector, super::class::Imp)>,
    protocols: Vec<Protocol>,
}

impl ClassBuilder {
    /// Create a new class builder.
    ///
    /// # Arguments
    ///
    /// * `name` - The name for the new class
    /// * `superclass` - Optional superclass
    ///
    /// # Returns
    ///
    /// A new `ClassBuilder` instance.
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::runtime::introspection::ClassBuilder;
    ///
    /// let builder = ClassBuilder::new("MyClass", None);
    /// ```
    pub fn new(name: &str, superclass: Option<&Class>) -> Self {
        Self {
            name: name.to_string(),
            superclass: superclass.cloned(),
            methods: Vec::new(),
            protocols: Vec::new(),
        }
    }

    /// Add an instance method to the class.
    ///
    /// # Arguments
    ///
    /// * `selector` - The method selector
    /// * `imp` - The method implementation
    ///
    /// # Returns
    ///
    /// `&mut self` for chaining.
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::runtime::{Selector, introspection::ClassBuilder};
    /// use std::str::FromStr;
    ///
    /// let mut builder = ClassBuilder::new("MyClass", None);
    /// let selector = Selector::from_str("doSomething:").unwrap();
    ///
    /// // Add your method implementation here
    /// // builder.add_method(selector, my_implementation);
    /// ```
    pub fn add_method(&mut self, selector: Selector, imp: super::class::Imp) -> &mut Self {
        self.methods.push((selector, imp));
        self
    }

    /// Add a protocol to the class.
    ///
    /// # Arguments
    ///
    /// * `protocol` - The protocol to adopt
    ///
    /// # Returns
    ///
    /// `&mut self` for chaining.
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::runtime::{Protocol, introspection::ClassBuilder};
    ///
    /// let mut builder = ClassBuilder::new("MyClass", None);
    /// let protocol = Protocol::new("MyProtocol", None).unwrap();
    ///
    /// builder.add_protocol(&protocol);
    /// ```
    pub fn add_protocol(&mut self, protocol: &Protocol) -> &mut Self {
        self.protocols.push(protocol.clone());
        self
    }

    /// Register the class with the runtime.
    ///
    /// Creates the class and registers all methods and protocols.
    ///
    /// # Returns
    ///
    /// `Ok(Class)` if successful, `Err` otherwise.
    ///
    /// # Errors
    ///
    /// Returns `Error::ClassAlreadyExists` if a class with this name
    /// already exists.
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::runtime::introspection::ClassBuilder;
    ///
    /// let builder = ClassBuilder::new("MyClass", None);
    /// let class = builder.register().unwrap();
    /// ```
    pub fn register(self) -> Result<Class> {
        // Create the class
        let class = if let Some(ref superclass) = self.superclass {
            Class::new(&self.name, superclass)?
        } else {
            Class::new_root(&self.name)?
        };

        // Add methods
        for (selector, imp) in self.methods {
            let method = Method {
                selector,
                imp,
                types: crate::runtime::RuntimeString::new("", get_global_arena()),
            };
            class.add_method(method)?;
        }

        // Add protocols
        for protocol in &self.protocols {
            class.add_protocol(protocol)?;
        }

        Ok(class)
    }
}

/// Allocate a class dynamically.
///
/// Convenience function that creates a `ClassBuilder` for you.
///
/// # Arguments
///
/// * `name` - The name for the new class
/// * `superclass` - Optional superclass
///
/// # Returns
///
/// A new `ClassBuilder` instance.
///
/// # Example
///
/// ```rust
/// use oxidec::runtime::introspection::allocate_class;
///
/// let builder = allocate_class("MyClass", None);
/// let class = builder.register().unwrap();
/// ```
#[must_use]
pub fn allocate_class(name: &str, superclass: Option<&Class>) -> ClassBuilder {
    ClassBuilder::new(name, superclass)
}

// ============================================================================
// Object Introspection
// ============================================================================

/// Get the class of an object.
///
/// This is a convenience wrapper around `Object::class`.
///
/// # Arguments
///
/// * `object` - The object to query
///
/// # Returns
///
/// The object's class.
///
/// # Example
///
/// ```rust
/// use oxidec::runtime::{Class, Object, introspection::object_get_class};
///
/// let class = Class::new_root("MyClass").unwrap();
/// let object = Object::new(&class).unwrap();
///
/// let obj_class = object_get_class(&object);
/// assert_eq!(obj_class.name(), "MyClass");
/// ```
#[must_use]
pub fn object_get_class(object: &Object) -> Class {
    object.class()
}

/// Check if an object is an instance of a class.
///
/// Returns `true` if the object is an instance of the specified class
/// or any of its subclasses.
///
/// # Arguments
///
/// * `object` - The object to check
/// * `class` - The class to check against
///
/// # Returns
///
/// `true` if the object is an instance of the class.
///
/// # Example
///
/// ```rust
/// use oxidec::runtime::{Class, Object, introspection::object_is_instance};
///
/// let class = Class::new_root("MyClass").unwrap();
/// let object = Object::new(&class).unwrap();
///
/// assert!(object_is_instance(&object, &class));
/// ```
#[must_use]
pub fn object_is_instance(object: &Object, class: &Class) -> bool {
    is_subclass(&object.class(), class)
}

/// Check if an object responds to a selector.
///
/// Returns `true` if the object's class implements the selector.
///
/// # Arguments
///
/// * `object` - The object to check
/// * `selector` - The selector to check for
///
/// # Returns
///
/// `true` if the object responds to the selector.
///
/// # Example
///
/// ```rust
/// use oxidec::runtime::{Class, Object, Selector, introspection::object_responds_to};
/// use std::str::FromStr;
///
/// let class = Class::new_root("MyClass").unwrap();
/// let object = Object::new(&class).unwrap();
/// let selector = Selector::from_str("doSomething:").unwrap();
///
/// if object_responds_to(&object, &selector) {
///     println!("Object responds to selector");
/// }
/// ```
#[must_use]
pub fn object_responds_to(object: &Object, selector: &Selector) -> bool {
    has_method(&object.class(), selector)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::str::FromStr;

    static TEST_ID: AtomicUsize = AtomicUsize::new(0);

    fn setup_test_class() -> Class {
        let id = TEST_ID.fetch_add(1, Ordering::SeqCst);
        let class_name = format!("IntrospectionTest_{}", id);
        Class::new_root(&class_name).unwrap()
    }

    #[test]
    fn test_all_classes() {
        let class = setup_test_class();
        let classes = all_classes();

        assert!(classes.len() >= 1);
        assert!(classes.iter().any(|c| c.name() == class.name()));
    }

    #[test]
    fn test_class_from_name() {
        let class = setup_test_class();

        let found = class_from_name(class.name());
        assert!(found.is_some());
        assert_eq!(found.unwrap().name(), class.name());

        let not_found = class_from_name("NonExistentClass");
        assert!(not_found.is_none());
    }

    #[test]
    fn test_class_hierarchy() {
        let root = setup_test_class();
        let id = TEST_ID.fetch_add(1, Ordering::SeqCst);
        let child = Class::new(&format!("Child_{}", id), &root).unwrap();

        let hierarchy = class_hierarchy(&child);
        assert_eq!(hierarchy.len(), 2);
        assert_eq!(hierarchy[0].name(), child.name());
        assert_eq!(hierarchy[1].name(), root.name());
    }

    #[test]
    fn test_is_subclass() {
        let parent = setup_test_class();
        let id = TEST_ID.fetch_add(1, Ordering::SeqCst);
        let child = Class::new(&format!("Child_{}", id), &parent).unwrap();

        assert!(is_subclass(&child, &parent));
        assert!(!is_subclass(&parent, &child));
        assert!(is_subclass(&child, &child)); // Class is subclass of itself
    }

    #[test]
    fn test_instance_methods() {
        let class = setup_test_class();
        let methods = instance_methods(&class);

        // New class should have no methods
        assert_eq!(methods.len(), 0);
    }

    #[test]
    fn test_has_method() {
        let class = setup_test_class();
        let selector = Selector::from_str("nonExistentMethod:").unwrap();

        assert!(!has_method(&class, &selector));
    }

    #[test]
    fn test_method_provider() {
        let class = setup_test_class();
        let selector = Selector::from_str("nonExistentMethod:").unwrap();

        let provider = method_provider(&class, &selector);
        assert!(provider.is_none());
    }

    #[test]
    fn test_subclasses() {
        let parent = setup_test_class();

        let id1 = TEST_ID.fetch_add(1, Ordering::SeqCst);
        let child1 = Class::new(&format!("Child1_{}", id1), &parent).unwrap();

        let id2 = TEST_ID.fetch_add(1, Ordering::SeqCst);
        let child2 = Class::new(&format!("Child2_{}", id2), &parent).unwrap();

        let children = subclasses(&parent);
        assert_eq!(children.len(), 2);
        assert!(children.iter().any(|c| c.name() == child1.name()));
        assert!(children.iter().any(|c| c.name() == child2.name()));
    }

    #[test]
    fn test_adopted_protocols() {
        let class = setup_test_class();
        let protocols = adopted_protocols(&class);

        // New class should have no protocols
        assert_eq!(protocols.len(), 0);
    }

    #[test]
    fn test_class_builder() {
        let id = TEST_ID.fetch_add(1, Ordering::SeqCst);
        let name = format!("BuilderTest_{}", id);

        let builder = ClassBuilder::new(&name, None);
        let class = builder.register();

        assert!(class.is_ok());
        assert_eq!(class.unwrap().name(), name);
    }

    #[test]
    fn test_class_builder_with_superclass() {
        let parent = setup_test_class();
        let id = TEST_ID.fetch_add(1, Ordering::SeqCst);
        let name = format!("BuilderTest_{}", id);

        let builder = ClassBuilder::new(&name, Some(&parent));
        let class = builder.register();

        assert!(class.is_ok());
        let class = class.unwrap();
        assert_eq!(class.name(), name);
        assert_eq!(class.super_class().unwrap().name(), parent.name());
    }

    #[test]
    fn test_object_get_class() {
        let class = setup_test_class();
        let object = Object::new(&class).unwrap();

        let obj_class = object_get_class(&object);
        assert_eq!(obj_class.name(), class.name());
    }

    #[test]
    fn test_object_is_instance() {
        let class = setup_test_class();
        let object = Object::new(&class).unwrap();

        assert!(object_is_instance(&object, &class));
    }

    #[test]
    fn test_object_responds_to() {
        let class = setup_test_class();
        let object = Object::new(&class).unwrap();
        let selector = Selector::from_str("someMethod:").unwrap();

        assert!(!object_responds_to(&object, &selector));
    }

    #[test]
    fn test_allocate_class() {
        let id = TEST_ID.fetch_add(1, Ordering::SeqCst);
        let name = format!("Allocated_{}", id);

        let builder = allocate_class(&name, None);
        let class = builder.register();

        assert!(class.is_ok());
        assert_eq!(class.unwrap().name(), name);
    }
}
