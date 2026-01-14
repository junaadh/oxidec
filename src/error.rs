//! Error types for `OxideC` runtime.
//!
//! This module defines the error types used throughout the `OxideC` runtime,
//! including arena allocation failures, reference counting errors, and
//! general runtime errors.

use std::fmt;

/// Errors that can occur in the `OxideC` runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// Arena allocation failed due to insufficient memory.
    OutOfMemory,

    /// Arena is full and cannot allocate requested size.
    ArenaFull {
        /// The requested allocation size.
        requested: usize,
        /// The available space in the arena.
        available: usize,
    },

    /// Chunk allocation failed with the given size.
    ChunkAllocationFailed {
        /// The requested chunk size.
        size: usize,
    },

    /// Invalid alignment specified.
    InvalidAlignment {
        /// The requested alignment.
        alignment: usize,
    },

    /// Reference count overflow detected.
    RefCountOverflow,

    /// Invalid pointer provided.
    InvalidPointer {
        /// The pointer value.
        ptr: usize,
    },

    /// Invalid arena state detected.
    InvalidArenaState,

    /// Class name already exists in registry.
    ClassAlreadyExists,

    /// Inheritance cycle detected.
    InheritanceCycle,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::OutOfMemory => write!(f, "Out of memory"),
            Error::ArenaFull { requested, available } => {
                write!(f, "Arena full: requested {requested} bytes, available {available} bytes")
            }
            Error::ChunkAllocationFailed { size } => {
                write!(f, "Failed to allocate chunk of size {size} bytes")
            }
            Error::InvalidAlignment { alignment } => {
                write!(f, "Invalid alignment: {alignment} is not a power of two")
            }
            Error::RefCountOverflow => write!(f, "Reference count overflow detected"),
            Error::InvalidPointer { ptr } => write!(f, "Invalid pointer: {ptr:#x}"),
            Error::InvalidArenaState => write!(f, "Invalid arena state"),
            Error::ClassAlreadyExists => write!(f, "Class name already exists in registry"),
            Error::InheritanceCycle => write!(f, "Inheritance cycle detected"),
        }
    }
}

impl std::error::Error for Error {}

/// Result type for OxideC runtime operations.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        assert_eq!(format!("{}", Error::OutOfMemory), "Out of memory");
        assert_eq!(
            format!("{}", Error::ArenaFull { requested: 100, available: 50 }),
            "Arena full: requested 100 bytes, available 50 bytes"
        );
    }

    #[test]
    fn test_error_equality() {
        assert_eq!(Error::OutOfMemory, Error::OutOfMemory);
        assert_ne!(
            Error::ArenaFull { requested: 100, available: 50 },
            Error::ArenaFull { requested: 200, available: 50 }
        );
    }
}
