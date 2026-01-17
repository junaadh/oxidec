//! `OxideX` memory management infrastructure
//!
//! This crate provides efficient memory management primitives for the `OxideX`
//! compiler and runtime, including:
//!
//! - **Arena allocators**: Fast, bump-pointer allocation (feature-gated)
//! - **String interning**: Deduplicated string storage with ID-based references
//!   (requires `string-interner` feature)
//!

pub mod arena;
