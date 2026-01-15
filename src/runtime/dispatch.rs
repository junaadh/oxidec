//! Message dispatch system for ``OxideC`` runtime.
//!
//! This module implements the core message dispatch mechanism, equivalent to
//! `Object`ive-C's `objc_msgSend`. It provides:

// Allow pointer alignment casting - we ensure proper alignment in dispatch code
#![allow(clippy::cast_ptr_alignment)]
//!
//! - **Dynamic method lookup**: Searches class and inheritance chain
//! - **`Method` caching**: O(1) lookup after first call via per-class caches
//! - **Argument marshalling**: Packs arguments for C function pointer calls
//! - **Return value handling**: Extracts return values based on type encoding
//!
//! # Dispatch Algorithm
//!
//! 1. Get object's class (isa pointer)
//! 2. Check method cache (fast path)
//! 3. If cache miss, walk inheritance chain
//! 4. Cache the found method
//! 5. Invoke the implementation
//! 6. Return result or error
//!
//! # Thread Safety
//!
//! All dispatch operations are thread-safe:
//! - `Object` reference counting ensures object lifetime during dispatch
//! - Method cache is protected by `RwLock`
//! - Function pointer calls have no shared mutable state
//!
//! # Example
//!
//! ```rust
//! use oxidec::runtime::{Object, Class, Selector, MessageArgs};
//! use oxidec::runtime::dispatch;
//! use std::str::FromStr;
//!
//! // Create object and selector
//! let class = Class::new_root("MyClass").unwrap();
//! let obj = Object::new(&class).unwrap();
//! let sel = Selector::from_str("doSomething").unwrap();
//!
//! // Send message with no arguments (returns Result with encoded return value)
//! match unsafe { dispatch::send_message(&obj, &sel, &MessageArgs::None) } {
//!     Ok(retval) => println!("Return value: {:?}", retval),
//!     Err(e) => println!("Error: {:?}", e),
//! }
//!
//! // Send message with one argument
//! match unsafe { dispatch::send_message(&obj, &sel, &MessageArgs::one(42)) } {
//!     Ok(retval) => println!("Return value: {:?}", retval),
//!     Err(e) => println!("Error: {:?}", e),
//! }
//!
//! // Send message with two arguments
//! match unsafe { dispatch::send_message(&obj, &sel, &MessageArgs::two(10, 20)) } {
//!     Ok(retval) => println!("Return value: {:?}", retval),
//!     Err(e) => println!("Error: {:?}", e),
//! }
//! ```

use crate::error::{Error, Result};
use crate::runtime::MessageArgs;
use crate::runtime::Object;
use crate::runtime::Selector;

