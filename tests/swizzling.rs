// Method Swizzling Integration Tests
//
// These tests verify the method swizzling system works correctly
// in various scenarios.

mod common;

use oxidec::runtime::MessageArgs;
use oxidec::runtime::{Class, Object, Selector};
use std::str::FromStr;
use std::sync::atomic::{AtomicI32, Ordering};
use std::thread;

/// Test basic swizzle and restore
#[test]
#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::cast_possible_wrap)]
fn test_swizzle_basic() {
    // Create class with a method
    let class = Class::new_root("SwizzleBasic").unwrap();
    let sel = Selector::from_str("getValue").unwrap();
    let method = common::create_test_method_returning_int(
        sel.clone(),
        common::return_42_impl,
    );
    class.add_method(method).unwrap();

    let obj = Object::new(&class).unwrap();

    // Call original method (should return 42)
    let result = Object::send_message(&obj, &sel, &MessageArgs::None).unwrap();
    let original_value = result.unwrap() as i32;
    assert_eq!(original_value, 42, "Original method should return 42");

    // Swizzle to return 100 instead
    let original_imp =
        class.swizzle_method(&sel, common::return_100_impl).unwrap();

    // Call swizzled method (should now return 100)
    let result = Object::send_message(&obj, &sel, &MessageArgs::None).unwrap();
    let swizzled_value = result.unwrap() as i32;
    assert_eq!(swizzled_value, 100, "Swizzled method should return 100");

    // Restore original implementation
    class.swizzle_method(&sel, original_imp).unwrap();

    // Call restored method (should return 42 again)
    let result = Object::send_message(&obj, &sel, &MessageArgs::None).unwrap();
    let restored_value = result.unwrap() as i32;
    assert_eq!(restored_value, 42, "Restored method should return 42");
}

/// Test that swizzling a child class doesn't affect parent
#[test]
#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::cast_possible_wrap)]
fn test_swizzle_inheritance() {
    // Create parent class with a method
    let parent_class = Class::new_root("SwizzleParent").unwrap();
    let sel = Selector::from_str("inheritedMethod").unwrap();
    let method = common::create_test_method_returning_int(
        sel.clone(),
        common::return_42_impl,
    );
    parent_class.add_method(method).unwrap();

    let parent_obj = Object::new(&parent_class).unwrap();

    // Create child class
    let child_class = Class::new("SwizzleChild", &parent_class).unwrap();
    let child_obj = Object::new(&child_class).unwrap();

    // Verify both return 42 initially
    let parent_result =
        Object::send_message(&parent_obj, &sel, &MessageArgs::None).unwrap();
    assert_eq!(parent_result.unwrap() as i32, 42);

    let child_result =
        Object::send_message(&child_obj, &sel, &MessageArgs::None).unwrap();
    assert_eq!(child_result.unwrap() as i32, 42);

    // Swizzle child class method
    child_class
        .add_method(common::create_test_method_returning_int(
            sel.clone(),
            common::return_42_impl,
        ))
        .unwrap();
    let _original = child_class
        .swizzle_method(&sel, common::return_100_impl)
        .unwrap();

    // Child should now return 100
    let child_result =
        Object::send_message(&child_obj, &sel, &MessageArgs::None).unwrap();
    assert_eq!(
        child_result.unwrap() as i32,
        100,
        "Child should return 100 after swizzle"
    );

    // Parent should still return 42 (unaffected)
    let parent_result =
        Object::send_message(&parent_obj, &sel, &MessageArgs::None).unwrap();
    assert_eq!(
        parent_result.unwrap() as i32,
        42,
        "Parent should still return 42"
    );
}

/// Test that cache is invalidated after swizzling
#[test]
#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::cast_possible_wrap)]
fn test_swizzle_cache_invalidation() {
    // Create class with method
    let class = Class::new_root("CacheInvalidation").unwrap();
    let sel = Selector::from_str("cachedMethod").unwrap();
    let method = common::create_test_method_returning_int(
        sel.clone(),
        common::return_42_impl,
    );
    class.add_method(method).unwrap();

    let obj = Object::new(&class).unwrap();

    // Call method multiple times to populate cache
    for _ in 0..3 {
        let result =
            Object::send_message(&obj, &sel, &MessageArgs::None).unwrap();
        assert_eq!(result.unwrap() as i32, 42);
    }

    // Swizzle method
    let _original =
        class.swizzle_method(&sel, common::return_100_impl).unwrap();

    // Cache should be invalidated, so new implementation should be called
    let result = Object::send_message(&obj, &sel, &MessageArgs::None).unwrap();
    assert_eq!(
        result.unwrap() as i32,
        100,
        "Cache should be invalidated after swizzle"
    );

    // Restore
    class.swizzle_method(&sel, common::return_42_impl).unwrap();
}

