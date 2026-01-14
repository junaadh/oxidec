//! `OxideC`: Modern Objective-C Runtime for Rust
//!
//! `OxideC` is a dynamic object runtime inspired by Objective-C, redesigned for
//! modern systems programming in Rust. It provides:
//!
//! - **Dynamic Dispatch** with late binding and method caching
//! - **Memory Safety** with manual management in unsafe internals, safe public API wrapping
//! - **C ABI Compatibility** for FFI and interoperability
//! - **Runtime Reflection** for introspection and tooling
//! - **High Performance** through arena allocation and zero-cost abstractions
//!
//! # Architecture
//!
//! `OxideC` is built on a layered architecture:
//!
//! - **Public API Layer**: Type-safe, validated abstractions
//! - **Runtime Layer**: Unsafe internals with comprehensive safety documentation
//! - **Memory Layer**: Arena allocators for high-performance allocation
//!
//! # Example
//!
//! ```rust
//! use oxidec::runtime::get_global_arena;
//!
//! let arena = get_global_arena();
//! let value: *mut u32 = arena.alloc(42);
//!
//! unsafe {
//!     assert_eq!(*value, 42);
//! }
//! ```

pub mod error;
pub mod runtime;

// Re-export commonly used types
pub use error::{Error, Result};
pub use runtime::{
    Arena, LocalArena, RuntimeString, get_global_arena,
    Selector,
    Class, Method,
    Object,
};
