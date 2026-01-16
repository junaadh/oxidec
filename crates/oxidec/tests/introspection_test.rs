//! Introspection API tests
//!
//! Tests for runtime introspection functionality:
//! - Class enumeration
//! - Method enumeration
//! - Protocol queries
//! - Hierarchy traversal
//! - Dynamic class creation
//!
//! Run with: `cargo test --test introspection_test`

use oxidec::runtime::{Class, Object, Protocol, Selector, introspection::*};
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};

static TEST_ID: AtomicUsize = AtomicUsize::new(0);

fn setup_test_class() -> (Class, Object) {
    let id = TEST_ID.fetch_add(1, Ordering::SeqCst);
    let class_name = format!("IntrospectionTest_{}", id);
    let class = Class::new_root(&class_name).unwrap();
    let object = Object::new(&class).unwrap();
    (class, object)
}

// ============================================================================
// Class Enumeration Tests
// ============================================================================

#[test]
fn test_all_classes() {
    let (_class, _) = setup_test_class();

    let classes = all_classes();
    assert!(!classes.is_empty(), "Should have at least one class");

    // Our test class should be in the list
    let found = classes
        .iter()
        .any(|c| c.name().starts_with("IntrospectionTest_"));
    assert!(found, "Test class should be registered");
}

#[test]
fn test_class_from_name() {
    let (class, _) = setup_test_class();
    let class_name = class.name().to_string();

    let found = class_from_name(&class_name);
    assert!(found.is_some(), "Should find class by name");

    let found_class = found.unwrap();
    assert_eq!(found_class.name(), class.name());
}

#[test]
fn test_class_from_name_nonexistent() {
    let found = class_from_name("NonexistentClass");
    assert!(found.is_none(), "Should not find nonexistent class");
}

#[test]
fn test_class_hierarchy_root() {
    let (class, _) = setup_test_class();

    let hierarchy = class_hierarchy(&class);
    assert_eq!(
        hierarchy.len(),
        1,
        "Root class should have hierarchy of length 1"
    );
    assert_eq!(hierarchy[0].name(), class.name());
}

#[test]
fn test_class_hierarchy_inheritance() {
    let id = TEST_ID.fetch_add(1, Ordering::SeqCst);
    let parent_name = format!("Parent_{}", id);
    let child_name = format!("Child_{}", id);

    let parent = Class::new_root(&parent_name).unwrap();
    let child = Class::new(&child_name, &parent).unwrap();

    let hierarchy = class_hierarchy(&child);
    assert_eq!(hierarchy.len(), 2, "Child should have 2-level hierarchy");
    assert_eq!(hierarchy[0].name(), child_name);
    assert_eq!(hierarchy[1].name(), parent_name);
}

#[test]
fn test_is_subclass_true() {
    let id = TEST_ID.fetch_add(1, Ordering::SeqCst);
    let parent_name = format!("Parent_{}", id);
    let child_name = format!("Child_{}", id);

    let parent = Class::new_root(&parent_name).unwrap();
    let child = Class::new(&child_name, &parent).unwrap();

    assert!(is_subclass(&child, &parent));
    assert!(is_subclass(&child, &child), "Class is subclass of itself");
}

#[test]
fn test_is_subclass_false() {
    let id = TEST_ID.fetch_add(1, Ordering::SeqCst);
    let parent = Class::new_root(&format!("Parent_{}", id)).unwrap();
    let child = Class::new(&format!("Child_{}", id), &parent).unwrap();

    assert!(!is_subclass(&parent, &child));
}

#[test]
fn test_subclasses() {
    let id = TEST_ID.fetch_add(1, Ordering::SeqCst);
    let parent_name = format!("Parent_{}", id);

    let parent = Class::new_root(&parent_name).unwrap();
    let child1 = Class::new(&format!("Child1_{}", id), &parent).unwrap();
    let child2 = Class::new(&format!("Child2_{}", id), &parent).unwrap();

    let children = subclasses(&parent);

    // Should contain both children
    assert!(children.iter().any(|c| c.name() == child1.name()));
    assert!(children.iter().any(|c| c.name() == child2.name()));

    // Should not contain parent itself
    assert!(!children.iter().any(|c| c.name() == parent.name()));
}

// ============================================================================
// Method Introspection Tests
// ============================================================================

#[test]
fn test_instance_methods_empty() {
    let (class, _) = setup_test_class();

    let methods = instance_methods(&class);
    assert_eq!(
        methods.len(),
        0,
        "New class should have no instance methods"
    );
}

#[test]
fn test_has_method_false() {
    let (class, _) = setup_test_class();
    let selector = Selector::from_str("nonexistentMethod:").unwrap();

    assert!(!has_method(&class, &selector));
}

