//! Symbol type for interned strings.
//!
//! Symbols are lightweight identifiers that represent interned strings.
//! Each unique string is assigned a unique 32-bit ID, allowing efficient
//! comparison and storage.
//!
//! # Examples
//!
//! ```
//! use oxidex_mem::Symbol;
//!
//! let sym1 = Symbol::new(42);
//! let sym2 = Symbol::new(42);
//! let sym3 = Symbol::new(100);
//!
//! assert_eq!(sym1, sym2);  // Same ID = equal
//! assert_ne!(sym1, sym3);  // Different ID = not equal
//! assert_eq!(sym1.as_u32(), 42);
//! ```

use std::fmt;

/// A symbol representing an interned string.
///
/// Symbols are 32-bit identifiers that point to interned string data.
/// They provide:
/// - O(1) equality comparison (just compare u32 IDs)
/// - Minimal memory footprint (4 bytes vs heap allocation)
/// - Cache-friendly storage
/// - Type safety through newtype wrapper
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Symbol(u32);

impl Symbol {
    /// Creates a new symbol from a raw ID.
    ///
    /// # Examples
    ///
    /// ```
    /// use oxidex_mem::Symbol;
    ///
    /// let sym = Symbol::new(42);
    /// assert_eq!(sym.as_u32(), 42);
    /// ```
    #[must_use]
    pub const fn new(id: u32) -> Self {
        Self(id)
    }

    /// Returns the raw ID value.
    ///
    /// # Examples
    ///
    /// ```
    /// use oxidex_mem::Symbol;
    ///
    /// let sym = Symbol::new(42);
    /// assert_eq!(sym.as_u32(), 42);
    /// ```
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self.0
    }

    /// Returns the raw ID value as usize.
    ///
    /// # Examples
    ///
    /// ```
    /// use oxidex_mem::Symbol;
    ///
    /// let sym = Symbol::new(42);
    /// assert_eq!(sym.as_usize(), 42);
    /// ```
    #[must_use]
    pub const fn as_usize(self) -> usize {
        self.0 as usize
    }

    /// Returns true if this is the invalid/placeholder symbol.
    ///
    /// The invalid symbol uses the maximum u32 value (u32::MAX) as its ID.
    ///
    /// # Examples
    ///
    /// ```
    /// use oxidex_mem::Symbol;
    ///
    /// let valid = Symbol::new(42);
    /// let invalid = Symbol::invalid();
    ///
    /// assert!(!valid.is_invalid());
    /// assert!(invalid.is_invalid());
    /// ```
    #[must_use]
    pub const fn is_invalid(self) -> bool {
        self.0 == u32::MAX
    }

    /// The invalid/placeholder symbol.
    ///
    /// This symbol uses u32::MAX as its ID and can be used as a sentinel value.
    ///
    /// # Examples
    ///
    /// ```
    /// use oxidex_mem::Symbol;
    ///
    /// let sym = Symbol::invalid();
    /// assert!(sym.is_invalid());
    /// assert_eq!(sym.as_u32(), u32::MAX);
    /// ```
    #[must_use]
    pub const fn invalid() -> Self {
        Self(u32::MAX)
    }
}

impl fmt::Display for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Symbol({})", self.0)
    }
}

impl From<u32> for Symbol {
    fn from(id: u32) -> Self {
        Self(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symbol_creation() {
        let sym = Symbol::new(42);
        assert_eq!(sym.as_u32(), 42);
        assert_eq!(sym.as_usize(), 42);
    }

    #[test]
    fn test_symbol_equality() {
        let sym1 = Symbol::new(1);
        let sym2 = Symbol::new(1);
        let sym3 = Symbol::new(2);
        assert_eq!(sym1, sym2);
        assert_ne!(sym1, sym3);
    }

    #[test]
    fn test_symbol_ord() {
        let sym1 = Symbol::new(1);
        let sym2 = Symbol::new(2);
        assert!(sym1 < sym2);
        assert!(sym2 > sym1);
    }

    #[test]
    fn test_symbol_invalid() {
        let valid = Symbol::new(42);
        let invalid = Symbol::invalid();

        assert!(!valid.is_invalid());
        assert!(invalid.is_invalid());
        assert_eq!(invalid.as_u32(), u32::MAX);
    }

    #[test]
    fn test_symbol_from_u32() {
        let sym = Symbol::from(42);
        assert_eq!(sym.as_u32(), 42);
    }

    #[test]
    fn test_symbol_display() {
        let sym = Symbol::new(42);
        assert_eq!(format!("{}", sym), "Symbol(42)");
    }

    #[test]
    fn test_symbol_copy() {
        let sym1 = Symbol::new(42);
        let sym2 = sym1; // Copy, not move
        assert_eq!(sym1, sym2);
        assert_eq!(sym1.as_u32(), 42);
        assert_eq!(sym2.as_u32(), 42);
    }

    #[test]
    fn test_symbol_hash() {
        use std::collections::HashMap;
        let mut map = HashMap::new();

        let sym1 = Symbol::new(1);
        let sym2 = Symbol::new(1);
        let sym3 = Symbol::new(2);

        map.insert(sym1, "first");
        map.insert(sym3, "second");

        // sym1 and sym2 are the same, so this should overwrite
        map.insert(sym2, "updated");

        assert_eq!(map.len(), 2);
        assert_eq!(map.get(&Symbol::new(1)), Some(&"updated"));
        assert_eq!(map.get(&Symbol::new(2)), Some(&"second"));
    }
}
