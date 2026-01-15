// Common test utilities for integration tests
//
// This module provides shared helper functions and test fixtures
// for use across all integration tests.

#![allow(dead_code)]

use oxidec::runtime::{
    Class, Method, RuntimeString, Selector, get_global_arena,
};
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Test method implementation that increments a counter
pub static TEST_METHOD_CALL_COUNT: AtomicUsize = AtomicUsize::new(0);

/// Creates a test class with the given name
pub fn create_test_class(name: &str) -> Class {
    Class::new_root(name).expect("Failed to create test class")
}

/// Creates a test selector from a string
pub fn create_test_selector(name: &str) -> Selector {
    Selector::from_str(name).expect("Failed to create test selector")
}

/// Creates a simple test method that returns void
pub fn create_test_method(
    selector: Selector,
    imp: oxidec::runtime::class::Imp,
) -> Method {
    let arena = get_global_arena();
    Method {
        selector,
        imp,
        types: RuntimeString::new("v@:", arena),
    }
}

/// Creates a test method that takes one integer argument
pub fn create_test_method_with_int_arg(
    selector: Selector,
    imp: oxidec::runtime::class::Imp,
) -> Method {
    let arena = get_global_arena();
    Method {
        selector,
        imp,
        types: RuntimeString::new("v@:i", arena),
    }
}

/// Creates a test method that returns an integer
pub fn create_test_method_returning_int(
    selector: Selector,
    imp: oxidec::runtime::class::Imp,
) -> Method {
    let arena = get_global_arena();
    Method {
        selector,
        imp,
        types: RuntimeString::new("i@:", arena),
    }
}

/// Simple test method implementation that does nothing
///
/// # Safety
///
/// This function is a valid method implementation and does not dereference
/// any raw pointers.
pub unsafe extern "C" fn void_method_impl(
    _self: oxidec::runtime::object::ObjectPtr,
    _cmd: oxidec::runtime::selector::SelectorHandle,
    _args: *const *mut u8,
    _ret: *mut u8,
) {
    // Intentionally empty - just a no-op method
}

/// Test method implementation that increments a counter
///
/// # Safety
///
/// This function is a valid method implementation and does not dereference
/// any raw pointers.
pub unsafe extern "C" fn counter_method_impl(
    _self: oxidec::runtime::object::ObjectPtr,
    _cmd: oxidec::runtime::selector::SelectorHandle,
    _args: *const *mut u8,
    _ret: *mut u8,
) {
    TEST_METHOD_CALL_COUNT.fetch_add(1, Ordering::SeqCst);
}

/// Test method implementation that returns a constant value (42)
///
/// # Safety
///
/// This function writes to the return pointer, which is guaranteed to be
/// valid and properly aligned for the return type.
pub unsafe extern "C" fn return_42_impl(
    _self: oxidec::runtime::object::ObjectPtr,
    _cmd: oxidec::runtime::selector::SelectorHandle,
    _args: *const *mut u8,
    ret: *mut u8,
) {
    // Write 42 to the return value location
    // SAFETY: ret points to valid memory for i32 return value
    // Use write_unaligned to handle potentially misaligned pointer
    unsafe {
        ret.cast::<i32>().write_unaligned(42);
    }
}

/// Test method implementation that returns a constant value (100)
///
/// # Safety
///
/// This function writes to the return pointer, which is guaranteed to be
/// valid and properly aligned for the return type.
pub unsafe extern "C" fn return_100_impl(
    _self: oxidec::runtime::object::ObjectPtr,
    _cmd: oxidec::runtime::selector::SelectorHandle,
    _args: *const *mut u8,
    ret: *mut u8,
) {
    // Write 100 to the return value location
    // SAFETY: ret points to valid memory for i32 return value
    // Use write_unaligned to handle potentially misaligned pointer
    unsafe {
        ret.cast::<i32>().write_unaligned(100);
    }
}

/// Resets the test method call counter to zero
pub fn reset_call_counter() {
    TEST_METHOD_CALL_COUNT.store(0, Ordering::SeqCst);
}

/// Gets the current test method call count
pub fn get_call_count() -> usize {
    TEST_METHOD_CALL_COUNT.load(Ordering::SeqCst)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_test_class() {
        let class = create_test_class("TestClass");
        assert_eq!(class.name(), "TestClass");
    }

    #[test]
    fn test_create_test_selector() {
        let sel = create_test_selector("testMethod");
        assert_eq!(sel.name(), "testMethod");
    }

    #[test]
    fn test_call_counter() {
        reset_call_counter();
        assert_eq!(get_call_count(), 0);

        TEST_METHOD_CALL_COUNT.fetch_add(1, Ordering::SeqCst);
        assert_eq!(get_call_count(), 1);

        reset_call_counter();
        assert_eq!(get_call_count(), 0);
    }
}