#[test]
fn test_method_provider() {
    let id = TEST_ID.fetch_add(1, Ordering::SeqCst);
    let parent_name = format!("Parent_{}", id);
    let child_name = format!("Child_{}", id);
    let selector = Selector::from_str("testMethod:").unwrap();

    let parent = Class::new_root(&parent_name).unwrap();

    // Add method to parent
    use oxidec::runtime::Method;
    use oxidec::runtime::RuntimeString;
    use oxidec::runtime::get_global_arena;
    use oxidec::runtime::object::ObjectPtr;
    use oxidec::runtime::selector::SelectorHandle;

    // Dummy function pointer for testing (never called)
    unsafe extern "C" fn dummy_method(
        _self: ObjectPtr,
        _cmd: SelectorHandle,
        _args: *const *mut u8,
        _ret: *mut u8,
    ) {
        unreachable!()
    }

    let method = Method {
        selector: selector.clone(),
        imp: dummy_method,
        types: RuntimeString::new("", get_global_arena()),
    };
    parent.add_method(method).unwrap();

    let child = Class::new(&child_name, &parent).unwrap();

    // Method should be provided by parent
    let provider = method_provider(&child, &selector);
    assert!(provider.is_some(), "Should find method provider");
    assert_eq!(provider.unwrap().name(), parent_name);
}

// ============================================================================
// Protocol Introspection Tests
// ============================================================================

#[test]
fn test_all_protocols_empty() {
    let protocols = all_protocols();
    // May or may not have protocols from other tests
    assert!(protocols.is_empty());
}

#[test]
fn test_adopted_protocols_empty() {
    let (class, _) = setup_test_class();

    let protocols = adopted_protocols(&class);
    assert_eq!(protocols.len(), 0, "New class should have no protocols");
}

#[test]
fn test_conforms_to_false() {
    let (class, _) = setup_test_class();
    let protocol = Protocol::new("TestProtocol", None).unwrap();

    assert!(!conforms_to(&class, &protocol));
}

#[test]
fn test_adopted_protocols_with_protocol() {
    let id = TEST_ID.fetch_add(1, Ordering::SeqCst);
    let class_name = format!("TestClass_{}", id);
    let protocol_name = format!("TestProtocol_{}", id);

    let class = Class::new_root(&class_name).unwrap();
    let protocol = Protocol::new(&protocol_name, None).unwrap();

    class.add_protocol(&protocol).unwrap();

    let protocols = adopted_protocols(&class);
    assert_eq!(protocols.len(), 1);
    assert_eq!(protocols[0].name(), protocol_name);
}

#[test]
fn test_conforms_to_with_protocol() {
    let id = TEST_ID.fetch_add(1, Ordering::SeqCst);
    let class_name = format!("TestClass_{}", id);
    let protocol_name = format!("TestProtocol_{}", id);

    let class = Class::new_root(&class_name).unwrap();
    let protocol = Protocol::new(&protocol_name, None).unwrap();

    class.add_protocol(&protocol).unwrap();

    assert!(conforms_to(&class, &protocol));
}

// ============================================================================
// Object Introspection Tests
// ============================================================================

#[test]
fn test_object_get_class() {
    let (class, object) = setup_test_class();

    let object_class = object_get_class(&object);
    assert_eq!(object_class.name(), class.name());
}

#[test]
fn test_object_is_instance_true() {
    let (class, object) = setup_test_class();

    assert!(object_is_instance(&object, &class));
}

#[test]
fn test_object_is_instance_false() {
    let (_class1, object) = setup_test_class();
    let (class2, _) = setup_test_class();

    assert!(!object_is_instance(&object, &class2));
}

#[test]
fn test_object_responds_to() {
    let (class, object) = setup_test_class();
    let selector = Selector::from_str("someMethod:").unwrap();

    // Class has no methods yet
    assert!(!object_responds_to(&object, &selector));

    // Add a method
    use oxidec::runtime::Method;
    use oxidec::runtime::RuntimeString;
    use oxidec::runtime::get_global_arena;
    use oxidec::runtime::object::ObjectPtr;
    use oxidec::runtime::selector::SelectorHandle;

    // Dummy function pointer for testing (never called)
    unsafe extern "C" fn dummy_method(
        _self: ObjectPtr,
        _cmd: SelectorHandle,
        _args: *const *mut u8,
        _ret: *mut u8,
    ) {
        unreachable!()
    }

    let method = Method {
        selector: selector.clone(),
        imp: dummy_method,
        types: RuntimeString::new("", get_global_arena()),
    };
    class.add_method(method).unwrap();

    // Now object should respond
    assert!(object_responds_to(&object, &selector));
}

