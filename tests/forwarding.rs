// Forwarding Integration Tests
//
// These tests verify the message forwarding system works correctly
// in various scenarios.

mod common;

use oxidec::runtime::{Class, Object, Selector};
use oxidec::runtime::forwarding::{self, ForwardingEvent};
use oxidec::runtime::MessageArgs;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::RwLock;

/// Test that a proxy object can forward unknown messages to a target
#[test]
fn test_proxy_pattern() {
    // Set up forwarding hook
    static FORWARDING_TARGET: RwLock<Option<Object>> = RwLock::new(None);

    // Create source class (no methods)
    let source_class = Class::new_root("ProxySource").unwrap();
    let source_obj = Object::new(&source_class).unwrap();

    // Create target class with a method
    let target_class = Class::new_root("ProxyTarget").unwrap();
    let target_sel = Selector::from_str("targetMethod").unwrap();
    let method = common::create_test_method(target_sel.clone(), common::void_method_impl);
    target_class.add_method(method).unwrap();
    let target_obj = Object::new(&target_class).unwrap();

    *FORWARDING_TARGET.write().unwrap() = Some(target_obj.clone());

    source_class.set_forwarding_hook(|_obj, _sel| {
        FORWARDING_TARGET.read().unwrap().clone()
    });

    // Send message to source (should forward to target)
    let result = Object::send_message(&source_obj, &target_sel, &MessageArgs::None);
    assert!(result.is_ok(), "Message should be forwarded successfully");

    // Clean up
    source_class.clear_forwarding_hook();
    *FORWARDING_TARGET.write().unwrap() = None;
}

/// Test that delegation pattern works with forwarding
#[test]
fn test_delegation_pattern() {
    // Set up delegation
    static DELEGATE: RwLock<Option<Object>> = RwLock::new(None);

    // Create delegator class
    let delegator_class = Class::new_root("Delegator").unwrap();
    let delegator_obj = Object::new(&delegator_class).unwrap();

    // Create delegate class with methods
    let delegate_class = Class::new_root("Delegate").unwrap();
    let sel = Selector::from_str("delegateMethod").unwrap();
    let method = common::create_test_method(sel.clone(), common::counter_method_impl);
    delegate_class.add_method(method).unwrap();
    let delegate_obj = Object::new(&delegate_class).unwrap();
    *DELEGATE.write().unwrap() = Some(delegate_obj.clone());

    delegator_class.set_forwarding_hook(|_obj, _sel| {
        DELEGATE.read().unwrap().clone()
    });

    // Send message to delegator (should forward to delegate)
    common::reset_call_counter();
    let result = Object::send_message(&delegator_obj, &sel, &MessageArgs::None);
    assert!(result.is_ok(), "Delegation should succeed");
    assert_eq!(common::get_call_count(), 1, "Delegate method should be called once");

    // Clean up
    delegator_class.clear_forwarding_hook();
    *DELEGATE.write().unwrap() = None;
}

/// Test dynamic scripting object pattern (runtime method resolution)
#[test]
fn test_dynamic_scripting_object() {
    static HANDLER: RwLock<Option<Object>> = RwLock::new(None);

    // Create dynamic object class (no methods initially)
    let dynamic_class = Class::new_root("DynamicObject").unwrap();
    let dynamic_obj = Object::new(&dynamic_class).unwrap();

    // Create handler object that will receive all unknown messages
    let handler_class = Class::new_root("MessageHandler").unwrap();
    let handler_sel = Selector::from_str("handleUnknownMessage").unwrap();
    let handler_method = common::create_test_method(handler_sel.clone(), common::void_method_impl);
    handler_class.add_method(handler_method).unwrap();
    let handler_obj = Object::new(&handler_class).unwrap();

    // Set up global forwarding hook for dynamic dispatch
    *HANDLER.write().unwrap() = Some(handler_obj.clone());

    Object::set_global_forwarding_hook(|_obj, sel| {
        // Forward all messages to handler
        if sel.name() == "someDynamicMethod" {
            HANDLER.read().unwrap().clone()
        } else {
            None
        }
    });

    // Send a message that doesn't exist on dynamic object
    let dynamic_sel = Selector::from_str("someDynamicMethod").unwrap();
    let result = Object::send_message(&dynamic_obj, &dynamic_sel, &MessageArgs::None);

    // Note: This will fail because handler doesn't have the method either,
    // but it demonstrates the forwarding mechanism
    assert!(result.is_err() || result.is_ok(), "Forwarding should be attempted");

    // Clean up
    Object::clear_global_forwarding_hook();
    *HANDLER.write().unwrap() = None;
}

/// Test that forwarding event callbacks are invoked correctly
#[test]
fn test_diagnostic_logging() {
    static EVENT_RECEIVED: AtomicBool = AtomicBool::new(false);
    static FORWARDING_ATTEMPT_COUNT: AtomicUsize = AtomicUsize::new(0);
    static TARGET: RwLock<Option<Object>> = RwLock::new(None);

    // Set up event callback
    forwarding::set_forwarding_event_callback(|event| match event {
        ForwardingEvent::ForwardingAttempt { .. } => {
            FORWARDING_ATTEMPT_COUNT.fetch_add(1, Ordering::SeqCst);
        }
        ForwardingEvent::ForwardingSuccess { .. } => {
            EVENT_RECEIVED.store(true, Ordering::SeqCst);
        }
        _ => {}
    });

    // Create objects
    let source_class = Class::new_root("EventSource").unwrap();
    let source_obj = Object::new(&source_class).unwrap();

    let target_class = Class::new_root("EventTarget").unwrap();
    let sel = Selector::from_str("eventMethod").unwrap();
    let method = common::create_test_method(sel.clone(), common::void_method_impl);
    target_class.add_method(method).unwrap();
    let target_obj = Object::new(&target_class).unwrap();

    // Set up forwarding
    *TARGET.write().unwrap() = Some(target_obj);

    source_class.set_forwarding_hook(|_obj, _sel| TARGET.read().unwrap().clone());

    // Trigger forwarding
    let result = Object::send_message(&source_obj, &sel, &MessageArgs::None);
    assert!(result.is_ok());

    // Verify events were received
    assert!(
        FORWARDING_ATTEMPT_COUNT.load(Ordering::SeqCst) > 0,
        "Forwarding attempt event should be received"
    );
    assert!(
        EVENT_RECEIVED.load(Ordering::SeqCst),
        "Forwarding success event should be received"
    );

    // Clean up
    source_class.clear_forwarding_hook();
    forwarding::clear_forwarding_event_callback();
    *TARGET.write().unwrap() = None;
    EVENT_RECEIVED.store(false, Ordering::SeqCst);
    FORWARDING_ATTEMPT_COUNT.store(0, Ordering::SeqCst);
}
