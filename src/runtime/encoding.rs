//! Type encoding for `Object`ive-C style method signatures.
//!
//! This module implements a simplified type encoding system inspired by
//! `Object`ive-C's `@encode()` directive. Type encodings are stored as interned
//! strings and used for:

// Allow match arms with identical bodies - different types have same byte size, kept for clarity
#![allow(clippy::match_same_arms)]
//!
//! - `Method` signature validation
//! - Argument marshalling for FFI
//! - Debugging and introspection
//!
//! # Encoding Format
//!
//! The encoding format is a string where each character represents a type:
//!
//! - `v` - void
//! - `@` - object (id)
//! - `:` - selector (SEL)
//! - `i` - int (i32)
//! - `l` - long (platform-dependent)
//! - `q` - long long (i64)
//! - `f` - float (f32)
//! - `d` - double (f64)
//! - `*` - C string (char*)
//! - `^` - pointer (void*)
//! - `#` - class (`Class`)
//! - `?` - unknown (used in blocks)
//!
//! Example encodings:
//! - `"v@:"` - void return, id self, SEL _cmd (no arguments)
//! - `"i@:i"` - int return, id self, SEL _cmd, int argument
//! - `"@@:@"` - object return, id self, SEL _cmd, object argument

use crate::error::{Error, Result};

/// Type encoding constant definitions.
pub mod types {
    /// Void type encoding
    pub const VOID: &str = "v";

    /// `Object` (id) type encoding
    pub const OBJECT: &str = "@";

    /// `Selector` (SEL) type encoding
    pub const SELECTOR: &str = ":";

    /// Int (i32) type encoding
    pub const INT: &str = "i";

    /// Long (platform-dependent) type encoding
    pub const LONG: &str = "l";

    /// Long long (i64) type encoding
    pub const LONG_LONG: &str = "q";

    /// Float (f32) type encoding
    pub const FLOAT: &str = "f";

    /// Double (f64) type encoding
    pub const DOUBLE: &str = "d";

    /// C string (char*) type encoding
    pub const C_STRING: &str = "*";

    /// Pointer (void*) type encoding
    pub const POINTER: &str = "^";

    /// `Class` (`Class`) type encoding
    pub const CLASS: &str = "#";
}

/// Validates a type encoding string for a method signature.
///
/// `Method` signatures must include at least:
/// - Return type
/// - Self parameter (@)
/// - `Selector` parameter (:)
///
/// # Arguments
///
/// * `encoding` - The encoding string to validate
///
/// # Returns
///
/// `Ok(())` if valid, `Err(Error::InvalidEncoding)` if invalid.
///
/// # Example
///
/// ```
/// use oxidec::runtime::encoding::validate_encoding;
///
/// assert!(validate_encoding("v@:").is_ok());
/// assert!(validate_encoding("i@:i").is_ok());
/// assert!(validate_encoding("xyz").is_err());
/// assert!(validate_encoding("@").is_err()); // Missing self and _cmd
/// ```
///
/// # Errors
///
/// Returns [`Error::InvalidEncoding`] if the encoding string is empty, contains
/// invalid type characters, or doesn't include the required self and _cmd
/// parameters.
///
/// # Panics
///
/// Panics if the encoding string is empty (should not happen as we check
/// for this condition and return an error instead).
pub fn validate_encoding(encoding: &str) -> Result<()> {
    if encoding.is_empty() {
        return Err(Error::InvalidEncoding);
    }

    let mut chars = encoding.chars();

    // First character must be a valid return type
    let return_type = chars.next().unwrap();
    if !is_valid_type_char(return_type) {
        return Err(Error::InvalidEncoding);
    }

    // Collect remaining characters (argument types)
    let arg_types: Vec<char> = chars.collect();

    // All characters must be valid type chars
    for ch in &arg_types {
        if !is_valid_type_char(*ch) {
            return Err(Error::InvalidEncoding);
        }
    }

    // `Method` signatures MUST include at least self (@) and _cmd (:)
    // This is the `Object`ive-C calling convention - all methods receive self and _cmd
    if arg_types.len() < 2 {
        return Err(Error::InvalidEncoding);
    }

    // First two arguments must be @ (self) and : (_cmd)
    if arg_types[0] != '@' || arg_types[1] != ':' {
        return Err(Error::InvalidEncoding);
    }

    Ok(())
}

