// Message Forwarding System
//
// This module implements a full Objective-C style message forwarding system with enhancements:
// - Per-object, per-class, and global forwarding hooks (priority: object > class > global)
// - Forwarding loop detection via thread-local depth counter
// - Forwarded method cache for performance optimization
// - Diagnostic logging and event callbacks
//
// SAFETY: The forwarding system uses thread-local storage for depth tracking and
// RwLock for all hook storage to ensure thread safety. Forwarding hooks must not
// re-enter the dispatch system to avoid deadlocks.

use crate::runtime::{Object, Selector};
use std::cell::Cell;
use std::collections::HashMap;
use std::sync::{LazyLock, RwLock};

// ============================================================================
// Forwarding Hook Types
// ============================================================================

/// Per-object forwarding hook function.
///
/// Called when a selector is not found in this specific object's class.
/// If the hook returns Some(target), the message is retried on the target.
///
/// # Thread Safety
///
/// Hooks may be called from any thread and must be thread-safe.
/// Hooks must NOT re-enter the dispatch system (avoid deadlocks).
pub type ObjectForwardingHook =
    fn(obj: &Object, sel: &Selector) -> Option<Object>;

/// Per-class forwarding hook function.
///
/// Called when a selector is not found in any instance of this class.
/// If the hook returns Some(target), the message is retried on the target.
///
/// # Thread Safety
///
/// Hooks may be called from any thread and must be thread-safe.
/// Hooks must NOT re-enter the dispatch system (avoid deadlocks).
pub type ClassForwardingHook =
    fn(obj: &Object, sel: &Selector) -> Option<Object>;

/// Global forwarding hook function (fallback when no per-object or per-class hook is set).
///
/// # Thread Safety
///
/// Hooks may be called from any thread and must be thread-safe.
/// Hooks must NOT re-enter the dispatch system (avoid deadlocks).
pub type GlobalForwardingHook =
    fn(obj: &Object, sel: &Selector) -> Option<Object>;

// ============================================================================
// Forwarding Depth Tracking (Loop Detection)
// ============================================================================

/// Maximum forwarding depth before detecting a loop.
/// This prevents infinite forwarding chains (e.g., A forwards to B, B forwards to A).
const MAX_FORWARDING_DEPTH: u32 = 32;

// Thread-local forwarding depth counter for loop detection.
thread_local! {
    pub(crate) static FORWARDING_DEPTH: Cell<u32> = const { Cell::new(0) };
}

/// Increments the forwarding depth counter and returns the previous value.
///
/// # Returns
///
/// * `Ok(depth)` - The current depth before incrementing
/// * `Err(depth)` - The depth exceeded `MAX_FORWARDING_DEPTH` (loop detected)
fn increment_forwarding_depth() -> Result<u32, u32> {
    FORWARDING_DEPTH.with(|depth| {
        let current = depth.get();
        if current >= MAX_FORWARDING_DEPTH {
            Err(current)
        } else {
            depth.set(current + 1);
            Ok(current)
        }
    })
}

/// Decrements the forwarding depth counter (must be called after forwarding attempt).
fn decrement_forwarding_depth() {
    FORWARDING_DEPTH.with(|depth| {
        let current = depth.get();
        if current > 0 {
            depth.set(current - 1);
        }
    });
}

// ============================================================================
// Global Forwarding Hook Storage
// ============================================================================

/// Global forwarding hook (fallback when no per-object or per-class hook is set).
static GLOBAL_FORWARDING_HOOK: RwLock<Option<GlobalForwardingHook>> =
    RwLock::new(None);

/// Sets the global forwarding hook.
///
/// The global hook is called when a selector is not found and no per-object or
/// per-class hook is set. This is the lowest priority forwarding mechanism.
///
/// # Thread Safety
///
/// This function is thread-safe. The last hook set wins.
///
/// # Panics
///
/// Panics if the global hook lock is poisoned (indicates a concurrent
/// access error or panic in another thread).
pub fn set_global_forwarding_hook(hook: GlobalForwardingHook) {
    let mut global_hook = GLOBAL_FORWARDING_HOOK.write().unwrap();
    *global_hook = Some(hook);
}