/// Test thread safety of concurrent swizzles
#[test]
fn test_swizzle_thread_safety() {
    static SWIZZLE_COUNT: AtomicI32 = AtomicI32::new(0);

    // Create class with method
    let class = Class::new_root("ThreadSafeSwizzle").unwrap();
    let sel = Selector::from_str("threadSafeMethod").unwrap();
    let method = common::create_test_method_returning_int(
        sel.clone(),
        common::return_42_impl,
    );
    class.add_method(method).unwrap();

    let obj = Object::new(&class).unwrap();

    // Spawn multiple threads that swizzle the method
    let handles: Vec<_> = (0..5)
        .map(|_i| {
            let class_clone = class.clone();
            let sel_clone = sel.clone();
            thread::spawn(move || {
                // Swizzle back and forth
                let _ = class_clone
                    .swizzle_method(&sel_clone, common::return_100_impl);
                SWIZZLE_COUNT.fetch_add(1, Ordering::SeqCst);
                let _ = class_clone
                    .swizzle_method(&sel_clone, common::return_42_impl);
            })
        })
        .collect();

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    // All swizzles should have completed without deadlocking
    assert_eq!(SWIZZLE_COUNT.load(Ordering::SeqCst), 5);

    // Method should still work
    let result = Object::send_message(&obj, &sel, &MessageArgs::None);
    assert!(result.is_ok());
}

/// Test runtime patching scenario (hotfix)
#[test]
#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::cast_possible_wrap)]
fn test_swizzle_runtime_patching() {
    // Buggy implementation returns -1 (error)
    unsafe extern "C" fn buggy_impl(
        _self: oxidec::runtime::object::ObjectPtr,
        _cmd: oxidec::runtime::selector::SelectorHandle,
        _args: *const *mut u8,
        ret: *mut u8,
    ) {
        unsafe {
            ret.cast::<i32>().write_unaligned(-1); // Bug: returns error code
        }
    }

    // Fixed implementation returns 0 (success)
    unsafe extern "C" fn fixed_impl(
        _self: oxidec::runtime::object::ObjectPtr,
        _cmd: oxidec::runtime::selector::SelectorHandle,
        _args: *const *mut u8,
        ret: *mut u8,
    ) {
        unsafe {
            ret.cast::<i32>().write_unaligned(0); // Fixed: returns success
        }
    }

    // Simulate a class with a buggy method
    let class = Class::new_root("BuggyClass").unwrap();
    let sel = Selector::from_str("buggyMethod").unwrap();

    let method =
        common::create_test_method_returning_int(sel.clone(), buggy_impl);
    class.add_method(method).unwrap();

    let obj = Object::new(&class).unwrap();

    // Verify buggy behavior
    let result = Object::send_message(&obj, &sel, &MessageArgs::None).unwrap();
    assert_eq!(result.unwrap() as i32, -1, "Buggy method returns error");

    // Apply hotfix: swizzle with correct implementation
    let _buggy_impl = class.swizzle_method(&sel, fixed_impl).unwrap();

    // Verify fixed behavior
    let result = Object::send_message(&obj, &sel, &MessageArgs::None).unwrap();
    assert_eq!(result.unwrap() as i32, 0, "Hotfixed method returns success");

    // Can keep buggy impl for rollback if needed
}

/// Test debugging injection via swizzling
#[test]
#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::cast_possible_wrap)]
fn test_swizzle_debugging_injection() {
    // Original implementation
    unsafe extern "C" fn original_impl(
        _self: oxidec::runtime::object::ObjectPtr,
        _cmd: oxidec::runtime::selector::SelectorHandle,
        _args: *const *mut u8,
        ret: *mut u8,
    ) {
        unsafe {
            ret.cast::<i32>().write_unaligned(42);
        }
    }

    static DEBUG_CALL_COUNT: AtomicI32 = AtomicI32::new(0);

    // Debug wrapper that logs calls
    unsafe extern "C" fn debug_wrapper(
        _self: oxidec::runtime::object::ObjectPtr,
        _cmd: oxidec::runtime::selector::SelectorHandle,
        _args: *const *mut u8,
        ret: *mut u8,
    ) {
        DEBUG_CALL_COUNT.fetch_add(1, Ordering::SeqCst);
        // Call original
        unsafe {
            ret.cast::<i32>().write_unaligned(42);
        }
    }

    // Create class with method
    let class = Class::new_root("DebugClass").unwrap();
    let sel = Selector::from_str("debugMethod").unwrap();
    let method =
        common::create_test_method_returning_int(sel.clone(), original_impl);
    class.add_method(method).unwrap();

    let obj = Object::new(&class).unwrap();

    // Call without debug wrapper
    let result = Object::send_message(&obj, &sel, &MessageArgs::None).unwrap();
    assert_eq!(result.unwrap() as i32, 42);
    assert_eq!(DEBUG_CALL_COUNT.load(Ordering::SeqCst), 0);

    // Inject debug wrapper
    let _original = class.swizzle_method(&sel, debug_wrapper).unwrap();

    // Call with debug wrapper
    let result = Object::send_message(&obj, &sel, &MessageArgs::None).unwrap();
    assert_eq!(result.unwrap() as i32, 42);
    assert_eq!(
        DEBUG_CALL_COUNT.load(Ordering::SeqCst),
        1,
        "Debug wrapper should track calls"
    );

    // Remove debug wrapper (restore original)
    class.swizzle_method(&sel, original_impl).unwrap();

    DEBUG_CALL_COUNT.store(0, Ordering::SeqCst);
}
