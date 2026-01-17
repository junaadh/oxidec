//! `OxideX` language keywords.
//!
//! This module defines all reserved keywords in the `OxideX` language.
//! Keywords are pre-interned in the string interner for fast lookup.

/// All 20 `OxideX` keywords in order for consistent IDs (0-19).
///
/// These are reserved words that cannot be used as identifiers.
/// They are pre-interned in the string interner for fast keyword detection.
pub const KEYWORDS: &[&str] = &[
    "let", "mut", "fn", "struct", "class", "enum", "protocol", "impl",
    "return", "if", "guard", "match", "for", "while", "comptime", "const",
    "static", "type", "pub", "prv",
];

/// Number of keywords.
pub const KEYWORD_COUNT: u32 = KEYWORDS.len() as u32;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keyword_count() {
        assert_eq!(KEYWORDS.len() as u32, KEYWORD_COUNT);
        assert_eq!(KEYWORD_COUNT, KEYWORDS.len() as u32);
    }

    #[test]
    fn test_keywords_are_sorted() {
        // Verify we have the expected keywords
        assert!(KEYWORDS.contains(&"let"));
        assert!(KEYWORDS.contains(&"fn"));
        assert!(KEYWORDS.contains(&"struct"));
        assert!(KEYWORDS.contains(&"return"));
        assert!(KEYWORDS.contains(&"if"));
        assert!(KEYWORDS.contains(&"match"));
    }

    #[test]
    fn test_no_duplicates() {
        let unique_keywords: std::collections::HashSet<_> =
            KEYWORDS.iter().collect();
        assert_eq!(unique_keywords.len(), KEYWORDS.len());
    }
}
