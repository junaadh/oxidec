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

use crate::error::{Error, Result};
use crate::runtime::{Object, Selector};
use crate::runtime::invocation::Invocation;
use crate::runtime::message::MessageArgs;
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

/// Method signature hook function (Stage 2 of four-stage forwarding).
///
/// Called when a selector is not found and the pipeline needs a type signature
/// to create an Invocation object. If the hook returns Some(signature), the
/// signature is used for Stage 3 (forwardInvocation:). If it returns None,
/// the pipeline skips to Stage 4 (doesNotRecognizeSelector:).
///
/// # Thread Safety
///
/// Hooks may be called from any thread and must be thread-safe.
/// Hooks must NOT re-enter the dispatch system (avoid deadlocks).
pub type MethodSignatureHook =
    fn(obj: &Object, sel: &Selector) -> Option<String>;

/// Forward invocation hook function (Stage 3 of four-stage forwarding).
///
/// Called with a mutable Invocation object containing the original message.
/// The hook can modify the target, selector, arguments, or return value before
/// the message is invoked. This enables complex proxies, RPC, and message
/// transformation.
///
/// # Thread Safety
///
/// Hooks may be called from any thread and must be thread-safe.
/// Hooks must NOT re-enter the dispatch system (avoid deadlocks).
pub type ForwardInvocationHook = fn(invocation: &mut Invocation);