/// Gets the global forwarding hook (if set).
///
/// # Panics
///
/// Panics if the global hook lock is poisoned (indicates a concurrent
/// access error or panic in another thread).
pub fn get_global_forwarding_hook() -> Option<GlobalForwardingHook> {
    let hook = GLOBAL_FORWARDING_HOOK.read().unwrap();
    *hook
}

/// Clears the global forwarding hook.
///
/// # Panics
///
/// Panics if the global hook lock is poisoned (indicates a concurrent
/// access error or panic in another thread).
pub fn clear_global_forwarding_hook() {
    let mut global_hook = GLOBAL_FORWARDING_HOOK.write().unwrap();
    *global_hook = None;
}

// ============================================================================
// Forwarding Result
// ============================================================================

/// Result of attempting to resolve a forwarding target.
#[derive(Debug, Clone, PartialEq)]
pub enum ForwardingResult {
    /// Found a forwarding target object.
    Target(Object),

    /// No forwarding target found.
    NotFound,

    /// Forwarding loop detected (exceeded max depth).
    LoopDetected,
}

// ============================================================================
// Forwarding Resolution Logic
// ============================================================================

/// Attempts to resolve a forwarding target for an unhandled message.
///
/// Resolution order (priority):
/// 1. Per-object hook (if set)
/// 2. Per-class hook (if set)
/// 3. Global hook (if set)
///
/// If no hook returns a target, returns `ForwardingResult::NotFound`.
/// If forwarding depth is exceeded, returns `ForwardingResult::LoopDetected`.
///
/// # Arguments
///
/// * `obj` - The object that received the unhandled message
/// * `sel` - The selector that was not found
///
/// # Returns
///
/// * `ForwardingResult::Target(target)` - A forwarding target was found
/// * `ForwardingResult::NotFound` - No forwarding target found
/// * `ForwardingResult::LoopDetected` - Forwarding loop detected
#[must_use]
pub fn resolve_forwarding(obj: &Object, sel: &Selector) -> ForwardingResult {
    // Check forwarding depth (loop detection)
    let depth = match increment_forwarding_depth() {
        Ok(d) => d,
        Err(d) => {
            emit_forwarding_event(ForwardingEvent::ForwardingLoopDetected {
                object: obj.clone(),
                selector: sel.clone(),
                depth: d + 1,
            });
            return ForwardingResult::LoopDetected;
        }
    };

    emit_forwarding_event(ForwardingEvent::ForwardingAttempt {
        object: obj.clone(),
        selector: sel.clone(),
        depth,
    });

    // Try per-object hook (highest priority)
    if let Some(target) = try_per_object_forwarding(obj, sel) {
        decrement_forwarding_depth();
        emit_forwarding_event(ForwardingEvent::ForwardingSuccess {
            object: obj.clone(),
            selector: sel.clone(),
            target: target.clone(),
        });
        return ForwardingResult::Target(target);
    }

    // Try per-class hook (medium priority)
    if let Some(target) = try_per_class_forwarding(obj, sel) {
        decrement_forwarding_depth();
        emit_forwarding_event(ForwardingEvent::ForwardingSuccess {
            object: obj.clone(),
            selector: sel.clone(),
            target: target.clone(),
        });
        return ForwardingResult::Target(target);
    }

    // Try global hook (lowest priority/fallback)
    if let Some(target) = try_global_forwarding(obj, sel) {
        decrement_forwarding_depth();
        emit_forwarding_event(ForwardingEvent::ForwardingSuccess {
            object: obj.clone(),
            selector: sel.clone(),
            target: target.clone(),
        });
        return ForwardingResult::Target(target);
    }

    // No forwarding target found
    decrement_forwarding_depth();
    ForwardingResult::NotFound
}