/// Returns the size in bytes of a type encoding character.
///
/// # Arguments
///
/// * `type_char` - Single character type encoding
///
/// # Returns
///
/// Size in bytes, or `None` if invalid type character.
///
/// # Example
///
/// ```
/// use oxidec::runtime::encoding::size_of_type;
///
/// assert_eq!(size_of_type('i'), Some(4));
/// assert_eq!(size_of_type('q'), Some(8));
/// assert_eq!(size_of_type('v'), Some(0));
/// ```
#[must_use]
pub const fn size_of_type(type_char: char) -> Option<usize> {
    match type_char {
        'v' => Some(0),  // void
        '@' => Some(8),  // object (pointer)
        ':' => Some(8),  // selector (pointer)
        'i' => Some(4),  // i32
        'l' => Some(8),  // long (64-bit)
        'q' => Some(8),  // i64
        'f' => Some(4),  // f32
        'd' => Some(8),  // f64
        '*' => Some(8),  // char*
        '^' => Some(8),  // void*
        '#' => Some(8),  // `Class` (pointer)
        '?' => Some(8),  // unknown/block (pointer)
        _ => None,
    }
}

/// Checks if a character is a valid type encoding character.
const fn is_valid_type_char(ch: char) -> bool {
    matches!(ch, 'v' | '@' | ':' | 'i' | 'l' | 'q' | 'f' | 'd' | '*' | '^' | '#' | '?')
}

/// Parses a method signature encoding into return type and argument types.
///
/// # Arguments
///
/// * `encoding` - Full method encoding string (e.g., "v@:i")
///
/// # Returns
///
/// `Ok((return_type, arg_types))` where:
/// - `return_type` is the first character of the encoding
/// - `arg_types` is a vector of the remaining characters
///
/// # Example
///
/// ```
/// use oxidec::runtime::encoding::parse_signature;
///
/// let (ret, args) = parse_signature("i@:if").unwrap();
/// assert_eq!(ret, 'i');
/// assert_eq!(args, vec!['@', ':', 'i', 'f']);
/// ```
///
/// # Errors
///
/// Returns [`Error::InvalidEncoding`] if the encoding string is invalid
/// (see [`validate_encoding`] for details).
///
/// # Panics
///
/// Panics if the encoding string is empty (should not happen as validation
/// ensures it's non-empty).
pub fn parse_signature(encoding: &str) -> Result<(char, Vec<char>)> {
    validate_encoding(encoding)?;

    let mut chars = encoding.chars();
    let return_type = chars.next().unwrap();
    let arg_types: Vec<char> = chars.collect();

    Ok((return_type, arg_types))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_encoding_valid() {
        assert!(validate_encoding("v@:").is_ok());
        assert!(validate_encoding("i@:i").is_ok());
        assert!(validate_encoding("@@:@").is_ok());
        assert!(validate_encoding("q@:dq").is_ok());
    }

    #[test]
    fn test_validate_encoding_invalid() {
        assert!(validate_encoding("").is_err());
        assert!(validate_encoding("xyz").is_err());
        assert!(validate_encoding("v@:x").is_err());
        assert!(validate_encoding("@").is_err()); // Missing self and _cmd
    }

    #[test]
    fn test_size_of_type() {
        assert_eq!(size_of_type('i'), Some(4));
        assert_eq!(size_of_type('q'), Some(8));
        assert_eq!(size_of_type('v'), Some(0));
        assert_eq!(size_of_type('@'), Some(8));
        assert_eq!(size_of_type(':'), Some(8));
        assert_eq!(size_of_type('f'), Some(4));
        assert_eq!(size_of_type('d'), Some(8));
        assert_eq!(size_of_type('x'), None);
    }

    #[test]
    fn test_parse_signature() {
        let (ret, args) = parse_signature("i@:if").unwrap();
        assert_eq!(ret, 'i');
        assert_eq!(args, vec!['@', ':', 'i', 'f']);

        let (ret, args) = parse_signature("v@:").unwrap();
        assert_eq!(ret, 'v');
        assert_eq!(args, vec!['@', ':']);

        let (ret, args) = parse_signature("@@:@").unwrap();
        assert_eq!(ret, '@');
        assert_eq!(args, vec!['@', ':', '@']);
    }

    #[test]
    fn test_parse_signature_invalid() {
        assert!(parse_signature("").is_err());
        assert!(parse_signature("xyz").is_err());
    }

    #[test]
    fn test_type_constants() {
        assert_eq!(types::VOID, "v");
        assert_eq!(types::OBJECT, "@");
        assert_eq!(types::SELECTOR, ":");
        assert_eq!(types::INT, "i");
        assert_eq!(types::LONG, "l");
        assert_eq!(types::LONG_LONG, "q");
        assert_eq!(types::FLOAT, "f");
        assert_eq!(types::DOUBLE, "d");
        assert_eq!(types::C_STRING, "*");
        assert_eq!(types::POINTER, "^");
        assert_eq!(types::CLASS, "#");
    }
}
