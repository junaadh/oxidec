//! OxideX memory management infrastructure.
//!
//! This crate provides efficient memory management primitives for the OxideX
//! compiler and runtime, including:
//!
//! - **Arena allocators**: Fast, bump-pointer allocation (feature-gated)
//! - **String interning**: Deduplicated string storage with ID-based references
//!   (requires `string-interner` feature)
//!
//! # Design Goals
//!
//! 1. **Performance**: Sub-30ns allocation, cache-friendly layout
//! 2. **Safety**: Stacked Borrows compliant, provenance-safe pointer arithmetic
//! 3. **Ergonomics**: Simple API, minimal boilerplate
//! 4. **Modularity**: Features allow using only what you need
//! 5. **Simplicity**: Single arena implementation for entire project
//!
//! # Features
//!
//! ## Arena Features
//!
//! - `global-arena` - Thread-safe arena for runtime metadata (oxidec)
//! - `local-arena` - Thread-local arena for compiler frontend (oxidex-syntax)
//! - `arena-factory` - Factory for creating arenas
//! - `runtime` - Full runtime feature set = ["global-arena", "arena-factory"]
//!
//! ## String Interning Features
//!
//! - `symbols` - Enable Symbol type (still no interner)
//! - `string-interner` - Enable full string interning system = ["local-arena", "arena-factory", "symbols"]
//!
//! # Feature Usage
//!
//! ## Runtime (oxidec)
//!
//! The runtime needs thread-safe arena allocators for global metadata:
//!
//! ```toml
//! [dependencies]
//! oxidex-mem = { path = "crates/oxidex-mem", features = ["runtime"] }
//! ```
//!
//! ## Compiler Frontend (oxidex-syntax)
//!
//! The compiler frontend needs string interning:
//!
//! ```toml
//! [dependencies]
//! oxidex-mem = { path = "crates/oxidex-mem", features = ["string-interner"] }
//! ```

// Public modules
pub mod arena; // Always available, contents are feature-gated
pub mod factory; // Always available, contents are feature-gated

// String interning is feature-gated
#[cfg(feature = "string-interner")]
pub mod interner;

#[cfg(feature = "symbols")]
pub mod symbol;

// Re-exports (feature-gated)
#[cfg(any(feature = "global-arena", feature = "local-arena"))]
pub use arena::{ArenaAllocError, ArenaStats};

#[cfg(feature = "global-arena")]
pub use arena::{GlobalArena, global_arena};

#[cfg(feature = "local-arena")]
pub use arena::LocalArena;

#[cfg(feature = "arena-factory")]
pub use factory::ArenaFactory;

// String interning re-exports (feature-gated)
#[cfg(feature = "string-interner")]
pub use interner::StringInterner;

#[cfg(feature = "symbols")]
pub use symbol::Symbol;