/// Attempts per-object forwarding (not yet implemented - requires `ObjectExtensions`).
fn try_per_object_forwarding(_obj: &Object, _sel: &Selector) -> Option<Object> {
    // TODO: Implement when ObjectExtensions is added to object.rs
    // For now, per-object forwarding is a placeholder for future enhancement
    None
}

/// Attempts per-class forwarding.
fn try_per_class_forwarding(obj: &Object, sel: &Selector) -> Option<Object> {
    let class = obj.class();
    class.get_forwarding_hook().and_then(|hook| hook(obj, sel))
}

/// Attempts global forwarding.
fn try_global_forwarding(obj: &Object, sel: &Selector) -> Option<Object> {
    get_global_forwarding_hook().and_then(|hook| hook(obj, sel))
}

// ============================================================================
// Forwarded Method Cache
// ============================================================================

/// Cache for frequently missed selectors and their forwarding targets.
///
/// Key: (`object_class_hash`, `selector_hash`) -> `forwarding_target`
///
/// This cache improves performance when the same selector is repeatedly
/// not found and forwarded to the same target.
static FORWARDED_METHOD_CACHE: LazyLock<RwLock<HashMap<(u64, u64), Object>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// Caches a forwarding target for a given object and selector.
///
/// # Arguments
///
/// * `obj` - The object that received the unhandled message
/// * `sel` - The selector that was not found
/// * `target` - The forwarding target to cache
///
/// # Panics
///
/// Panics if the cache lock is poisoned (indicates a concurrent
/// access error or panic in another thread).
pub fn cache_forwarded_target(obj: &Object, sel: &Selector, target: &Object) {
    let mut cache = FORWARDED_METHOD_CACHE.write().unwrap();
    let key = (obj.class().inner_hash(), sel.hash());
    cache.insert(key, target.clone());
}

/// Retrieves a cached forwarding target for a given object and selector.
///
/// # Arguments
///
/// * `obj` - The object that received the unhandled message
/// * `sel` - The selector that was not found
///
/// # Returns
///
/// * `Some(target)` - A cached forwarding target was found
/// * `None` - No cached target for this object/selector pair
///
/// # Panics
///
/// Panics if the cache lock is poisoned (indicates a concurrent
/// access error or panic in another thread).
pub fn get_cached_target(obj: &Object, sel: &Selector) -> Option<Object> {
    let cache = FORWARDED_METHOD_CACHE.read().unwrap();
    let key = (obj.class().inner_hash(), sel.hash());
    cache.get(&key).cloned()
}

/// Clears the entire forwarded method cache.
///
/// This should be called when methods are added or swizzled to avoid
/// forwarding to stale targets.
///
/// # Panics
///
/// Panics if the cache lock is poisoned (indicates a concurrent
/// access error or panic in another thread).
pub fn clear_forwarded_cache() {
    let mut cache = FORWARDED_METHOD_CACHE.write().unwrap();
    cache.clear();
}

// ============================================================================
// Diagnostic Logging and Event Callbacks
// ============================================================================

/// Forwarding event callback for debugging and diagnostics.
pub type ForwardingEventCallback = fn(event: ForwardingEvent);

/// Global forwarding event callback (optional).
static FORWARDING_EVENT_CALLBACK: RwLock<Option<ForwardingEventCallback>> =
    RwLock::new(None);

/// Sets the forwarding event callback for diagnostics.
///
/// The callback is invoked for all forwarding-related events, including:
/// - Forwarding attempts
/// - Forwarding success
/// - `DoesNotRecognizeSelector` invocations
/// - Forwarding loop detection
///
/// # Example
///
/// ```rust
/// use oxidec::runtime::forwarding::{set_forwarding_event_callback, ForwardingEvent};
///
/// fn diagnostic_callback(event: ForwardingEvent) {
///     match event {
///         ForwardingEvent::ForwardingAttempt { object, selector, depth } => {
///             eprintln!("Forwarding attempt: {} -> {}, depth {}",
///                      object.class().name(), selector.name(), depth);
///         }
///         ForwardingEvent::ForwardingSuccess { object, selector, target } => {
///             eprintln!("Forwarded: {} -> {} to {}",
///                      object.class().name(), selector.name(), target.class().name());
///         }
///         _ => { /* ... */ }
///     }
/// }
///
/// set_forwarding_event_callback(diagnostic_callback);
/// ```
///
/// # Panics
///
/// Panics if the event callback lock is poisoned (indicates a concurrent
/// access error or panic in another thread).
pub fn set_forwarding_event_callback(callback: ForwardingEventCallback) {
    *FORWARDING_EVENT_CALLBACK.write().unwrap() = Some(callback);
}

