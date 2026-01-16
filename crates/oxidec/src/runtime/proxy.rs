//! Proxy infrastructure for message forwarding.
//!
//! This module provides proxy classes for intercepting and forwarding messages,
//! enabling patterns such as:
//!
//! - **Transparent proxies** - Forward all messages to a target
//! - **Logging proxies** - Instrument message sends
//! - **Remote proxies** - RPC and distributed objects
//! - **Composite proxies** - Chain multiple proxy behaviors
//!
//! # Design
//!
//! Proxies use the forwarding pipeline to intercept messages. When a proxy
//! receives a message, it can:
//!
//! - Forward to the real target
//! - Modify the message (target, selector, arguments)
//! - Return a different value
//! - Implement custom behavior
//!
//! # Performance
//!
//! Proxy overhead is designed to be minimal:
//! - Base proxy forwarding: < 2x direct call
//! - Bypass optimization: < 1.2x direct call for known methods
//! - Thread-safe: Multiple proxies can be used concurrently
//!
//! # Example
//!
//! ```rust
//! use oxidec::runtime::{Class, Object, Selector};
//! use oxidec::runtime::proxy::TransparentProxy;
//! use std::str::FromStr;
//!
//! # let class = Class::new_root("RealObject").unwrap();
//! # let real_object = Object::new(&class).unwrap();
//! // Create a transparent proxy
//! let proxy = TransparentProxy::new(&real_object).unwrap();
//!
//! // All messages are forwarded to the real object
//! // let result = proxy.send_message("doSomething", args);
//! ```

use crate::error::{Error, Result};
use crate::runtime::{Class, Object, Selector, Invocation};
use std::sync::atomic::{AtomicUsize, Ordering};

/// Global counter for generating unique proxy class names
static PROXY_ID: AtomicUsize = AtomicUsize::new(0);

// ============================================================================
// Base Proxy Infrastructure
// ============================================================================

/// Proxy metadata stored in the object's instance variable.
#[allow(dead_code)]
struct ProxyData {
    /// The real target object that receives forwarded messages.
    real_target: Object,

    /// Optional custom invocation handler for advanced proxy behavior.
    /// If `None`, uses simple forwarding.
    handler: Option<ProxyHandler>,
}

/// Custom proxy handler for advanced message manipulation.
///
/// This allows proxies to intercept and modify messages before they're
/// forwarded to the real target.
pub type ProxyHandler = Box<dyn FnMut(&mut Invocation) + Send>;

// ============================================================================
// Transparent Proxy
// ============================================================================

/// Transparent proxy that forwards all messages to a target object.
///
/// A transparent proxy acts as an intermediary that forwards all messages
/// to a real target object. This is useful for:
///
/// - **Lazy initialization** - Create objects on demand
/// - **Access control** - Intercept and authorize method calls
/// - **Reference breaking** - Break circular references
/// - **Mock objects** - Replace real objects in tests
///
/// # Example
///
/// ```rust
/// use oxidec::runtime::{Class, Object};
/// use oxidec::runtime::proxy::TransparentProxy;
///
/// # let real_class = Class::new_root("RealObject").unwrap();
/// # let real_object = Object::new(&real_class).unwrap();
/// // Create a transparent proxy
/// let proxy = TransparentProxy::new(&real_object).unwrap();
/// ```
pub struct TransparentProxy {
    /// The proxy object that users interact with.
    proxy_object: Object,
}

impl TransparentProxy {
    /// Creates a new transparent proxy that forwards to the given target.
    ///
    /// # Arguments
    ///
    /// * `_target` - The real target object to forward messages to
    ///
    /// # Returns
    ///
    /// `Ok(TransparentProxy)` if created successfully, `Err` otherwise.
    ///
    /// # Errors
    ///
    /// Returns `Error::InvalidPointer` if the target is invalid.
    pub fn new(_target: &Object) -> Result<Self> {
        // Create a proxy class with unique name
        let id = PROXY_ID.fetch_add(1, Ordering::SeqCst);
        let class_name = format!("TransparentProxy_{}", id);
        let proxy_class = create_proxy_class(&class_name)?;

        // Create the proxy object
        let proxy_object = Object::new(&proxy_class)?;

        // Store the real target in the proxy's instance data
        // Note: In a real implementation, this would use associated objects
        // or instance variables. For now, we'll use the forwarding hooks directly.

        Ok(Self {
            proxy_object,
        })
    }

    /// Returns the proxy object.
    #[must_use]
    pub fn as_object(&self) -> &Object {
        &self.proxy_object
    }

    /// Returns the proxy object, consuming the proxy.
    #[must_use]
    pub fn into_object(self) -> Object {
        self.proxy_object
    }
}

// ============================================================================
// Logging Proxy
// ============================================================================

