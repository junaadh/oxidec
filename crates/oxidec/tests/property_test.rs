//! Property-based tests for OxideC runtime components.
//!
//! These tests validate crash resistance and robustness with arbitrary inputs,
//! providing fuzzing-like testing without requiring cargo-fuzz infrastructure.
//!
//! Run with: `cargo test --test property_test -- --test-threads=1`

use oxidec::runtime::{
    Class, Invocation, MessageArgs, Object, PooledInvocation, RuntimeString,
    Selector,
};
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};

static PROP_ID: AtomicUsize = AtomicUsize::new(0);

fn setup_prop_class() -> (Class, Object) {
    let id = PROP_ID.fetch_add(1, Ordering::SeqCst);
    let class_name = format!("PropTest_{id}");
    let class = Class::new_root(&class_name).unwrap();
    let object = Object::new(&class).unwrap();
    (class, object)
}

// ============================================================================
// Selector Property Tests
// ============================================================================

#[test]
fn test_selector_from_arbitrary_strings() {
    let test_strings = vec![
        "",                        // empty
        "normalMethod",            // normal
        "method:with:colons:",     // multiple colons
        "methodWithArg:",          // single arg
        "method:with:many:args:",  // many args
        "methodWith123Numbers",    // numbers
        "method_with_underscores", // underscores
        "method-WithDashes",       // dashes (invalid but should not crash)
        "方法",                    // unicode
        "method🚀rocket",          // emoji
        "a",                       // single char
        "a:b:c:d:e:f:g:h:i:j",     // many colons
    ];

    for s in test_strings {
        // Should not crash on any input
        let result = Selector::from_str(s);
        // We don't care if it succeeds or fails, just that it doesn't crash
        drop(result);
    }
}

#[test]
fn test_selector_with_invalid_utf8() {
    // Test that we handle invalid UTF-8 gracefully
    let invalid_bytes = vec![
        vec![0xFF, 0xFE],       // invalid UTF-8
        vec![0xC0, 0x80],       // overlong encoding
        vec![0xED, 0xA0, 0x80], // surrogate
    ];

    for bytes in invalid_bytes {
        // Try to create string from bytes (may fail, should not crash)
        let result = std::str::from_utf8(&bytes);
        if let Ok(s) = result {
            let _ = Selector::from_str(s);
        }
    }
}

// ============================================================================
// Message Arguments Property Tests
// ============================================================================

#[test]
fn test_message_args_with_various_counts() {
    let (_class, _target) = setup_prop_class();
    let _selector = Selector::from_str("testMethod:").unwrap();

    // Test creating MessageArgs with different counts
    for count in 1..=2 {
        let args = match count {
            1 => MessageArgs::one(42),
            2 => MessageArgs::two(42, 99),
            _ => continue,
        };

        // Should not crash
        #[allow(clippy::drop_non_drop)]
        drop(args);
    }
}

#[test]
fn test_message_args_with_various_types() {
    // Test with various argument types (MessageArgs only accepts usize)
    let _args1 = MessageArgs::two(42usize, 99usize);
    let _args2 = MessageArgs::two(0usize, usize::MAX);
    let _args3 = MessageArgs::two(123usize, 456usize);
    let _args4 = MessageArgs::two(1usize, 2usize);
}

#[test]
fn test_invocation_with_various_targets() {
    let (_class1, obj1) = setup_prop_class();
    let (_class2, obj2) = setup_prop_class();
    let selector = Selector::from_str("testMethod:").unwrap();

    // Test invocation creation with different targets
    let inv1 = Invocation::new(&obj1, &selector);
    let inv2 = Invocation::new(&obj2, &selector);

    assert!(inv1.is_ok());
    assert!(inv2.is_ok());

    // Should not crash
    drop(inv1);
    drop(inv2);
}

// ============================================================================
// Class Creation Property Tests
// ============================================================================

#[test]
fn test_class_with_various_names() {
    let class_names = vec![
        "NormalClass",
        "Class123",
        "类",
        "My_Class",
        "My-Class",
        "VeryLongClassName0123456789012345678901234567890123456789",
        "a",
        "A",
    ];

    for name in class_names {
        // Should not crash on any class name
        let id = PROP_ID.fetch_add(1, Ordering::SeqCst);
        let unique_name = format!("{}_{}", name, id);

        let result = Class::new_root(&unique_name);
        if let Ok(class) = result {
            // Should be able to create object
            let obj = Object::new(&class);
            assert!(obj.is_ok());
        }
        // We don't care if it fails, just that it doesn't crash
    }
}