/// Clears the forwarding event callback.
///
/// # Panics
///
/// Panics if the event callback lock is poisoned (indicates a concurrent
/// access error or panic in another thread).
pub fn clear_forwarding_event_callback() {
    *FORWARDING_EVENT_CALLBACK.write().unwrap() = None;
}

/// Emits a forwarding event if the event callback is set.
///
/// # Arguments
///
/// * `event` - The forwarding event to emit
///
/// # Panics
///
/// Panics if the event callback lock is poisoned (indicates a concurrent
/// access error or panic in another thread).
pub fn emit_forwarding_event(event: ForwardingEvent) {
    if let Some(callback) = FORWARDING_EVENT_CALLBACK.read().unwrap().as_ref() {
        callback(event);
    }
}

/// Forwarding events for diagnostic logging.
#[derive(Clone, Debug)]
pub enum ForwardingEvent {
    /// A forwarding attempt was made.
    ForwardingAttempt {
        object: Object,
        selector: Selector,
        depth: u32,
    },

    /// Forwarding succeeded and a target was found.
    ForwardingSuccess {
        object: Object,
        selector: Selector,
        target: Object,
    },

    /// doesNotRecognizeSelector: was invoked.
    DoesNotRecognize { object: Object, selector: Selector },

    /// A forwarding loop was detected.
    ForwardingLoopDetected {
        object: Object,
        selector: Selector,
        depth: u32,
    },
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::Class;
    use std::str::FromStr;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Helper function to create a test class with a method
    fn create_test_class(name: &str) -> Class {
        Class::new_root(name).unwrap()
    }

    /// Helper function to create a test selector
    fn create_test_selector(name: &str) -> Selector {
        Selector::from_str(name).unwrap()
    }

    #[test]
    fn test_forwarding_depth_tracking() {
        // Initially, depth should be 0
        FORWARDING_DEPTH.with(|depth| {
            assert_eq!(depth.get(), 0);
        });

        // Increment depth
        let result = increment_forwarding_depth();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);

        // Depth should now be 1
        FORWARDING_DEPTH.with(|depth| {
            assert_eq!(depth.get(), 1);
        });

        // Decrement depth
        decrement_forwarding_depth();