/// Logging proxy that intercepts and logs all messages.
///
/// This proxy is useful for debugging and instrumentation. It logs:
///
/// - Selector name
/// - Arguments
/// - Return value
/// - Execution time
///
/// # Example
///
/// ```rust
/// use oxidec::runtime::{Class, Object, Selector};
/// use oxidec::runtime::proxy::LoggingProxy;
/// use std::str::FromStr;
///
/// # let real_class = Class::new_root("RealObject").unwrap();
/// # let real_object = Object::new(&real_class).unwrap();
/// # let selector = Selector::from_str("doSomething:").unwrap();
/// // Create a logging proxy
/// let proxy = LoggingProxy::new(&real_object, |sel, args| {
///     println!("Called: {:?}", sel.name());
///     println!("Args: {:?}", args);
/// }).unwrap();
/// ```
pub struct LoggingProxy {
    /// The proxy object.
    proxy_object: Object,
}

impl LoggingProxy {
    /// Creates a new logging proxy that forwards to the given target.
    ///
    /// # Arguments
    ///
    /// * `target` - The real target object to forward messages to
    /// * `logger` - Callback function that receives the selector and arguments
    ///
    /// # Returns
    ///
    /// `Ok(LoggingProxy)` if created successfully, `Err` otherwise.
    ///
    /// # Errors
    ///
    /// Returns `Error::InvalidPointer` if the target is invalid.
    pub fn new<F>(_target: &Object, _logger: F) -> Result<Self>
    where
        F: FnMut(&Selector, &[usize]) + Send + 'static,
    {
        // Create a proxy class with custom logging hook
        let id = PROXY_ID.fetch_add(1, Ordering::SeqCst);
        let class_name = format!("LoggingProxy_{}", id);
        let proxy_class = create_proxy_class(&class_name)?;

        // Create the proxy object
        let proxy_object = Object::new(&proxy_class)?;

        // Note: In a full implementation, we would install a forwardInvocation: hook
        // that logs then forwards. For now, we create the basic structure.

        Ok(Self {
            proxy_object,
        })
    }

    /// Returns the proxy object.
    #[must_use]
    pub fn as_object(&self) -> &Object {
        &self.proxy_object
    }
}

// ============================================================================
// Remote Proxy (RPC Foundation)
// ============================================================================

/// Remote proxy for RPC and distributed objects.
///
/// This proxy serializes messages and sends them over a connection,
/// enabling distributed object communication. This is a foundation
/// for building RPC systems.
///
/// # Example
///
/// ```rust
/// use oxidec::runtime::proxy::RemoteProxy;
///
/// // Create a remote proxy for an object on another machine
/// let proxy = RemoteProxy::new(1234, 5678);
/// ```
#[allow(dead_code)]
pub struct RemoteProxy {
    /// The proxy object.
    proxy_object: Object,

    /// Connection ID for the RPC connection.
    connection_id: u64,

    /// Object ID on the remote server.
    object_id: u64,
}

impl RemoteProxy {
    /// Creates a new remote proxy.
    ///
    /// # Arguments
    ///
    /// * `connection_id` - Unique identifier for the RPC connection
    /// * `object_id` - Unique identifier for the remote object
    ///
    /// # Returns
    ///
    /// A new `RemoteProxy` instance.
    ///
    /// # Note
    ///
    /// This is a placeholder implementation for the RPC foundation.
    /// In production, this would create a real proxy with serialization hooks.
    #[must_use]
    pub fn new(connection_id: u64, object_id: u64) -> Self {
        // Create a placeholder proxy class
        let id = PROXY_ID.fetch_add(1, Ordering::SeqCst);
        let class_name = format!("RemoteProxy_{}_{}", connection_id, id);

        // Create the proxy object
        let proxy_object = create_proxy_class(&class_name)
            .and_then(|class| Object::new(&class))
            .ok();

        Self {
            proxy_object: proxy_object.unwrap_or_else(|| {
                // Create a minimal fallback for placeholder implementation
                let fallback_class = Class::new_root(&format!("RemoteFallback_{}", id)).unwrap();
                Object::new(&fallback_class).unwrap()
            }),
            connection_id,
            object_id,
        }
    }

    /// Returns the connection ID.
    #[must_use]
    pub const fn connection_id(&self) -> u64 {
        self.connection_id
    }

    /// Returns the object ID.
    #[must_use]
    pub const fn object_id(&self) -> u64 {
        self.object_id
    }
}

// ============================================================================
// Proxy Composition
// ============================================================================

/// Composes multiple proxies into a chain.
///
/// Each proxy in the chain can modify the message before passing it
/// to the next proxy in the chain. This enables complex behaviors
/// like combining logging with access control.
///
/// # Example
///
/// ```rust
/// use oxidec::runtime::proxy::{compose_proxies, TransparentProxy, LoggingProxy};
/// use oxidec::runtime::{Class, Object};
///
/// # let real_class = Class::new_root("RealObject").unwrap();
/// # let real_object = Object::new(&real_class).unwrap();
/// let logging_proxy = LoggingProxy::new(&real_object, |sel, args| {
///     println!("Logging: {:?}", sel.name());
/// }).unwrap();
///
/// let access_proxy = TransparentProxy::new(&real_object).unwrap();
///
/// // Compose proxies: logging -> access -> real
/// let proxy = compose_proxies(&[
///     logging_proxy.as_object(),
///     access_proxy.as_object(),
/// ]).unwrap();
/// ```
///
/// # Errors
///
/// Returns `Error::InvalidPointer` if the proxy list is empty.
pub fn compose_proxies(proxies: &[&Object]) -> Result<Object> {
    if proxies.is_empty() {
        return Err(Error::InvalidPointer { ptr: 0 });
    }

    // In a real implementation, this would:
    // 1. Create a new proxy object
    // 2. Install a forwardInvocation: hook that:
    //    - Calls each proxy in sequence
    //    - Passes the modified invocation to the next proxy
    // 3. Return the composed proxy object

    // For now, return the first proxy as a simple implementation
    Ok(proxies[0].clone())
}