// ============================================================================
// Dynamic Class Creation Tests
// ============================================================================

#[test]
fn test_class_builder_new() {
    let _builder = ClassBuilder::new("DynamicClass", None);
    // Should not panic
}

#[test]
fn test_class_builder_register() {
    let id = TEST_ID.fetch_add(1, Ordering::SeqCst);
    let class_name = format!("DynamicClass_{}", id);

    let builder = ClassBuilder::new(&class_name, None);
    let class = builder.register().unwrap();

    assert_eq!(class.name(), class_name);
}

#[test]
fn test_class_builder_with_superclass() {
    let id = TEST_ID.fetch_add(1, Ordering::SeqCst);
    let parent_name = format!("DynamicParent_{}", id);
    let child_name = format!("DynamicChild_{}", id);

    let parent = Class::new_root(&parent_name).unwrap();

    let builder = ClassBuilder::new(&child_name, Some(&parent));
    let child = builder.register().unwrap();

    assert_eq!(child.name(), child_name);
    assert!(is_subclass(&child, &parent));
}

#[test]
fn test_class_builder_add_protocol() {
    let id = TEST_ID.fetch_add(1, Ordering::SeqCst);
    let class_name = format!("DynamicClass_{}", id);
    let protocol_name = format!("DynamicProtocol_{}", id);

    let protocol = Protocol::new(&protocol_name, None).unwrap();

    let mut builder = ClassBuilder::new(&class_name, None);
    builder.add_protocol(&protocol);

    let class = builder.register().unwrap();

    assert!(conforms_to(&class, &protocol));
}

#[test]
fn test_allocate_class() {
    let id = TEST_ID.fetch_add(1, Ordering::SeqCst);
    let class_name = format!("AllocatedClass_{}", id);

    let builder = allocate_class(&class_name, None);
    let class = builder.register().unwrap();

    assert_eq!(class.name(), class_name);
}

#[test]
fn test_class_builder_duplicate_name() {
    let id = TEST_ID.fetch_add(1, Ordering::SeqCst);
    let class_name = format!("DuplicateClass_{}", id);

    let builder1 = ClassBuilder::new(&class_name, None);
    builder1.register().unwrap();

    let builder2 = ClassBuilder::new(&class_name, None);
    let result = builder2.register();

    assert!(result.is_err(), "Duplicate class name should fail");
}

// ============================================================================
// Hierarchy Edge Cases
// ============================================================================

#[test]
fn test_deep_hierarchy() {
    let id = TEST_ID.fetch_add(1, Ordering::SeqCst);

    // Create deep hierarchy: Level0 -> Level1 -> Level2 -> ... -> Level10
    let root = Class::new_root(&format!("Level0_{}", id)).unwrap();
    let mut current = root.clone();

    for i in 1..=10 {
        let next = Class::new(&format!("Level{}_{}", i, id), &current).unwrap();
        current = next;
    }

    let hierarchy = class_hierarchy(&current);
    assert_eq!(hierarchy.len(), 11, "Should have 11 levels");

    // Verify inheritance chain (hierarchy is child first, then parents)
    // hierarchy[0] = Level10, hierarchy[1] = Level9, ..., hierarchy[10] = Level0
    (0..=10).for_each(|i| {
        let level_num = 10 - i;
        assert!(hierarchy[i].name().contains(&format!("Level{}", level_num)));
    });
}

#[test]
fn test_multiple_inheritance_levels() {
    let id = TEST_ID.fetch_add(1, Ordering::SeqCst);

    let root = Class::new_root(&format!("Root_{}", id)).unwrap();
    let branch1 = Class::new(&format!("Branch1_{}", id), &root).unwrap();
    let branch2 = Class::new(&format!("Branch2_{}", id), &root).unwrap();

    let leaf1 = Class::new(&format!("Leaf1_{}", id), &branch1).unwrap();
    let leaf2 = Class::new(&format!("Leaf2_{}", id), &branch2).unwrap();

    // Check subclass relationships
    assert!(is_subclass(&leaf1, &branch1));
    assert!(is_subclass(&leaf1, &root));
    assert!(!is_subclass(&leaf1, &branch2));

    assert!(is_subclass(&leaf2, &branch2));
    assert!(is_subclass(&leaf2, &root));
    assert!(!is_subclass(&leaf2, &branch1));

    // Check subclasses
    let root_children = subclasses(&root);
    assert!(root_children.iter().any(|c| c.name() == branch1.name()));
    assert!(root_children.iter().any(|c| c.name() == branch2.name()));

    let branch1_children = subclasses(&branch1);
    assert!(branch1_children.iter().any(|c| c.name() == leaf1.name()));
}