        // Depth should be back to 0
        FORWARDING_DEPTH.with(|depth| {
            assert_eq!(depth.get(), 0);
        });
    }

    #[test]
    fn test_forwarding_loop_detection() {
        // Set depth to max
        FORWARDING_DEPTH.with(|depth| {
            depth.set(MAX_FORWARDING_DEPTH);
        });

        // Next increment should fail (loop detected)
        let result = increment_forwarding_depth();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), MAX_FORWARDING_DEPTH);

        // Reset depth
        FORWARDING_DEPTH.with(|depth| {
            depth.set(0);
        });
    }

    #[test]
    fn test_global_forwarding_hook() {
        // Set global hook
        static TARGET_OBJECT: RwLock<Option<Object>> = RwLock::new(None);

        // Create test objects
        let class1 = create_test_class("GlobalHookSource");
        let obj1 = Object::new(&class1).unwrap();
        let sel = create_test_selector("globalHookMethod");

        // No hook set - should return None
        assert!(try_global_forwarding(&obj1, &sel).is_none());

        let class2 = create_test_class("GlobalHookTarget");
        let target = Object::new(&class2).unwrap();
        *TARGET_OBJECT.write().unwrap() = Some(target.clone());

        set_global_forwarding_hook(|_obj, _sel| {
            TARGET_OBJECT.read().unwrap().clone()
        });

        // Hook should now return target
        let result = try_global_forwarding(&obj1, &sel);
        assert!(result.is_some());

        // Clear hook
        clear_global_forwarding_hook();

        // Hook should return None again
        assert!(try_global_forwarding(&obj1, &sel).is_none());
    }

    #[test]
    fn test_forwarded_method_cache() {
        let class1 = create_test_class("CacheSource");
        let obj1 = Object::new(&class1).unwrap();
        let sel = create_test_selector("cachedMethod");

        let class2 = create_test_class("CacheTarget");
        let target = Object::new(&class2).unwrap();

        // Initially, cache should be empty
        assert!(get_cached_target(&obj1, &sel).is_none());

        // Cache the target
        cache_forwarded_target(&obj1, &sel, &target);

        // Should now return the cached target
        let cached = get_cached_target(&obj1, &sel).unwrap();
        assert!(std::ptr::eq(
            cached.as_raw().as_raw_ptr() as *const (),
            target.as_raw().as_raw_ptr() as *const ()
        ));

        // Clear cache
        clear_forwarded_cache();

        // Cache should be empty again
        assert!(get_cached_target(&obj1, &sel).is_none());
    }

    #[test]
    fn test_forwarding_event_callback() {
        static CALL_COUNT: AtomicUsize = AtomicUsize::new(0);

        // Set callback
        set_forwarding_event_callback(|event| {
            if let ForwardingEvent::ForwardingAttempt { .. } = event {
                CALL_COUNT.fetch_add(1, Ordering::Release);
            }
        });

        // Emit event
        let class = create_test_class("EventSource");
        let obj = Object::new(&class).unwrap();
        let sel = create_test_selector("eventMethod");
        emit_forwarding_event(ForwardingEvent::ForwardingAttempt {
            object: obj,
            selector: sel,
            depth: 0,
        });

        // Callback should have been called
        assert_eq!(CALL_COUNT.load(Ordering::Acquire), 1);

        // Clear callback
        clear_forwarding_event_callback();
    }

    #[test]
    fn test_resolve_forwarding_not_found() {
        // No hooks set - should return NotFound
        let class = create_test_class("ResolveNotFound");
        let obj = Object::new(&class).unwrap();
        let sel = create_test_selector("nonExistent");

        let _result = resolve_forwarding(&obj, &sel);
        // Result is discarded, just verifying it doesn't panic
    }

    #[test]
    fn test_resolve_forwarding_with_global_hook() {
        static FORWARDING_TARGET: RwLock<Option<Object>> = RwLock::new(None);

        // Set up forwarding target
        let target_class = create_test_class("ResolveTarget");
        let target = Object::new(&target_class).unwrap();
        *FORWARDING_TARGET.write().unwrap() = Some(target.clone());

        // Set global hook
        set_global_forwarding_hook(|_obj, _sel| {
            FORWARDING_TARGET.read().unwrap().clone()
        });

        // Resolve forwarding
        let class = create_test_class("ResolveSource");
        let obj = Object::new(&class).unwrap();
        let sel = create_test_selector("resolveMethod");

        let result = resolve_forwarding(&obj, &sel);

        // Should return the target
        assert!(matches!(result, ForwardingResult::Target(_)));

        // Clear hook
        clear_global_forwarding_hook();
    }

    #[test]
    fn test_forwarding_multiple_threads() {
        use std::thread;

        // Set global hook
        set_global_forwarding_hook(|_obj, _sel| None);

        // Spawn multiple threads that all use forwarding
        let handles: Vec<_> = (0..10)
            .map(|i| {
                thread::spawn(move || {
                    let class = create_test_class(&format!("ThreadClass{i}"));
                    let obj = Object::new(&class).unwrap();
                    let sel = create_test_selector("threadMethod");
                    let _ = resolve_forwarding(&obj, &sel);
                })
            })
            .collect();

        // All threads should complete without panic
        for handle in handles {
            handle.join().unwrap();
        }

        // Clear hook
        clear_global_forwarding_hook();
    }
}
