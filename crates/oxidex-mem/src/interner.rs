//! String interning with ID-based references.
//!
//! This module provides a high-performance string interning system that
//! assigns unique IDs to strings, enabling efficient comparison and storage.
//!
//! # Design
//!
//! The interner maintains two data structures:
//! - `strings`: Maps Symbol → &str (for resolving IDs to strings)
//! - `symbols`: Maps &str → Symbol (for interning strings)
//!
//! All string data is stored in an arena for efficient lifetime management
//! and cache-friendly access patterns.
//!
//! # Examples
//!
//! ```
//! use oxidex_mem::StringInterner;
//!
//! let mut interner = StringInterner::new();
//!
//! // Intern some strings
//! let sym1 = interner.intern("myVariable");
//! let sym2 = interner.intern("myVariable");  // Returns same ID
//! let sym3 = interner.intern("otherVariable");
//!
//! assert_eq!(sym1, sym2);  // Same string = same ID
//! assert_ne!(sym1, sym3);  // Different string = different ID
//!
//! // Resolve back to string when needed
//! assert_eq!(interner.resolve(sym1), Some("myVariable"));
//! ```
//!
//! # Performance
//!
//! - **Interned string**: O(1) hash lookup + O(1) arena allocation (amortized)
//! - **New string**: O(n) to copy + O(1) hash insert
//! - **Resolve**: O(1) array indexing

use crate::arena::LocalArena;
use crate::symbol::Symbol;

// Use hashbrown if available (faster), otherwise std::collections::HashMap
// The "symbols" feature enables hashbrown dependency
#[cfg(feature = "symbols")]
use hashbrown::HashMap;

#[cfg(not(feature = "symbols"))]
use std::collections::HashMap;

/// String interner with bidirectional mapping.
///
/// The interner maintains two data structures:
/// - `strings`: Maps Symbol ID to string slice
/// - `symbols`: Maps &str to Symbol ID
///
/// All string data is stored in an arena for efficient lifetime management
/// and cache-friendly access patterns.
///
/// # Examples
///
/// ```
/// use oxidex_mem::StringInterner;
///
/// let mut interner = StringInterner::new();
///
/// // Intern an identifier
/// let sym = interner.intern("myVariable");
/// assert_eq!(interner.resolve(sym), Some("myVariable"));
///
/// // Re-interning returns the same ID
/// let sym2 = interner.intern("myVariable");
/// assert_eq!(sym, sym2);
/// ```
pub struct StringInterner {
    /// Arena for storing string data
    arena: LocalArena,

    /// Map from Symbol ID to string slice
    strings: Vec<&'static str>,

    /// Map from string to Symbol ID
    symbols: HashMap<String, Symbol>,

    /// Next available ID
    next_id: u32,
}

