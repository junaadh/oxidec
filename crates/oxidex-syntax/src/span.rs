//! Source location tracking for tokens and AST nodes.
//!
//! This module provides types for tracking source code locations, which are
//! essential for error reporting and debugging. Every token and AST node
//! carries a `Span` indicating its position in the source code.
//!
//! # Examples
//!
//! ```
//! use oxidex_syntax::span::{Span, LineCol};
//!
//! // Create a span from byte offsets
//! let span = Span::new(10, 20, 1, 5, 1, 10);
//!
//! // Merge two spans
//! let left = Span::new(0, 10, 1, 1, 1, 11);
//! let right = Span::new(15, 25, 2, 1, 2, 11);
//! let merged = Span::merge(left, right);
//!
//! assert_eq!(merged.start, 0);
//! assert_eq!(merged.end, 25);
//! ```

use std::fmt;

/// A source code span tracking byte offsets and line/column positions.
///
/// Spans are used throughout the lexer, parser, and AST to track the location
/// of tokens and nodes in the source code. This information is critical for
/// error reporting.
///
/// # Fields
///
/// - `start`: Byte offset of the span start (0-indexed)
/// - `end`: Byte offset of the span end (exclusive)
/// - `start_line`: Line number of the span start (1-indexed)
/// - `start_col`: Column number of the span start (1-indexed, in bytes)
/// - `end_line`: Line number of the span end (1-indexed)
/// - `end_col`: Column number of the span end (1-indexed, in bytes)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    /// Byte offset of the span start (0-indexed)
    pub start: usize,

    /// Byte offset of the span end (exclusive)
    pub end: usize,

    /// Line number of the span start (1-indexed)
    pub start_line: usize,

    /// Column number of the span start (1-indexed, in bytes)
    pub start_col: usize,

    /// Line number of the span end (1-indexed)
    pub end_line: usize,

    /// Column number of the span end (1-indexed, in bytes)
    pub end_col: usize,
}

impl Span {
    /// Creates a new span from byte offsets and line/column positions.
    ///
    /// # Arguments
    ///
    /// * `start` - Byte offset of the span start (0-indexed)
    /// * `end` - Byte offset of the span end (exclusive)
    /// * `start_line` - Line number of the span start (1-indexed)
    /// * `start_col` - Column number of the span start (1-indexed)
    /// * `end_line` - Line number of the span end (1-indexed)
    /// * `end_col` - Column number of the span end (1-indexed)
    ///
    /// # Examples
    ///
    /// ```
    /// use oxidex_syntax::span::Span;
    ///
    /// let span = Span::new(10, 20, 1, 5, 1, 10);
    /// ```
    #[must_use]
    pub const fn new(
        start: usize,
        end: usize,
        start_line: usize,
        start_col: usize,
        end_line: usize,
        end_col: usize,
    ) -> Self {
        Self {
            start,
            end,
            start_line,
            start_col,
            end_line,
            end_col,
        }
    }

    /// Merges two spans, creating a new span that covers both.
    ///
    /// The merged span starts at the left span's start and ends at the right
    /// span's end. Line/column information is taken from the respective spans.
    ///
    /// # Arguments
    ///
    /// * `left` - The left (earlier) span
    /// * `right` - The right (later) span
    ///
    /// # Examples
    ///
    /// ```
    /// use oxidex_syntax::span::Span;
    ///
    /// let left = Span::new(0, 10, 1, 1, 1, 11);
    /// let right = Span::new(15, 25, 2, 1, 2, 11);
    /// let merged = Span::merge(left, right);
    ///
    /// assert_eq!(merged.start, 0);
    /// assert_eq!(merged.end, 25);
    /// assert_eq!(merged.start_line, 1);
    /// assert_eq!(merged.end_line, 2);
    /// ```
    #[must_use]
    pub const fn merge(left: Span, right: Span) -> Self {
        Self {
            start: left.start,
            end: right.end,
            start_line: left.start_line,
            start_col: left.start_col,
            end_line: right.end_line,
            end_col: right.end_col,
        }
    }

    /// Returns the length of the span in bytes.
    ///
    /// # Examples
    ///
    /// ```
    /// use oxidex_syntax::span::Span;
    ///
    /// let span = Span::new(10, 20, 1, 5, 1, 10);
    /// assert_eq!(span.len(), 10);
    /// ```
    #[must_use]
    pub const fn len(&self) -> usize {
        self.end - self.start
    }