#[test]
fn test_class_creation_reuse() {
    // Try to create class with same name multiple times
    let id = PROP_ID.fetch_add(1, Ordering::SeqCst);
    let class_name = format!("TestReuse_{}", id);

    let class1 = Class::new_root(&class_name);
    let class2 = Class::new_root(&class_name);

    // Second should fail (already exists)
    assert!(class1.is_ok());
    assert!(class2.is_err());

    // Should not crash
}

// ============================================================================
// Object Lifecycle Property Tests
// ============================================================================

#[test]
fn test_object_rapid_creation_destruction() {
    let (class, _) = setup_prop_class();

    // Create and destroy many objects rapidly
    for _ in 0..1000 {
        let obj = Object::new(&class).unwrap();
        drop(obj);
    }

    // Should not crash
}

#[test]
fn test_object_clone_many() {
    let (class, _) = setup_prop_class();
    let obj = Object::new(&class).unwrap();

    // Clone object many times
    let clones: Vec<_> = (0..100).map(|_| obj.clone()).collect();

    assert_eq!(clones.len(), 100);

    // Drop all clones
    drop(clones);

    // Should not crash
}

#[test]
fn test_object_concurrent_access() {
    use std::sync::Arc;
    use std::thread;

    let (class, _) = setup_prop_class();
    let obj = Arc::new(Object::new(&class).unwrap());

    let handles: Vec<_> = (0..10)
        .map(|_| {
            let obj = Arc::clone(&obj);
            thread::spawn(move || {
                for _ in 0..100 {
                    let _ = obj.class();
                    let _ = obj.clone();
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    // Should not crash
}

// ============================================================================
// Pool Property Tests
// ============================================================================

#[test]
fn test_pool_with_various_patterns() {
    let (_class, target) = setup_prop_class();
    let selector = Selector::from_str("testMethod:").unwrap();
    let args = MessageArgs::two(42, 99);

    PooledInvocation::clear_pool();

    // Test various acquisition/return patterns
    // Pattern 1: Sequential
    for _ in 0..10 {
        let _ = PooledInvocation::with_arguments(&target, &selector, &args);
    }

    // Pattern 2: Batch acquire, batch release
    let invocations: Vec<_> = (0..10)
        .map(|_| {
            PooledInvocation::with_arguments(&target, &selector, &args).unwrap()
        })
        .collect();
    drop(invocations);

    // Pattern 3: Interleaved
    for i in 0..10 {
        if i % 2 == 0 {
            let inv =
                PooledInvocation::with_arguments(&target, &selector, &args)
                    .unwrap();
            drop(inv);
        } else {
            let inv1 =
                PooledInvocation::with_arguments(&target, &selector, &args)
                    .unwrap();
            let inv2 =
                PooledInvocation::with_arguments(&target, &selector, &args)
                    .unwrap();
            drop(inv1);
            drop(inv2);
        }
    }

    // Should not crash
}

#[test]
fn test_pool_exhaustion_recovery() {
    let (_class, target) = setup_prop_class();
    let selector = Selector::from_str("testMethod:").unwrap();
    let args = MessageArgs::two(42, 99);

    PooledInvocation::clear_pool();

    // Exhaust the pool
    let invocations: Vec<_> = (0..1000)
        .map(|_| {
            PooledInvocation::with_arguments(&target, &selector, &args).unwrap()
        })
        .collect();

    // Pool should still work after exhaustion
    drop(invocations);
    let inv = PooledInvocation::with_arguments(&target, &selector, &args);
    assert!(inv.is_ok());

    // Should not crash
}

// ============================================================================
// Runtime String Property Tests
// ============================================================================

#[test]
fn test_runtime_string_with_various_inputs() {
    let test_strings: Vec<String> = vec![
        "".to_string(),
        "a".to_string(),
        "abc".to_string(),
        "hello world".to_string(),
        "Lorem ipsum dolor sit amet".to_string(),
        "🚀🎉✨".to_string(),
        "a".repeat(10),  // inline
        "a".repeat(100), // heap
        "类".to_string(),
        "\n\t\r".to_string(),
        "\0\x01\x02".to_string(),
    ];

    for s in test_strings {
        // Should not crash
        let rs = RuntimeString::new(&s, oxidec::runtime::get_global_arena());
        drop(rs);
    }
}

#[test]
fn test_runtime_string_interning_stress() {
    let arena = oxidec::runtime::get_global_arena();

    // Create same string many times (should intern)
    for _ in 0..1000 {
        let rs1 = RuntimeString::new("test", arena);
        let rs2 = RuntimeString::new("test", arena);
        // Should be interned (same pointer)
        // We don't assert this, just verify no crash
        drop(rs1);
        drop(rs2);
    }

    // Should not crash
}

// ============================================================================
// Edge Case Property Tests
// ============================================================================

#[test]
fn test_zero_size_selector() {
    // Empty selector should be handled
    let result = Selector::from_str("");
    // May fail, but shouldn't crash
    drop(result);
}

#[test]
fn test_very_long_selector() {
    let long_selector = "a".repeat(10000);
    let result = Selector::from_str(&long_selector);
    // May fail, but shouldn't crash
    drop(result);
}

#[test]
fn test_selector_with_special_characters() {
    let special_selectors = vec![
        "test:method:",
        "test\nmethod",
        "test\tmethod",
        "test\x00method",
        "test!@#$%^&*()method",
    ];

    for s in special_selectors {
        let result = Selector::from_str(s);
        // May fail, but shouldn't crash
        drop(result);
    }
}

#[test]
fn test_concurrent_class_creation() {
    use std::thread;

    let handles: Vec<_> = (0..10)
        .map(|i| {
            thread::spawn(move || {
                let id = PROP_ID.fetch_add(1, Ordering::SeqCst);
                let class_name = format!("ConcurrentClass_{}_{}", i, id);

                let class = Class::new_root(&class_name);
                if let Ok(class) = class {
                    let obj = Object::new(&class);
                    obj.is_ok()
                } else {
                    false
                }
            })
        })
        .collect();

    let results: Vec<bool> =
        handles.into_iter().map(|h| h.join().unwrap()).collect();

    // Most should succeed (no name collisions due to unique IDs)
    let success_count = results.iter().filter(|&&r| r).count();
    assert!(
        success_count >= 8,
        "Expected at least 8 successes, got {}",
        success_count
    );

    // Should not crash
}

#[test]
fn test_many_classes() {
    // Create many different classes
    for i in 0..1000 {
        let id = PROP_ID.fetch_add(1, Ordering::SeqCst);
        let class_name = format!("ManyClasses_{}_{}", i, id);

        let class = Class::new_root(&class_name);
        if let Ok(class) = class {
            let obj = Object::new(&class);
            assert!(obj.is_ok());
        }
    }

    // Should not crash or leak
}

#[test]
fn test_deep_inheritance_chain() {
    // Create deep inheritance chain
    let mut current_class = Some(Class::new_root("DeepRoot").unwrap());

    for i in 0..100 {
        let id = PROP_ID.fetch_add(1, Ordering::SeqCst);
        let class_name = format!("DeepChild_{}_{}", i, id);

        if let Some(ref superclass) = current_class {
            let new_class = Class::new(&class_name, superclass);
            if let Ok(new_class) = new_class {
                current_class = Some(new_class);
            }
        }
    }

    // Should not crash
}

// ============================================================================
// Memory Stress Tests
// ============================================================================

#[test]
fn test_memory_allocation_stress() {
    let (class, _) = setup_prop_class();

    // Allocate many objects
    let objects: Vec<_> =
        (0..10000).map(|_| Object::new(&class).unwrap()).collect();

    assert_eq!(objects.len(), 10000);

    // Drop all
    drop(objects);

    // Should not crash or leak
}

#[test]
fn test_selector_intern_stress() {
    // Create many selectors (should intern duplicates)
    let base_names = vec!["foo", "bar", "baz:", "qux:arg:", "test"];

    for _ in 0..1000 {
        for name in &base_names {
            let _ = Selector::from_str(name);
        }
    }

    // Should not crash
}