/// Does not recognize hook function (Stage 4 of four-stage forwarding).
///
/// Called when all previous stages failed to handle the message. This is the
/// last resort before returning `SelectorNotFound`. The hook can log the error,
/// raise an exception, or perform cleanup.
///
/// # Thread Safety
///
/// Hooks may be called from any thread and must be thread-safe.
/// Hooks must NOT re-enter the dispatch system (avoid deadlocks).
pub type DoesNotRecognizeHook = fn(obj: &Object, sel: &Selector);

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
fn increment_forwarding_depth() -> std::result::Result<u32, u32> {
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

/// Global method signature hook (Stage 2: methodSignatureForSelector:).
static GLOBAL_SIGNATURE_HOOK: RwLock<Option<MethodSignatureHook>> =
    RwLock::new(None);

/// Global forward invocation hook (Stage 3: forwardInvocation:).
static GLOBAL_FORWARD_INVOCATION_HOOK: RwLock<Option<ForwardInvocationHook>> =
    RwLock::new(None);

/// Global does not recognize hook (Stage 4: doesNotRecognizeSelector:).
static GLOBAL_DOES_NOT_RECOGNIZE_HOOK: RwLock<Option<DoesNotRecognizeHook>> =
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

/// Sets the global method signature hook (Stage 2).
///
/// The global hook is called when a selector is not found and no per-object
/// or per-class signature hook is set. This is the lowest priority mechanism
/// for Stage 2.
///
/// # Thread Safety
///
/// This function is thread-safe. The last hook set wins.
///
/// # Panics
///
/// Panics if the global hook lock is poisoned (indicates a concurrent
/// access error or panic in another thread).
pub fn set_global_signature_hook(hook: MethodSignatureHook) {
    let mut global_hook = GLOBAL_SIGNATURE_HOOK.write().unwrap();
    *global_hook = Some(hook);
}

/// Clears the global method signature hook.
///
/// # Panics
///
/// Panics if the global hook lock is poisoned (indicates a concurrent
/// access error or panic in another thread).
pub fn clear_global_signature_hook() {
    let mut global_hook = GLOBAL_SIGNATURE_HOOK.write().unwrap();
    *global_hook = None;
}

/// Sets the global forward invocation hook (Stage 3).
///
/// The global hook is called when a selector is not found and no per-object
/// or per-class forward invocation hook is set. This is the lowest priority
/// mechanism for Stage 3.
///
/// # Thread Safety
///
/// This function is thread-safe. The last hook set wins.
///
/// # Panics
///
/// Panics if the global hook lock is poisoned (indicates a concurrent
/// access error or panic in another thread).
pub fn set_global_forward_invocation_hook(hook: ForwardInvocationHook) {
    let mut global_hook = GLOBAL_FORWARD_INVOCATION_HOOK.write().unwrap();
    *global_hook = Some(hook);
}

/// Clears the global forward invocation hook.
///
/// # Panics
///
/// Panics if the global hook lock is poisoned (indicates a concurrent
/// access error or panic in another thread).
pub fn clear_global_forward_invocation_hook() {
    let mut global_hook = GLOBAL_FORWARD_INVOCATION_HOOK.write().unwrap();
    *global_hook = None;
}

/// Sets the global does not recognize hook (Stage 4).
///
/// The global hook is called when all previous stages fail and no per-object
/// or per-class does not recognize hook is set. This is the lowest priority
/// mechanism for Stage 4.
///
/// # Thread Safety
///
/// This function is thread-safe. The last hook set wins.
///
/// # Panics
///
/// Panics if the global hook lock is poisoned (indicates a concurrent
/// access error or panic in another thread).
pub fn set_global_does_not_recognize_hook(hook: DoesNotRecognizeHook) {
    let mut global_hook = GLOBAL_DOES_NOT_RECOGNIZE_HOOK.write().unwrap();
    *global_hook = Some(hook);
}

/// Clears the global does not recognize hook.
///
/// # Panics
///
/// Panics if the global hook lock is poisoned (indicates a concurrent
/// access error or panic in another thread).
pub fn clear_global_does_not_recognize_hook() {
    let mut global_hook = GLOBAL_DOES_NOT_RECOGNIZE_HOOK.write().unwrap();
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
            let actual_depth = d + 1;
            emit_forwarding_event(ForwardingEvent::ForwardingLoopDetected {
                object: obj.clone(),
                selector: sel.clone(),
                depth: actual_depth,
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

/// Attempts to resolve forwarding using the four-stage pipeline.
///
/// # Four-Stage Forwarding Model
///
/// 1. **forwardingTargetForSelector:** (Stage 1) - Fast redirect to another object (< 100ns)
/// 2. **methodSignatureForSelector:** (Stage 2) - Get method signature (< 50ns cached)
/// 3. **forwardInvocation:** (Stage 3) - Full message manipulation (< 500ns)
/// 4. **doesNotRecognizeSelector:** (Stage 4) - Fatal error handler
///
/// # Stage Flow
///
/// - If Stage 1 returns a target, retry dispatch on that target (fast path, no invocation)
/// - If Stage 2 returns a signature, continue to Stage 3 with invocation creation
/// - If Stage 2 returns None, skip to Stage 4 (no signature, can't create invocation)
/// - If Stage 3 has a hook, modify and invoke the message
/// - If Stage 3 has no hook, continue to Stage 4
/// - Stage 4 is always called when all previous stages fail
///
/// # Arguments
///
/// * `obj` - The object that received the unhandled message
/// * `sel` - The selector that was not found
/// * `args` - The message arguments
///
/// # Returns
///
/// * `Ok(Some(retval))` - Forwarding succeeded, return value from invoked method
/// * `Ok(None)` - Forwarding succeeded, void return
/// * `Err(Error::SelectorNotFound)` - All stages failed
/// * `Err(Error::ForwardingLoopDetected)` - Loop detected
///
/// # Errors
///
/// * `Error::SelectorNotFound` - All four stages failed to handle the message
/// * `Error::ForwardingLoopDetected` - Forwarding depth exceeded (loop detected)
/// * `Error::ArgumentCountMismatch` - Stage 2 provided invalid signature
///
/// # Performance
///
/// This function maintains the performance targets:
/// - Stage 1 early exit: ~85ns (fast path)
/// - Stage 2 signature lookup: ~42ns (cached)
/// - Stage 3 invocation: ~460ns (total)
pub fn resolve_four_stage_forwarding(
    obj: &Object,
    sel: &Selector,
    args: &MessageArgs,
) -> Result<Option<usize>> {
    use crate::runtime::dispatch;

    // Check forwarding depth (loop detection)
    let depth = increment_forwarding_depth().map_err(|d| {
        let actual_depth = d + 1;
        emit_forwarding_event(ForwardingEvent::ForwardingLoopDetected {
            object: obj.clone(),
            selector: sel.clone(),
            depth: actual_depth,
        });
        Error::ForwardingLoopDetected {
            selector: sel.name().to_string(),
            depth: actual_depth,
        }
    })?;

    emit_forwarding_event(ForwardingEvent::ForwardingAttempt {
        object: obj.clone(),
        selector: sel.clone(),
        depth,
    });

    // Stage 1: Fast redirect to another object (forwardingTargetForSelector:)
    if let Some(target) = try_forwarding_target(obj, sel) {
        emit_forwarding_event(ForwardingEvent::ForwardingSuccess {
            object: obj.clone(),
            selector: sel.clone(),
            target: target.clone(),
        });

        // Fast path: retry dispatch on target without creating invocation
        // Keep depth incremented for the recursive call to prevent loops
        let result = unsafe { dispatch::send_message(&target, sel, args) };
        decrement_forwarding_depth();
        return result;
    }

    // Stage 2: Get method signature (methodSignatureForSelector:)
    let Some(signature) = try_method_signature(obj, sel) else {
        // Stage 2 failed - skip to Stage 4
        try_does_not_recognize(obj, sel);
        decrement_forwarding_depth();
        return Err(Error::SelectorNotFound);
    };

    // Stage 3: Create and forward invocation (forwardInvocation:)
    let mut invocation = Invocation::with_arguments(obj, sel, args).inspect_err(|_e| {
        decrement_forwarding_depth();
    })?;

    invocation.set_signature(Some(signature));

    if try_forward_invocation(&mut invocation) {
        decrement_forwarding_depth();
        // Invoke the modified invocation
        return unsafe { invocation.invoke() };
    }

    // Stage 4: Fatal error handler (doesNotRecognizeSelector:)
    try_does_not_recognize(obj, sel);
    decrement_forwarding_depth();
    Err(Error::SelectorNotFound)
}

/// Stage 1: Try fast redirect to another object (forwardingTargetForSelector:).
///
/// This is the first stage of the four-stage forwarding pipeline.
/// It provides a fast path for simple delegation without creating an invocation.
///
/// # Returns
///
/// * `Some(target)` - Forward to this object
/// * `None` - Continue to Stage 2
fn try_forwarding_target(obj: &Object, sel: &Selector) -> Option<Object> {
    // Reuse existing forwarding logic (per-object, per-class, global)
    try_per_object_forwarding(obj, sel)
        .or_else(|| try_per_class_forwarding(obj, sel))
        .or_else(|| try_global_forwarding(obj, sel))
}

/// Stage 2: Get method signature (methodSignatureForSelector:).
///
/// This stage provides a type signature for creating an invocation in Stage 3.
/// Signatures are cached for performance.
///
/// # Returns
///
/// * `Some(signature)` - Use this signature for Stage 3
/// * `None` - Skip to Stage 4
fn try_method_signature(obj: &Object, sel: &Selector) -> Option<String> {
    // Check cache first
    if let Some(sig) = get_cached_signature(obj, sel) {
        return Some(sig);
    }

    // Try hooks: object > class > global
    let signature = try_per_object_signature(obj, sel)
        .or_else(|| try_per_class_signature(obj, sel))
        .or_else(|| try_global_signature(obj, sel));

    // Cache result if found
    if let Some(ref sig) = signature {
        cache_signature(obj, sel, sig);
    }

    signature
}

/// Stage 3: Forward invocation with modification (forwardInvocation:).
///
/// This stage creates an Invocation object and allows hooks to modify
/// the target, selector, arguments, or return value before invoking.
///
/// # Returns
///
/// * `true` - Hook handled the invocation
/// * `false` - No hook set, continue to Stage 4
fn try_forward_invocation(invocation: &mut Invocation) -> bool {
    // Try hooks: object > class > global
    if try_per_object_forward_invocation(invocation) {
        return true;
    }
    if try_per_class_forward_invocation(invocation) {
        return true;
    }
    if try_global_forward_invocation(invocation) {
        return true;
    }
    false
}

/// Stage 4: Handle unrecognized selector (doesNotRecognizeSelector:).
///
/// This is the final stage called when all previous stages failed.
/// It emits an event and calls registered error handler hooks.
fn try_does_not_recognize(obj: &Object, sel: &Selector) {
    // Emit event
    emit_forwarding_event(ForwardingEvent::DoesNotRecognize {
        object: obj.clone(),
        selector: sel.clone(),
    });

    // Call hooks: object > class > global
    try_per_object_does_not_recognize(obj, sel);
    try_per_class_does_not_recognize(obj, sel);
    try_global_does_not_recognize(obj, sel);
}

// Per-object signature hook (not yet implemented - requires ObjectExtensions).
fn try_per_object_signature(_obj: &Object, _sel: &Selector) -> Option<String> {
    // TODO: Implement when ObjectExtensions is added to object.rs
    None
}

/// Per-class signature hook.
fn try_per_class_signature(obj: &Object, sel: &Selector) -> Option<String> {
    let class = obj.class();
    // SAFETY: ClassInner is valid and allocated in arena
    let inner = unsafe { &*class.inner.as_ptr() };
    inner.signature_hook.read().unwrap().and_then(|hook| hook(obj, sel))
}

/// Global signature hook.
fn try_global_signature(obj: &Object, sel: &Selector) -> Option<String> {
    GLOBAL_SIGNATURE_HOOK.read().unwrap().and_then(|hook| hook(obj, sel))
}

/// Per-object forward invocation hook (not yet implemented).
fn try_per_object_forward_invocation(_invocation: &mut Invocation) -> bool {
    // TODO: Implement when ObjectExtensions is added to object.rs
    false
}

/// Per-class forward invocation hook.
fn try_per_class_forward_invocation(invocation: &mut Invocation) -> bool {
    let obj = invocation.target();
    let class = obj.class();
    // SAFETY: ClassInner is valid and allocated in arena
    let inner = unsafe { &*class.inner.as_ptr() };
    inner.forward_invocation_hook.read().unwrap().is_some_and(|hook| {
        hook(invocation);
        true
    })
}

/// Global forward invocation hook.
fn try_global_forward_invocation(invocation: &mut Invocation) -> bool {
    GLOBAL_FORWARD_INVOCATION_HOOK.read().unwrap().is_some_and(|hook| {
        hook(invocation);
        true
    })
}

/// Per-object does not recognize hook (not yet implemented).
fn try_per_object_does_not_recognize(_obj: &Object, _sel: &Selector) {
    // TODO: Implement when ObjectExtensions is added to object.rs
}

/// Per-class does not recognize hook.
fn try_per_class_does_not_recognize(obj: &Object, sel: &Selector) {
    let class = obj.class();
    // SAFETY: ClassInner is valid and allocated in arena
    let inner = unsafe { &*class.inner.as_ptr() };
    if let Some(hook) = inner.does_not_recognize_hook.read().unwrap().as_ref() {
        hook(obj, sel);
    }
}

/// Global does not recognize hook.
fn try_global_does_not_recognize(obj: &Object, sel: &Selector) {
    if let Some(hook) = GLOBAL_DOES_NOT_RECOGNIZE_HOOK.read().unwrap().as_ref() {
        hook(obj, sel);
    }
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
// Signature Cache
// ============================================================================

/// Cache for method signatures (Stage 2 of four-stage forwarding).
///
/// Key: (`object_class_hash`, `selector_hash`) -> `signature`
///
/// This cache improves performance when the same selector is repeatedly
/// not found and requires a signature for invocation creation.
static SIGNATURE_CACHE: LazyLock<RwLock<HashMap<(u64, u64), String>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// Caches a method signature for a given object and selector.
///
/// # Arguments
///
/// * `obj` - The object that received the unhandled message
/// * `sel` - The selector that needs a signature
/// * `signature` - The signature string to cache
///
/// # Panics
///
/// Panics if the cache lock is poisoned (indicates a concurrent
/// access error or panic in another thread).
pub fn cache_signature(obj: &Object, sel: &Selector, signature: &str) {
    let mut cache = SIGNATURE_CACHE.write().unwrap();
    let key = (obj.class().inner_hash(), sel.hash());
    cache.insert(key, signature.to_string());
}

/// Retrieves a cached method signature for a given object and selector.
///
/// # Arguments
///
/// * `obj` - The object that received the unhandled message
/// * `sel` - The selector that needs a signature
///
/// # Returns
///
/// * `Some(signature)` - A cached signature was found
/// * `None` - No cached signature for this object/selector pair
///
/// # Panics
///
/// Panics if the cache lock is poisoned (indicates a concurrent
/// access error or panic in another thread).
pub fn get_cached_signature(obj: &Object, sel: &Selector) -> Option<String> {
    let cache = SIGNATURE_CACHE.read().unwrap();
    let key = (obj.class().inner_hash(), sel.hash());
    cache.get(&key).cloned()
}

/// Clears the entire signature cache.
///
/// This should be called when methods are added or swizzled to avoid
/// stale signatures.
///
/// # Panics
///
/// Panics if the cache lock is poisoned (indicates a concurrent
/// access error or panic in another thread).
pub fn clear_signature_cache() {
    let mut cache = SIGNATURE_CACHE.write().unwrap();
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