impl StringInterner {
    /// Creates a new empty interner.
    ///
    /// # Examples
    ///
    /// ```
    /// use oxidex_mem::StringInterner;
    ///
    /// let mut interner = StringInterner::new();
    /// assert_eq!(interner.len(), 0);
    ///
    /// let sym = interner.intern("myVariable");
    /// assert_eq!(interner.len(), 1);
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {
            arena: LocalArena::new(8192), // 8 KiB initial chunk
            strings: Vec::new(),
            symbols: HashMap::new(),
            next_id: 0,
        }
    }

    /// Creates a new interner with pre-interned strings.
    ///
    /// This is useful for pre-interning keywords or other frequently-used strings
    /// to ensure they have consistent IDs (starting from 0).
    ///
    /// # Arguments
    ///
    /// * `strings` - Slice of strings to pre-intern in order
    ///
    /// # Examples
    ///
    /// ```
    /// use oxidex_mem::StringInterner;
    ///
    /// let keywords = &["let", "mut", "fn"];
    /// let mut interner = StringInterner::with_pre_interned(keywords);
    ///
    /// // All keywords are pre-interned
    /// assert_eq!(interner.len(), 3);
    ///
    /// // Keywords have consistent IDs
    /// let let_sym = interner.intern("let");
    /// assert_eq!(let_sym.as_u32(), 0);
    /// ```
    #[must_use]
    pub fn with_pre_interned(strings: &[&str]) -> Self {
        let mut interner = Self {
            arena: LocalArena::new(8192),
            strings: Vec::new(),
            symbols: HashMap::new(),
            next_id: 0,
        };

        // Pre-intern all strings (in order for consistent IDs)
        for s in strings {
            interner.intern_pre_allocated(s);
        }

        interner
    }

    /// Interns a string without checking if it already exists.
    ///
    /// Used only during initialization with pre-allocated strings.
    /// Assumes the string doesn't already exist.
    fn intern_pre_allocated(&mut self, s: &str) -> Symbol {
        let id = self.next_id;
        self.next_id += 1;

        // Allocate string in arena
        let ptr = self.arena.alloc_str(s);

        // SAFETY: Creating a &'static str from arena-allocated memory:
        // - ptr points to valid, null-terminated UTF-8 data (from alloc_str)
        // - Data is allocated from arena and lives for program duration
        // - Arena never deallocates individual allocations
        // - Length calculation is correct (s.len() is the string length)
        let string_ref: &'static str = unsafe {
            let slice = std::slice::from_raw_parts(ptr, s.len());
            std::str::from_utf8_unchecked(slice)
        };

        self.strings.push(string_ref);
        let sym = Symbol::new(id);
        self.symbols.insert(s.to_string(), sym);

        sym
    }

    /// Interns a string, returning its Symbol.
    ///
    /// If the string has already been interned, returns the existing Symbol.
    /// Otherwise, allocates the string in the arena and returns a new Symbol.
    ///
    /// # Performance
    ///
    /// - **Interned string**: O(1) hash lookup
    /// - **New string**: O(n) to copy + O(1) hash insert
    ///
    /// # Examples
    ///
    /// ```
    /// use oxidex_mem::StringInterner;
    ///
    /// let mut interner = StringInterner::new();
    ///
    /// // First interning allocates and returns a new ID
    /// let sym1 = interner.intern("myVariable");
    ///
    /// // Second interning returns the same ID (no allocation)
    /// let sym2 = interner.intern("myVariable");
    ///
    /// assert_eq!(sym1, sym2);
    /// ```
    pub fn intern(&mut self, s: &str) -> Symbol {
        // Try fast path: hash lookup
        if let Some(&sym) = self.symbols.get(s) {
            return sym;
        }

        // Slow path: allocate new string
        let id = self.next_id;
        self.next_id += 1;

        // Allocate string in arena
        let ptr = self.arena.alloc_str(s);

        // SAFETY: Creating a &'static str from arena-allocated memory:
        // - ptr points to valid, null-terminated UTF-8 data (from alloc_str)
        // - Data is allocated from arena and lives for program duration
        // - Arena never deallocates individual allocations
        // - Length calculation is correct (s.len() is the string length)
        let string_ref: &'static str = unsafe {
            let slice = std::slice::from_raw_parts(ptr, s.len());
            std::str::from_utf8_unchecked(slice)
        };

        self.strings.push(string_ref);
        let sym = Symbol::new(id);
        self.symbols.insert(s.to_string(), sym);

        sym
    }

    /// Resolves a Symbol to its string slice.
    ///
    /// Returns `None` if the Symbol is invalid (out of range).
    ///
    /// # Examples
    ///
    /// ```
    /// use oxidex_mem::{StringInterner, Symbol};
    ///
    /// let mut interner = StringInterner::new();
    ///
    /// let sym = interner.intern("testIdentifier");
    /// assert_eq!(interner.resolve(sym), Some("testIdentifier"));
    ///
    /// let invalid = Symbol::new(9999);
    /// assert_eq!(interner.resolve(invalid), None);
    /// ```
    #[must_use]
    pub fn resolve(&self, sym: Symbol) -> Option<&str> {
        let id = sym.as_usize();
        self.strings.get(id).copied()
    }

    /// Returns the number of interned strings.
    ///
    /// # Examples
    ///
    /// ```
    /// use oxidex_mem::StringInterner;
    ///
    /// let mut interner = StringInterner::new();
    /// assert_eq!(interner.len(), 0);
    ///
    /// interner.intern("myVariable");
    /// assert_eq!(interner.len(), 1);
    /// ```
    #[must_use]
    pub fn len(&self) -> usize {
        self.strings.len()
    }

    /// Returns true if no strings are interned.
    ///
    /// # Examples
    ///
    /// ```
    /// use oxidex_mem::StringInterner;
    ///
    /// let interner = StringInterner::new();
    /// assert!(interner.is_empty());
    /// ```
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.strings.is_empty()
    }

    /// Returns the Symbol for a string if it has been interned.
    ///
    /// # Examples
    ///
    /// ```
    /// use oxidex_mem::StringInterner;
    ///
    /// let mut interner = StringInterner::new();
    ///
    /// interner.intern("let");
    /// let let_sym = interner.get_symbol("let").unwrap();
    /// assert_eq!(let_sym.as_u32(), 0);
    ///
    /// assert!(interner.get_symbol("notInterner").is_none());
    /// ```
    #[must_use]
    pub fn get_symbol(&self, s: &str) -> Option<Symbol> {
        self.symbols.get(s).copied()
    }
}