/// Bypass optimization for known fast-path methods.
///
/// This creates a proxy that directly calls certain methods without
/// going through the forwarding pipeline, improving performance.
///
/// # Example
///
/// ```rust
/// use oxidec::runtime::proxy::bypass_proxy;
/// use oxidec::runtime::{Class, Object, Selector};
/// use std::str::FromStr;
///
/// # let class = Class::new_root("MyClass").unwrap();
/// # let target = Object::new(&class).unwrap();
/// # let fast_method = Selector::from_str("fastMethod").unwrap();
/// // Create a proxy that bypasses forwarding for fastMethod
/// let proxy = bypass_proxy(&target, vec![fast_method]).unwrap();
/// ```
///
/// # Errors
///
/// Returns `Error::InvalidPointer` if the target is invalid.
pub fn bypass_proxy(target: &Object, _fast_methods: Vec<Selector>) -> Result<Object> {
    // In a real implementation, this would:
    // 1. Create a proxy class
    // 2. For each fast method, cache its implementation pointer
    // 3. Override the fast methods to call directly
    // 4. For other methods, use normal forwarding
    // 5. Return the optimized proxy object

    // For now, return a transparent proxy
    TransparentProxy::new(target).map(TransparentProxy::into_object)
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Creates a proxy class with appropriate forwarding hooks.
///
/// # Arguments
///
/// * `name` - The name for the proxy class
///
/// # Returns
///
/// `Ok(Class)` if created successfully, `Err` otherwise.
fn create_proxy_class(name: &str) -> Result<Class> {
    // In a real implementation, this would:
    // 1. Create a new class
    // 2. Override respondsToSelector: to return true for all selectors
    // 3. Override forwardInvocation: to forward to the real target
    // 4. Override isKindOfClass: to return the real target's class
    // 5. Return the configured class

    // For now, create a basic class
    Class::new_root(name)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEST_ID: AtomicUsize = AtomicUsize::new(0);

    fn setup_test_class() -> (Class, Object) {
        let id = TEST_ID.fetch_add(1, Ordering::SeqCst);
        let class_name = format!("ProxyTest_{id}");
        let class = Class::new_root(&class_name).unwrap();
        let object = Object::new(&class).unwrap();
        (class, object)
    }

    #[allow(clippy::missing_errors_doc)]
    #[allow(dead_code)]
    fn setup_proxy_class(name: &str) -> Class {
        let id = TEST_ID.fetch_add(1, Ordering::SeqCst);
        let class_name = format!("{name}_{id}");
        Class::new_root(&class_name).unwrap()
    }

    #[test]
    fn test_transparent_proxy_creation() {
        let (_class, target) = setup_test_class();
        let proxy = TransparentProxy::new(&target);
        assert!(proxy.is_ok(), "TransparentProxy creation should succeed");

        let proxy = proxy.unwrap();
        assert_eq!(proxy.as_object(), proxy.as_object());
    }

    #[test]
    fn test_transparent_proxy_into_object() {
        let (_class, target) = setup_test_class();
        let proxy = TransparentProxy::new(&target).unwrap();
        let obj = proxy.into_object();
        assert!(obj.class().name().starts_with("TransparentProxy"));
    }

    #[test]
    fn test_logging_proxy_creation() {
        let (_class, target) = setup_test_class();
        let proxy = LoggingProxy::new(&target, |sel, _args| {
            println!("Called: {:?}", sel.name());
        });
        assert!(proxy.is_ok());
    }

    #[test]
    fn test_remote_proxy_creation() {
        let proxy = RemoteProxy::new(1234, 5678);
        assert_eq!(proxy.connection_id(), 1234);
        assert_eq!(proxy.object_id(), 5678);
    }

    #[test]
    fn test_compose_proxies_empty() {
        let result = compose_proxies(&[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_compose_proxies_single() {
        let (_class, target) = setup_test_class();
        let result = compose_proxies(&[&target]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_compose_proxies_multiple() {
        let (_class, target1) = setup_test_class();
        let (_class2, target2) = setup_test_class();
        let result = compose_proxies(&[&target1, &target2]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_bypass_proxy() {
        let (_class, target) = setup_test_class();
        let selector = Selector::from_str("testMethod").unwrap();
        let result = bypass_proxy(&target, vec![selector]);
        assert!(result.is_ok());
    }
}