/// Sends a message to an object with the given arguments.
///
/// This is the core message dispatch function, equivalent to Objective-C's
/// `objc_msgSend`. It performs dynamic method lookup and invocation.
///
/// # Arguments
///
/// * `obj` - The receiver object (must be valid)
/// * `selector` - The message selector
/// * `args` - Arguments to pass to the method (using `MessageArgs` enum)
///
/// # Returns
///
/// - `Ok(Some(retval))` - Method returned a value (encoded as usize)
/// - `Ok(None)` - Method returned void
/// - `Err(Error::SelectorNotFound)` - Method not found in class or inheritance chain
/// - `Err(Error::ArgumentCountMismatch)` - Wrong number of arguments
///
/// # Safety
///
/// Caller must ensure:
/// - `obj` is a valid pointer to an `Object`
/// - The `Object`'s reference count > 0 (object is alive)
/// - Arguments in `args` are correctly encoded for the expected parameter types
///
/// # Thread Safety
///
/// This function is thread-safe. Multiple threads can send messages concurrently.
///
/// # Performance
///
/// - Cache hit: ~50ns (`HashMap` lookup + indirect call)
/// - Cache miss: ~150ns (inheritance walk + cache update)
///
/// # Example
///
/// ```rust,no_run
/// use oxidec::runtime::{Object, Selector, MessageArgs};
/// use oxidec::runtime::dispatch;
///
/// # let obj: Object = unsafe { std::mem::zeroed() };
/// # let sel: Selector = unsafe { std::mem::zeroed() };
/// unsafe {
///     // No arguments
///     match dispatch::send_message(&obj, &sel, &MessageArgs::None) {
///         Ok(retval) => println!("Success: {:?}", retval),
///         Err(e) => println!("Error: {:?}", e),
///     }
///
///     // One argument
///     match dispatch::send_message(&obj, &sel, &MessageArgs::one(42)) {
///         Ok(retval) => println!("Success: {:?}", retval),
///         Err(e) => println!("Error: {:?}", e),
///     }
///
///     // Two arguments
///     match dispatch::send_message(&obj, &sel, &MessageArgs::two(10, 20)) {
///         Ok(retval) => println!("Success: {:?}", retval),
///         Err(e) => println!("Error: {:?}", e),
///     }
/// }
/// ```
///
/// # Errors
///
/// Returns [`Error::SelectorNotFound`] if the selector is not found in the
/// class's inheritance chain, or [`Error::ArgumentCountMismatch`] if the
/// number of arguments provided doesn't match the method's signature.
///
/// # Panics
///
/// Panics if the method lookup fails (which should never happen if
/// `SelectorNotFound` was not returned).
///
/// Helper function to call a method with arguments.
///
/// This extracts the common method calling logic to avoid duplication
/// between normal dispatch and forwarded dispatch.
///
/// # Returns
///
/// * `Some(value)` - Method returned a value
/// * `None` - Method returned void
unsafe fn call_method_with_args(
    obj: &Object,
    imp: crate::runtime::class::Imp,
    selector: &Selector,
    args: &MessageArgs,
) -> Option<usize> {
    // Pack arguments based on MessageArgs variant
    let arg_slice = args.as_slice();
    let args_ptr: *const *mut u8 = if arg_slice.is_empty() {
        [].as_ptr()
    } else {
        // Convert &[usize] to [*const u8] for C ABI
        // SAFETY: We're transmuting usize pointers to u8 pointers, which is safe
        // since we're only changing the type, not the representation
        arg_slice.as_ptr().cast::<*mut u8>()
    };

    // Prepare return value storage
    let mut ret_value: [u8; 16] = [0; 16]; // Max size for common return types
    let ret_ptr = ret_value.as_mut_ptr();

    // Get raw object pointer (for _self parameter)
    // SAFETY: obj is a valid reference, and we need the raw pointer for the C ABI
    let self_ptr = obj.as_raw();

    // Call the method implementation
    // SAFETY:
    // - imp is a valid function pointer (from lookup_imp)
    // - self_ptr points to a valid object (guaranteed by caller)
    // - selector is valid (checked by lookup_imp)
    // - args_ptr points to valid arguments (if any)
    // - ret_ptr points to writable memory (16 bytes, stack-allocated)
    unsafe {
        imp(self_ptr, selector.as_handle(), args_ptr, ret_ptr);
    }

    // Get method encoding for return value extraction
    let class = obj.class();
    let method = class.lookup_method(selector).unwrap();
    let encoding = method.types.as_str().unwrap();

    // Extract return value based on method encoding
    let return_type = encoding.chars().next().unwrap();
    if return_type == 'v' {
        None // Void return
    } else {
        // Non-void return: read the value written by the method implementation
        // SAFETY: ret_ptr points to valid memory where the IMP wrote the return value.
        // We use read_unaligned to handle potentially misaligned pointers.
        // The IMP function is responsible for writing the correct type.
        let value =
            unsafe { std::ptr::read_unaligned(ret_ptr as *const usize) };
        Some(value)
    }
}

