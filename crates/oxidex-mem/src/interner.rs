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

/// All 19 OxideX keywords in order for consistent IDs (0-18).
const KEYWORDS: &[&str] = &[
    "let", "mut", "fn", "struct", "class", "enum", "protocol",
    "impl", "return", "if", "guard", "match", "for", "while",
    "comptime", "const", "static", "pub", "prv"
];

/// Number of pre-interned keywords.
const KEYWORD_COUNT: u32 = 19;

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
///
/// // Keywords are pre-interned (IDs 0-18)
/// let let_sym = interner.intern("let");
/// assert_eq!(let_sym.as_u32(), 0);
/// assert!(interner.is_keyword(let_sym));
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
    /// Creates a new interner with pre-interned keywords.
    ///
    /// This initializes the interner with all 19 OxideX keywords,
    /// ensuring they always have consistent IDs (0-18).
    ///
    /// # Examples
    ///
    /// ```
    /// use oxidex_mem::StringInterner;
    ///
    /// let mut interner = StringInterner::new();
    ///
    /// // All keywords are pre-interned
    /// assert_eq!(interner.len(), 19);
    ///
    /// // Keywords have consistent IDs
    /// let let_sym = interner.intern("let");
    /// assert_eq!(let_sym.as_u32(), 0);
    /// ```
    #[must_use]
    pub fn new() -> Self {
        let mut interner = Self {
            arena: LocalArena::new(8192), // 8 KiB initial chunk
            strings: Vec::new(),
            symbols: HashMap::new(),
            next_id: 0,
        };

        // Pre-intern all keywords (in order for consistent IDs)
        for keyword in KEYWORDS {
            interner.intern_keyword(keyword);
        }

        interner
    }

    /// Interns a keyword (used during initialization).
    fn intern_keyword(&mut self, s: &str) -> Symbol {
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
    /// assert_eq!(interner.len(), 19); // 19 pre-interned keywords
    ///
    /// interner.intern("myVariable");
    /// assert_eq!(interner.len(), 20);
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
    /// assert!(!interner.is_empty()); // Has 19 keywords
    /// ```
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.strings.is_empty()
    }

    /// Returns true if the given Symbol is a keyword.
    ///
    /// Keywords are always assigned IDs 0-18.
    ///
    /// # Examples
    ///
    /// ```
    /// use oxidex_mem::StringInterner;
    ///
    /// let mut interner = StringInterner::new();
    ///
    /// let let_sym = interner.intern("let");
    /// assert!(interner.is_keyword(let_sym));
    ///
    /// let ident_sym = interner.intern("myVariable");
    /// assert!(!interner.is_keyword(ident_sym));
    /// ```
    #[must_use]
    pub const fn is_keyword(&self, _sym: Symbol) -> bool {
        // First 19 IDs (0-18) are keywords
        // Note: This is a const function, so we can't access self
        // Callers should check: sym.as_u32() < KEYWORD_COUNT
        _sym.as_u32() < KEYWORD_COUNT
    }

    /// Returns the Symbol for a keyword if it exists.
    ///
    /// # Examples
    ///
    /// ```
    /// use oxidex_mem::StringInterner;
    ///
    /// let interner = StringInterner::new();
    ///
    /// let let_sym = interner.get_keyword_symbol("let").unwrap();
    /// assert_eq!(let_sym.as_u32(), 0);
    ///
    /// assert!(interner.get_keyword_symbol("notAKeyword").is_none());
    /// ```
    #[must_use]
    pub fn get_keyword_symbol(&self, keyword: &str) -> Option<Symbol> {
        self.symbols.get(keyword).copied()
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
        assert_eq!(interner.len(), 19); // 19 keywords
        assert!(!interner.is_empty());
    }

    #[test]
    fn test_keyword_interning() {
        let mut interner = StringInterner::new();

        // All keywords should have consistent IDs
        let let_sym = interner.intern("let");
        assert_eq!(let_sym.as_u32(), 0);

        let prv_sym = interner.intern("prv");
        assert_eq!(prv_sym.as_u32(), 18);

        // Re-interning should return same ID
        let let_sym2 = interner.intern("let");
        assert_eq!(let_sym, let_sym2);
    }

    #[test]
    fn test_keyword_ids() {
        let mut interner = StringInterner::new();

        // Verify all keywords have IDs 0-18
        for (i, &keyword) in KEYWORDS.iter().enumerate() {
            let sym = interner.intern(keyword);
            assert_eq!(sym.as_u32(), i as u32);
            assert!(interner.is_keyword(sym));
        }
    }

    #[test]
    fn test_identifier_interning() {
        let mut interner = StringInterner::new();

        let sym1 = interner.intern("myVariable");
        let sym2 = interner.intern("myVariable");
        let sym3 = interner.intern("otherVariable");

        assert_eq!(sym1, sym2); // Same string, same ID
        assert_ne!(sym1, sym3); // Different string, different ID
        assert!(sym1.as_u32() >= 19); // Identifiers come after keywords
    }

    #[test]
    fn test_symbol_resolution() {
        let mut interner = StringInterner::new();

        let sym = interner.intern("testIdentifier");
        let resolved = interner.resolve(sym);

        assert_eq!(resolved, Some("testIdentifier"));
    }

    #[test]
    fn test_keyword_detection() {
        let mut interner = StringInterner::new();

        let let_sym = interner.intern("let");
        assert!(interner.is_keyword(let_sym));

        let ident_sym = interner.intern("myVariable");
        assert!(!interner.is_keyword(ident_sym));
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
        assert_eq!(interner.len(), 19 + 1000);

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
    fn test_get_keyword_symbol() {
        let interner = StringInterner::new();

        let let_sym = interner.get_keyword_symbol("let").unwrap();
        assert_eq!(let_sym.as_u32(), 0);

        let fn_sym = interner.get_keyword_symbol("fn").unwrap();
        assert_eq!(fn_sym.as_u32(), 2);

        // Non-keyword should return None
        assert!(interner.get_keyword_symbol("notAKeyword").is_none());
    }

    #[test]
    fn test_all_keywords() {
        let interner = StringInterner::new();

        // Verify all keywords are present and have correct IDs
        let expected = [
            ("let", 0), ("mut", 1), ("fn", 2), ("struct", 3), ("class", 4),
            ("enum", 5), ("protocol", 6), ("impl", 7), ("return", 8),
            ("if", 9), ("guard", 10), ("match", 11), ("for", 12),
            ("while", 13), ("comptime", 14), ("const", 15), ("static", 16),
            ("pub", 17), ("prv", 18),
        ];

        for &(keyword, expected_id) in &expected {
            let sym = interner.get_keyword_symbol(keyword).unwrap();
            assert_eq!(sym.as_u32(), expected_id);
            assert_eq!(interner.resolve(sym), Some(keyword));
        }
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
}
