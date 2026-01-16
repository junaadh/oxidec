//! `OxideC` runtime module.
//!
//! This module provides the core runtime infrastructure for `OxideC`, including:
//!
//! - `Arena` allocation for long-lived metadata
//! - Thread-safe and thread-local allocators
//! - Runtime initialization and global state
//!
//! # Architecture
//!
//! The runtime is organized into several modules:
//!
//! - [`arena`]: Arena allocators for high-performance memory allocation
//! - [`string`]: Runtime string type with SSO and COW (✓ Implemented)
//! - [`selector`]: Selector interning and caching (✓ Implemented)
//! - [`class`]: Class creation, inheritance, and method registry (✓ Implemented)
//! - [`object`]: Object allocation and reference counting (✓ Implemented)
//! - `dispatch`: Message dispatch system (Phase 2 - TODO)
//! - `cache`: Method caching (Phase 2 - TODO)
//! - `protocol`: Protocol conformance checking (Phase 3 - TODO)
//!
//! # Global Arena
//!
//! The runtime maintains a global arena for allocating long-lived metadata
//! such as selectors, classes, and protocols. This arena is initialized once
//! and lives for the entire program duration.
//!
//! # Example
//!
//! ```rust
//! use oxidec::runtime::get_global_arena;
//!
//! let arena = get_global_arena();
//! let value: *mut u32 = arena.alloc(42);
//! ```

// Arena module removed - now using oxidex-mem
// pub mod arena;
pub mod category;
pub mod class;
pub mod dispatch;
pub mod encoding;
pub mod forwarding;
pub mod introspection;
pub mod invocation;
pub mod message;
pub mod object;
pub mod pool;
pub mod protocol;
pub mod proxy;
pub mod selector;
pub mod string;

// Re-export arena types from oxidex-mem for backward compatibility
pub use oxidex_mem::{GlobalArena as Arena, global_arena as get_global_arena};
pub use category::Category;
pub use class::{Class, Method};
pub use invocation::Invocation;
pub use message::MessageArgs;
pub use object::{Object, ObjectPtr};
pub use pool::{PoolStats, PooledInvocation};
pub use protocol::Protocol;
pub use proxy::{
    LoggingProxy, RemoteProxy, TransparentProxy, bypass_proxy, compose_proxies,
};
pub use selector::Selector;
pub use string::RuntimeString;

// Re-export commonly used introspection APIs
pub use introspection::{
    ClassBuilder, adopted_protocols, all_classes, all_protocols,
    allocate_class, class_from_name, class_hierarchy, class_methods,
    conforms_to, has_method, instance_methods, is_subclass, method_provider,
    object_get_class, object_is_instance, object_responds_to, subclasses,
};

// Note: Global arena and get_global_arena are now provided by oxidex-mem
// and re-exported above for backward compatibility.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_global_arena_initialization() {
        let arena1 = get_global_arena();
        let arena2 = get_global_arena();

        // Should return the same instance
        assert!(std::ptr::eq(arena1, arena2));
    }

    #[test]
    fn test_global_arena_allocation() {
        let arena = get_global_arena();

        let ptr: *mut u32 = arena.alloc(42);
        unsafe {
            assert_eq!(*ptr, 42);
        }
    }

    #[test]
    fn test_global_arena_multiple_allocations() {
        let arena = get_global_arena();

        let ptr1: *mut u32 = arena.alloc(1);
        let ptr2: *mut u64 = arena.alloc(2);
        let ptr3: *mut u32 = arena.alloc(3);

        unsafe {
            assert_eq!(*ptr1, 1);
            assert_eq!(*ptr2, 2);
            assert_eq!(*ptr3, 3);
        }
    }
}