/// Sends a message to an object with the given selector and arguments.
///
/// This is the core message dispatch function. It looks up the method
/// implementation for the given selector in the object's class hierarchy
/// (including categories), caches the result, and invokes the method.
///
/// # Arguments
///
/// * `obj` - The object to send the message to
/// * `selector` - The method selector to look up
/// * `args` - The arguments to pass to the method
///
/// # Returns
///
/// * `Ok(Some(value))` - The method returned a value
/// * `Ok(None)` - The method returned void
/// * `Err(Error::SelectorNotFound)` - The selector was not found
/// * `Err(Error::ArgumentCountMismatch)` - Argument count doesn't match signature
/// * `Err(Error::ForwardingFailed)` - Message forwarding failed
/// * `Err(Error::ForwardingLoopDetected)` - Forwarding loop detected
///
/// # Panics
///
/// Panics if the method lookup cache lock is poisoned (indicates a concurrent
/// access error or panic in another thread).
///
/// # Errors
///
/// This function returns `Err` if:
/// - The selector is not found in the class hierarchy
/// - The argument count doesn't match the method signature
/// - Message forwarding fails (target also doesn't recognize selector)
/// - A forwarding loop is detected (exceeds max depth)
///
/// # Safety
///
/// This function is unsafe because it calls arbitrary function pointers
/// (method implementations) that must conform to the C ABI calling convention.
pub unsafe fn send_message(
    obj: &Object,
    selector: &Selector,
    args: &MessageArgs,
) -> Result<Option<usize>> {
    // Get object's class
    let class = obj.class();

    // Lookup method implementation (with caching)
    let Some(imp) = class.lookup_imp(selector) else {
        // Method not found - try forwarding

        use crate::runtime::forwarding;

        // Check cache first (performance optimization)
        if let Some(cached_target) =
            forwarding::get_cached_target(obj, selector)
        {
            forwarding::emit_forwarding_event(
                forwarding::ForwardingEvent::ForwardingSuccess {
                    object: obj.clone(),
                    selector: selector.clone(),
                    target: cached_target.clone(),
                },
            );

            let target_class = cached_target.class();
            if let Some(imp) = target_class.lookup_imp(selector) {
                // Validate arguments for cached target
                let method = target_class.lookup_method(selector).unwrap();
                let encoding = method.types.as_str().unwrap();
                let (_ret_type, arg_types) =
                    crate::runtime::encoding::parse_signature(encoding)?;
                let expected_args = arg_types.len() - 2;
                let actual_args = args.count();

                if actual_args != expected_args {
                    return Err(Error::ArgumentCountMismatch {
                        expected: arg_types.len(),
                        got: actual_args + 2,
                    });
                }

                // Call on cached target
                return unsafe {
                    Ok(call_method_with_args(
                        &cached_target,
                        imp,
                        selector,
                        args,
                    ))
                };
            }
            // Cache miss - fall through to full forwarding resolution
        }

        // Full forwarding resolution
        match forwarding::resolve_forwarding(obj, selector) {
            forwarding::ForwardingResult::Target(target) => {
                // Cache for next time
                forwarding::cache_forwarded_target(obj, selector, &target);

                // Retry dispatch on target
                let target_class = target.class();
                let target_imp = target_class.lookup_imp(selector).ok_or(
                    Error::ForwardingFailed {
                        selector: selector.name().to_string(),
                        reason: "Target also doesn't recognize selector"
                            .to_string(),
                    },
                )?;

                // Validate arguments for target
                let method = target_class.lookup_method(selector).unwrap();
                let encoding = method.types.as_str().unwrap();
                let (_ret_type, arg_types) =
                    crate::runtime::encoding::parse_signature(encoding)?;
                let expected_args = arg_types.len() - 2;
                let actual_args = args.count();

                if actual_args != expected_args {
                    return Err(Error::ArgumentCountMismatch {
                        expected: arg_types.len(),
                        got: actual_args + 2,
                    });
                }

                return unsafe {
                    Ok(call_method_with_args(
                        &target, target_imp, selector, args,
                    ))
                };
            }
            forwarding::ForwardingResult::NotFound => {
                // Check if object implements doesNotRecognizeSelector:
                use std::str::FromStr;
                let dnr_sel = Selector::from_str("doesNotRecognizeSelector:");
                if dnr_sel.is_ok()
                    && class.lookup_imp(&dnr_sel.unwrap()).is_some()
                {
                    forwarding::emit_forwarding_event(
                        forwarding::ForwardingEvent::DoesNotRecognize {
                            object: obj.clone(),
                            selector: selector.clone(),
                        },
                    );
                    // Note: Full doesNotRecognizeSelector: invocation with selector argument
                    // would require packing the selector as a message argument. For now,
                    // we just emit the event and fall through to SelectorNotFound.
                }

                return Err(Error::SelectorNotFound);
            }
            forwarding::ForwardingResult::LoopDetected => {
                return Err(Error::ForwardingLoopDetected {
                    selector: selector.name().to_string(),
                    depth: forwarding::FORWARDING_DEPTH
                        .with(std::cell::Cell::get),
                });
            }
        }
    };

    // Validate argument count
    let method = class.lookup_method(selector).unwrap();
    let encoding = method.types.as_str().unwrap();
    let (_ret_type, arg_types) =
        crate::runtime::encoding::parse_signature(encoding)?;

    // arg_types includes self (@) and _cmd (:), so actual args = len - 2
    let expected_args = arg_types.len() - 2;
    let actual_args = args.count();

    if actual_args != expected_args {
        return Err(Error::ArgumentCountMismatch {
            expected: arg_types.len(),
            got: actual_args + 2,
        });
    }

    // Call the method using the helper
    unsafe { Ok(call_method_with_args(obj, imp, selector, args)) }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;
    use crate::runtime::Class;
    use crate::runtime::get_global_arena;
    use crate::runtime::selector::SelectorHandle;

    /// Test helper: no-op method implementation
    unsafe extern "C" fn test_noop_impl(
        _self: crate::runtime::object::ObjectPtr,
        _cmd: SelectorHandle,
        _args: *const *mut u8,
        _ret: *mut u8,
    ) {
        // No-op
    }

    /// Test helper: method that returns a value
    unsafe extern "C" fn test_return_42_impl(
        _self: crate::runtime::object::ObjectPtr,
        _cmd: SelectorHandle,
        _args: *const *mut u8,
        ret: *mut u8,
    ) {
        // Return 42 as usize
        // SAFETY: We use write_unaligned to handle potentially misaligned pointers.
        // The caller provides the return value buffer, and we write 42 to it.
        unsafe {
            std::ptr::write_unaligned(ret.cast::<usize>(), 42);
        };
    }

    #[test]
    fn test_send_message_0_basic() {
        let class = Class::new_root("DispatchTest0").unwrap();
        let sel = Selector::from_str("test`Method`").unwrap();
        let arena = get_global_arena();

        // Add a test method
        let imp = test_noop_impl;
        let method = crate::runtime::class::Method {
            selector: sel.clone(),
            imp,
            types: crate::runtime::RuntimeString::new("v@:", arena),
        };
        class.add_method(method).unwrap();

        // Create object and send message
        let obj = Object::new(&class).unwrap();
        let result = unsafe { send_message(&obj, &sel, &MessageArgs::None) };

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), None); // Void return
    }

    #[test]
    fn test_send_message_selector_not_found() {
        let class = Class::new_root("DispatchNotFound").unwrap();
        let sel = Selector::from_str("nonExistent`Method`").unwrap();
        let obj = Object::new(&class).unwrap();

        let result = unsafe { send_message(&obj, &sel, &MessageArgs::None) };
        assert!(matches!(result, Err(Error::SelectorNotFound)));
    }

    #[test]
    fn test_send_message_with_return_value() {
        let class = Class::new_root("DispatchReturn").unwrap();
        let sel = Selector::from_str("getValue").unwrap();
        let arena = get_global_arena();

        // Add a method that returns 42
        let imp = test_return_42_impl;
        let method = crate::runtime::class::Method {
            selector: sel.clone(),
            imp,
            types: crate::runtime::RuntimeString::new("q@:", arena), // q = long long (usize)
        };
        class.add_method(method).unwrap();

        let obj = Object::new(&class).unwrap();
        let result = unsafe { send_message(&obj, &sel, &MessageArgs::None) };

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Some(42));
    }

    #[test]
    fn test_send_message_1_basic() {
        let class = Class::new_root("DispatchTest1").unwrap();
        let sel = Selector::from_str("methodWithArg:").unwrap();
        let arena = get_global_arena();

        // Add a method taking one argument
        let imp = test_noop_impl;
        let method = crate::runtime::class::Method {
            selector: sel.clone(),
            imp,
            types: crate::runtime::RuntimeString::new("v@:i", arena), // void return, int arg
        };
        class.add_method(method).unwrap();

        let obj = Object::new(&class).unwrap();
        let result = unsafe { send_message(&obj, &sel, &MessageArgs::one(42)) };

        assert!(result.is_ok());
    }

    #[test]
    fn test_send_message_1_wrong_arg_count() {
        let class = Class::new_root("DispatchWrong1").unwrap();
        let sel = Selector::from_str("noArg`Method`").unwrap();
        let arena = get_global_arena();

        // Add a method with NO arguments
        let imp = test_noop_impl;
        let method = crate::runtime::class::Method {
            selector: sel.clone(),
            imp,
            types: crate::runtime::RuntimeString::new("v@:", arena), // void, no args
        };
        class.add_method(method).unwrap();

        let obj = Object::new(&class).unwrap();
        let result = unsafe { send_message(&obj, &sel, &MessageArgs::one(42)) };

        // Should fail: method expects 0 args (self + _cmd only), we provided 1 arg
        // Error message says "expected 2, got 3" meaning method has 2 args but we provided 3
        assert!(matches!(
            result,
            Err(Error::ArgumentCountMismatch {
                expected: 2,
                got: 3
            })
        ));
    }

    #[test]
    fn test_send_message_2_basic() {
        let class = Class::new_root("DispatchTest2").unwrap();
        let sel = Selector::from_str("method:withArg2:").unwrap();
        let arena = get_global_arena();

        // Add a method taking two arguments
        let imp = test_noop_impl;
        let method = crate::runtime::class::Method {
            selector: sel.clone(),
            imp,
            types: crate::runtime::RuntimeString::new("v@:ii", arena), // void, two int args
        };
        class.add_method(method).unwrap();

        let obj = Object::new(&class).unwrap();
        let result =
            unsafe { send_message(&obj, &sel, &MessageArgs::two(10, 20)) };

        assert!(result.is_ok());
    }

    #[test]
    fn test_send_message_inheritance() {
        // Test that message dispatch finds methods in parent classes
        let parent = Class::new_root("DispatchParent").unwrap();
        let sel = Selector::from_str("inherited`Method`").unwrap();
        let arena = get_global_arena();

        // Add method to parent
        let imp = test_noop_impl;
        let method = crate::runtime::class::Method {
            selector: sel.clone(),
            imp,
            types: crate::runtime::RuntimeString::new("v@:", arena),
        };
        parent.add_method(method).unwrap();

        // Create child class
        let child = Class::new("DispatchChild", &parent).unwrap();
        let obj = Object::new(&child).unwrap();

        // Send message to child (should find method in parent)
        let result = unsafe { send_message(&obj, &sel, &MessageArgs::None) };
        assert!(result.is_ok());
    }

    #[test]
    fn test_send_message_caching() {
        // Test that repeated calls use the cache (performance test)
        let class = Class::new_root("DispatchCache").unwrap();
        let sel = Selector::from_str("cached`Method`").unwrap();
        let arena = get_global_arena();

        let imp = test_noop_impl;
        let method = crate::runtime::class::Method {
            selector: sel.clone(),
            imp,
            types: crate::runtime::RuntimeString::new("v@:", arena),
        };
        class.add_method(method).unwrap();

        let obj = Object::new(&class).unwrap();

        // First call: cache miss
        let _ = unsafe { send_message(&obj, &sel, &MessageArgs::None) };

        // Second call: cache hit (should be faster)
        let _ = unsafe { send_message(&obj, &sel, &MessageArgs::None) };

        // Third call: cache hit
        let result = unsafe { send_message(&obj, &sel, &MessageArgs::None) };
        assert!(result.is_ok());
    }

    #[test]
    fn test_send_message_many_args() {
        static ARGS: [usize; 9] = [1, 2, 3, 4, 5, 6, 7, 8, 9];

        // Test the Many variant with multiple arguments
        let class = Class::new_root("DispatchMany").unwrap();
        let sel = Selector::from_str("methodWithManyArgs:").unwrap();
        let arena = get_global_arena();

        let imp = test_noop_impl;
        let method = crate::runtime::class::Method {
            selector: sel.clone(),
            imp,
            types: crate::runtime::RuntimeString::new("v@:iiiiiiiii", arena), // void, 9 int args
        };
        class.add_method(method).unwrap();

        let obj = Object::new(&class).unwrap();

        let result =
            unsafe { send_message(&obj, &sel, &MessageArgs::many(&ARGS)) };

        assert!(result.is_ok());
    }
}