impl Default for StringInterner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interner_creation() {
        let interner = StringInterner::new();
        assert_eq!(interner.len(), 0);
        assert!(interner.is_empty());
    }

    #[test]
    fn test_with_pre_interned() {
        let keywords = &["let", "mut", "fn"];
        let interner = StringInterner::with_pre_interned(keywords);
        assert_eq!(interner.len(), 3);
        assert!(!interner.is_empty());
    }

    #[test]
    fn test_identifier_interning() {
        let mut interner = StringInterner::new();

        let sym1 = interner.intern("myVariable");
        let sym2 = interner.intern("myVariable");
        let sym3 = interner.intern("otherVariable");

        assert_eq!(sym1, sym2); // Same string, same ID
        assert_ne!(sym1, sym3); // Different string, different ID
        assert_eq!(sym1.as_u32(), 0); // First identifier gets ID 0
    }

    #[test]
    fn test_symbol_resolution() {
        let mut interner = StringInterner::new();

        let sym = interner.intern("testIdentifier");
        let resolved = interner.resolve(sym);

        assert_eq!(resolved, Some("testIdentifier"));
    }

    #[test]
    fn test_many_identifiers() {
        let mut interner = StringInterner::new();

        // Intern many identifiers
        let mut syms = Vec::new();
        for i in 0..1000 {
            let ident = format!("identifier_{}", i);
            syms.push(interner.intern(&ident));
        }

        // All should be unique
        assert_eq!(interner.len(), 1000);

        // Re-interning should return same IDs
        for (i, &sym) in syms.iter().enumerate() {
            let ident = format!("identifier_{}", i);
            let sym2 = interner.intern(&ident);
            assert_eq!(sym, sym2);
        }
    }

    #[test]
    fn test_numeric_literals() {
        let mut interner = StringInterner::new();

        let num1 = interner.intern("42");
        let num2 = interner.intern("42");
        let num3 = interner.intern("123");

        assert_eq!(num1, num2);
        assert_ne!(num1, num3);

        assert_eq!(interner.resolve(num1), Some("42"));
        assert_eq!(interner.resolve(num3), Some("123"));
    }

    #[test]
    fn test_type_suffixes() {
        let mut interner = StringInterner::new();

        let u32_sym = interner.intern("u32");
        let i64_sym = interner.intern("i64");
        let f32_sym = interner.intern("f32");

        assert_eq!(interner.resolve(u32_sym), Some("u32"));
        assert_eq!(interner.resolve(i64_sym), Some("i64"));
        assert_eq!(interner.resolve(f32_sym), Some("f32"));
    }

    #[test]
    fn test_get_symbol() {
        let mut interner = StringInterner::new();

        interner.intern("let");
        let let_sym = interner.get_symbol("let").unwrap();
        assert_eq!(let_sym.as_u32(), 0);

        // Non-interned string should return None
        assert!(interner.get_symbol("notInterner").is_none());
    }

    #[test]
    fn test_string_with_special_chars() {
        let mut interner = StringInterner::new();

        let sym1 = interner.intern("myVariable_123");
        let sym2 = interner.intern("_private");
        let sym3 = interner.intern("CamelCase");

        assert_eq!(interner.resolve(sym1), Some("myVariable_123"));
        assert_eq!(interner.resolve(sym2), Some("_private"));
        assert_eq!(interner.resolve(sym3), Some("CamelCase"));
    }

    #[test]
    fn test_empty_string() {
        let mut interner = StringInterner::new();

        let sym = interner.intern("");
        assert_eq!(interner.resolve(sym), Some(""));
    }

    #[test]
    fn test_unicode_identifiers() {
        let mut interner = StringInterner::new();

        let sym1 = interner.intern("变量");
        let sym2 = interner.intern("変数");
        let sym3 = interner.intern("متغير");

        assert_eq!(interner.resolve(sym1), Some("变量"));
        assert_eq!(interner.resolve(sym2), Some("変数"));
        assert_eq!(interner.resolve(sym3), Some("متغير"));
    }

    #[test]
    fn test_pre_interned_consistency() {
        let keywords = &["let", "mut", "fn", "struct", "class"];
        let mut interner = StringInterner::with_pre_interned(keywords);

        // Pre-interned strings should have consistent IDs
        let let_sym = interner.get_symbol("let").unwrap();
        assert_eq!(let_sym.as_u32(), 0);

        let class_sym = interner.get_symbol("class").unwrap();
        assert_eq!(class_sym.as_u32(), 4);

        // Re-interning pre-interned strings returns same ID
        let let_sym2 = interner.intern("let");
        assert_eq!(let_sym, let_sym2);

        // New strings get IDs after pre-interned ones
        let new_sym = interner.intern("myVariable");
        assert_eq!(new_sym.as_u32(), 5);
    }
}