    /// Returns `true` if the span has zero length.
    ///
    /// # Examples
    ///
    /// ```
    /// use oxidex_syntax::span::Span;
    ///
    /// let empty = Span::new(10, 10, 1, 5, 1, 5);
    /// assert!(empty.is_empty());
    ///
    /// let non_empty = Span::new(10, 20, 1, 5, 1, 10);
    /// assert!(!non_empty.is_empty());
    /// ```
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// Creates a span from a single line/column position.
    ///
    /// This is useful for zero-length spans (e.g., EOF, missing tokens).
    ///
    /// # Arguments
    ///
    /// * `offset` - Byte offset
    /// * `line` - Line number (1-indexed)
    /// * `col` - Column number (1-indexed)
    ///
    /// # Examples
    ///
    /// ```
    /// use oxidex_syntax::span::Span;
    ///
    /// let span = Span::point(100, 5, 10);
    /// assert_eq!(span.start, 100);
    /// assert_eq!(span.end, 100);
    /// assert!(span.is_empty());
    /// ```
    #[must_use]
    pub const fn point(offset: usize, line: usize, col: usize) -> Self {
        Self {
            start: offset,
            end: offset,
            start_line: line,
            start_col: col,
            end_line: line,
            end_col: col,
        }
    }
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.start_line, self.start_col)
    }
}

/// A line and column position in source code.
///
/// Used for more precise position tracking than byte offsets alone.
///
/// # Fields
///
/// - `line`: Line number (1-indexed)
/// - `col`: Column number in bytes (1-indexed)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LineCol {
    /// Line number (1-indexed)
    pub line: usize,

    /// Column number in bytes (1-indexed)
    pub col: usize,
}

impl LineCol {
    /// Creates a new line/column position.
    ///
    /// # Arguments
    ///
    /// * `line` - Line number (1-indexed)
    /// * `col` - Column number (1-indexed)
    #[must_use]
    pub const fn new(line: usize, col: usize) -> Self {
        Self { line, col }
    }
}

impl fmt::Display for LineCol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.col)
    }
}

/// Trait for types that have a source span.
///
/// This trait is implemented by all AST nodes and tokens, allowing them
/// to provide their source location for error reporting.
///
/// # Examples
///
/// ```
/// use oxidex_syntax::span::{Span, Spanned};
///
/// struct MyNode {
///     span: Span,
/// }
///
/// impl Spanned for MyNode {
///     fn span(&self) -> Span {
///         self.span
///     }
/// }
/// ```
pub trait Spanned {
    /// Returns the source span of this item.
    fn span(&self) -> Span;
}

// Implement Spanned for Token (since Token is in a different module, we'll do this in lib.rs)
// Implement Spanned for Span itself
impl Spanned for Span {
    fn span(&self) -> Span {
        *self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_span_new() {
        let span = Span::new(10, 20, 1, 5, 1, 10);
        assert_eq!(span.start, 10);
        assert_eq!(span.end, 20);
        assert_eq!(span.start_line, 1);
        assert_eq!(span.start_col, 5);
        assert_eq!(span.end_line, 1);
        assert_eq!(span.end_col, 10);
    }

    #[test]
    fn test_span_merge() {
        let left = Span::new(0, 10, 1, 1, 1, 11);
        let right = Span::new(15, 25, 2, 1, 2, 11);
        let merged = Span::merge(left, right);

        assert_eq!(merged.start, 0);
        assert_eq!(merged.end, 25);
        assert_eq!(merged.start_line, 1);
        assert_eq!(merged.start_col, 1);
        assert_eq!(merged.end_line, 2);
        assert_eq!(merged.end_col, 11);
    }

    #[test]
    fn test_span_len() {
        let span = Span::new(10, 20, 1, 5, 1, 10);
        assert_eq!(span.len(), 10);
    }

    #[test]
    fn test_span_is_empty() {
        let empty = Span::new(10, 10, 1, 5, 1, 5);
        assert!(empty.is_empty());

        let non_empty = Span::new(10, 20, 1, 5, 1, 10);
        assert!(!non_empty.is_empty());
    }

    #[test]
    fn test_span_point() {
        let span = Span::point(100, 5, 10);
        assert_eq!(span.start, 100);
        assert_eq!(span.end, 100);
        assert!(span.is_empty());
        assert_eq!(span.start_line, 5);
        assert_eq!(span.start_col, 10);
    }

    #[test]
    fn test_line_col_new() {
        let pos = LineCol::new(5, 10);
        assert_eq!(pos.line, 5);
        assert_eq!(pos.col, 10);
    }

    #[test]
    fn test_span_display() {
        let span = Span::new(0, 10, 5, 10, 5, 20);
        assert_eq!(format!("{}", span), "5:10");
    }

    #[test]
    fn test_line_col_display() {
        let pos = LineCol::new(5, 10);
        assert_eq!(format!("{}", pos), "5:10");
    }
}
