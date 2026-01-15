//! Message arguments for dynamic dispatch.
//!
//! This module provides a type-safe, enum-based API for passing variable
//! numbers of arguments to message sending methods. All arguments are
//! type-erased as `usize` and validated against the method's type encoding
//! at runtime.
//!
//! # Design
//!
//! The `MessageArgs` enum provides:
//! - Fixed variants for 0-8 arguments (stack-allocated, zero overhead)
//! - `Many` variant for 9+ arguments (static lifetime)
//! - Convenience methods for easy construction
//! - Helper methods for accessing argument data
//!
//! # Example
//!
//! ```rust
//! use oxidec::runtime::MessageArgs;
//!
//! // No arguments
//! let args = MessageArgs::None;
//!
//! // One argument
//! let args = MessageArgs::one(42);
//!
//! // Two arguments
//! let args = MessageArgs::two(10, 20);
//!
//! // Variable arguments
//! static MANY_ARGS: [usize; 4] = [1, 2, 3, 4];
//! let args = MessageArgs::many(&MANY_ARGS);
//!
//! // Get argument count
//! assert_eq!(args.count(), 4);
//!
//! // Access as slice
//! assert_eq!(args.as_slice(), &[1, 2, 3, 4]);
//! ```

/// Arguments for message sending.
///
/// This enum provides a type-safe way to pass a variable number of
/// arguments to message sending methods. All arguments are type-erased
/// as `usize` and validated against the method's type encoding at runtime.
///
/// # Variants
///
/// - `None` - No arguments (besides self and _cmd)
/// - `One(usize)` - One argument
/// - `Two([usize; 2])` - Two arguments
/// - `Three([usize; 3])` - Three arguments
/// - `Four([usize; 4])` - Four arguments
/// - `Five([usize; 5])` - Five arguments
/// - `Six([usize; 6])` - Six arguments
/// - `Seven([usize; 7])` - Seven arguments
/// - `Eight([usize; 8])` - Eight arguments
/// - `Many(&'static [usize])` - Variable number of arguments (9+)
///
/// # Performance
///
/// All variants except `Many` are stack-allocated and have zero overhead.
/// The `Many` variant uses a static slice reference to avoid lifetime issues
/// and heap allocations.
///
/// # Example
///
/// ```rust
/// use oxidec::runtime::MessageArgs;
///
/// // No arguments
/// let args = MessageArgs::None;
/// assert_eq!(args.count(), 0);
///
/// // One argument
/// let args = MessageArgs::one(42);
/// assert_eq!(args.count(), 1);
/// assert_eq!(args.as_slice(), &[42]);
///
/// // Two arguments
/// let args = MessageArgs::two(10, 20);
/// assert_eq!(args.count(), 2);
/// assert_eq!(args.as_slice(), &[10, 20]);
///
/// // Variable arguments
/// static MANY: [usize; 4] = [1, 2, 3, 4];
/// let args = MessageArgs::many(&MANY);
/// assert_eq!(args.count(), 4);
/// assert_eq!(args.as_slice(), &[1, 2, 3, 4]);
/// ```
#[derive(Clone, Debug)]
pub enum MessageArgs {
    /// No arguments (besides self and _cmd)
    None,

    /// One argument
    One(usize),

    /// Two arguments
    Two([usize; 2]),

    /// Three arguments
    Three([usize; 3]),

    /// Four arguments
    Four([usize; 4]),

    /// Five arguments
    Five([usize; 5]),

    /// Six arguments
    Six([usize; 6]),

    /// Seven arguments
    Seven([usize; 7]),

    /// Eight arguments
    Eight([usize; 8]),

    /// Variable number of arguments (9+ or unknown count)
    Many(&'static [usize]),
}

impl MessageArgs {
    /// Creates a `MessageArgs::None` variant.
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::runtime::MessageArgs;
    ///
    /// let args = MessageArgs::none();
    /// assert!(matches!(args, MessageArgs::None));
    /// assert_eq!(args.count(), 0);
    /// ```
    #[must_use]
    pub const fn none() -> Self {
        MessageArgs::None
    }

    /// Creates a `MessageArgs::One` variant.
    ///
    /// # Arguments
    ///
    /// * `arg` - The argument value
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::runtime::MessageArgs;
    ///
    /// let args = MessageArgs::one(42);
    /// assert_eq!(args.count(), 1);
    /// assert_eq!(args.as_slice(), &[42]);
    /// ```
    #[must_use]
    pub const fn one(arg: usize) -> Self {
        MessageArgs::One(arg)
    }

    /// Creates a `MessageArgs::Two` variant.
    ///
    /// # Arguments
    ///
    /// * `arg1` - First argument
    /// * `arg2` - Second argument
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::runtime::MessageArgs;
    ///
    /// let args = MessageArgs::two(10, 20);
    /// assert_eq!(args.count(), 2);
    /// assert_eq!(args.as_slice(), &[10, 20]);
    /// ```
    #[must_use]
    pub const fn two(arg1: usize, arg2: usize) -> Self {
        MessageArgs::Two([arg1, arg2])
    }

    /// Creates a `MessageArgs::Three` variant.
    ///
    /// # Arguments
    ///
    /// * `args` - Array of three arguments
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::runtime::MessageArgs;
    ///
    /// let args = MessageArgs::three([1, 2, 3]);
    /// assert_eq!(args.count(), 3);
    /// assert_eq!(args.as_slice(), &[1, 2, 3]);
    /// ```
    #[must_use]
    pub const fn three(args: [usize; 3]) -> Self {
        MessageArgs::Three(args)
    }

    /// Creates a `MessageArgs::Four` variant.
    ///
    /// # Arguments
    ///
    /// * `args` - Array of four arguments
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::runtime::MessageArgs;
    ///
    /// let args = MessageArgs::four([1, 2, 3, 4]);
    /// assert_eq!(args.count(), 4);
    /// assert_eq!(args.as_slice(), &[1, 2, 3, 4]);
    /// ```
    #[must_use]
    pub const fn four(args: [usize; 4]) -> Self {
        MessageArgs::Four(args)
    }

    /// Creates a `MessageArgs::Five` variant.
    ///
    /// # Arguments
    ///
    /// * `args` - Array of five arguments
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::runtime::MessageArgs;
    ///
    /// let args = MessageArgs::five([1, 2, 3, 4, 5]);
    /// assert_eq!(args.count(), 5);
    /// assert_eq!(args.as_slice(), &[1, 2, 3, 4, 5]);
    /// ```
    #[must_use]
    pub const fn five(args: [usize; 5]) -> Self {
        MessageArgs::Five(args)
    }

    /// Creates a `MessageArgs::Six` variant.
    ///
    /// # Arguments
    ///
    /// * `args` - Array of six arguments
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::runtime::MessageArgs;
    ///
    /// let args = MessageArgs::six([1, 2, 3, 4, 5, 6]);
    /// assert_eq!(args.count(), 6);
    /// assert_eq!(args.as_slice(), &[1, 2, 3, 4, 5, 6]);
    /// ```
    #[must_use]
    pub const fn six(args: [usize; 6]) -> Self {
        MessageArgs::Six(args)
    }

    /// Creates a `MessageArgs::Seven` variant.
    ///
    /// # Arguments
    ///
    /// * `args` - Array of seven arguments
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::runtime::MessageArgs;
    ///
    /// let args = MessageArgs::seven([1, 2, 3, 4, 5, 6, 7]);
    /// assert_eq!(args.count(), 7);
    /// assert_eq!(args.as_slice(), &[1, 2, 3, 4, 5, 6, 7]);
    /// ```
    #[must_use]
    pub const fn seven(args: [usize; 7]) -> Self {
        MessageArgs::Seven(args)
    }

    /// Creates a `MessageArgs::Eight` variant.
    ///
    /// # Arguments
    ///
    /// * `args` - Array of eight arguments
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::runtime::MessageArgs;
    ///
    /// let args = MessageArgs::eight([1, 2, 3, 4, 5, 6, 7, 8]);
    /// assert_eq!(args.count(), 8);
    /// assert_eq!(args.as_slice(), &[1, 2, 3, 4, 5, 6, 7, 8]);
    /// ```
    #[must_use]
    pub const fn eight(args: [usize; 8]) -> Self {
        MessageArgs::Eight(args)
    }

    /// Creates a `MessageArgs::Many` variant for variable arguments.
    ///
    /// # Arguments
    ///
    /// * `args` - Static slice of arguments
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::runtime::MessageArgs;
    ///
    /// static ARGS: [usize; 4] = [1, 2, 3, 4];
    /// let args = MessageArgs::many(&ARGS);
    /// assert_eq!(args.count(), 4);
    /// assert_eq!(args.as_slice(), &[1, 2, 3, 4]);
    /// ```
    #[must_use]
    pub const fn many(args: &'static [usize]) -> Self {
        MessageArgs::Many(args)
    }

    /// Returns the number of arguments.
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::runtime::MessageArgs;
    ///
    /// assert_eq!(MessageArgs::none().count(), 0);
    /// assert_eq!(MessageArgs::one(42).count(), 1);
    /// assert_eq!(MessageArgs::two(1, 2).count(), 2);
    /// ```
    #[must_use]
    pub const fn count(&self) -> usize {
        match self {
            MessageArgs::None => 0,
            MessageArgs::One(_) => 1,
            MessageArgs::Two(_) => 2,
            MessageArgs::Three(_) => 3,
            MessageArgs::Four(_) => 4,
            MessageArgs::Five(_) => 5,
            MessageArgs::Six(_) => 6,
            MessageArgs::Seven(_) => 7,
            MessageArgs::Eight(_) => 8,
            MessageArgs::Many(slice) => slice.len(),
        }
    }

    /// Returns the arguments as a slice.
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::runtime::MessageArgs;
    ///
    /// assert_eq!(MessageArgs::none().as_slice(), &[]);
    /// assert_eq!(MessageArgs::one(42).as_slice(), &[42]);
    /// assert_eq!(MessageArgs::two(1, 2).as_slice(), &[1, 2]);
    /// ```
    #[must_use]
    pub fn as_slice(&self) -> &[usize] {
        match self {
            MessageArgs::None => &[],
            MessageArgs::One(a) => std::slice::from_ref(a),
            MessageArgs::Two(a) => a,
            MessageArgs::Three(a) => a,
            MessageArgs::Four(a) => a,
            MessageArgs::Five(a) => a,
            MessageArgs::Six(a) => a,
            MessageArgs::Seven(a) => a,
            MessageArgs::Eight(a) => a,
            MessageArgs::Many(a) => a,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_none_variant() {
        let args = MessageArgs::None;
        assert_eq!(args.count(), 0);
        assert_eq!(args.as_slice(), &[] as &[usize]);
        assert!(matches!(args, MessageArgs::None));
    }

    #[test]
    fn test_none_convenience() {
        let args = MessageArgs::none();
        assert_eq!(args.count(), 0);
        assert_eq!(args.as_slice(), &[] as &[usize]);
        assert!(matches!(args, MessageArgs::None));
    }

    #[test]
    fn test_one_variant() {
        let args = MessageArgs::One(42);
        assert_eq!(args.count(), 1);
        assert_eq!(args.as_slice(), &[42]);
    }

    #[test]
    fn test_one_convenience() {
        let args = MessageArgs::one(42);
        assert_eq!(args.count(), 1);
        assert_eq!(args.as_slice(), &[42]);
    }

    #[test]
    fn test_two_variant() {
        let args = MessageArgs::Two([10, 20]);
        assert_eq!(args.count(), 2);
        assert_eq!(args.as_slice(), &[10, 20]);
    }

    #[test]
    fn test_two_convenience() {
        let args = MessageArgs::two(10, 20);
        assert_eq!(args.count(), 2);
        assert_eq!(args.as_slice(), &[10, 20]);
    }

    #[test]
    fn test_three_variant() {
        let args = MessageArgs::Three([1, 2, 3]);
        assert_eq!(args.count(), 3);
        assert_eq!(args.as_slice(), &[1, 2, 3]);
    }

    #[test]
    fn test_three_convenience() {
        let args = MessageArgs::three([1, 2, 3]);
        assert_eq!(args.count(), 3);
        assert_eq!(args.as_slice(), &[1, 2, 3]);
    }

    #[test]
    fn test_four_variant() {
        let args = MessageArgs::Four([1, 2, 3, 4]);
        assert_eq!(args.count(), 4);
        assert_eq!(args.as_slice(), &[1, 2, 3, 4]);
    }

    #[test]
    fn test_four_convenience() {
        let args = MessageArgs::four([1, 2, 3, 4]);
        assert_eq!(args.count(), 4);
        assert_eq!(args.as_slice(), &[1, 2, 3, 4]);
    }

    #[test]
    fn test_five_variant() {
        let args = MessageArgs::Five([1, 2, 3, 4, 5]);
        assert_eq!(args.count(), 5);
        assert_eq!(args.as_slice(), &[1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_five_convenience() {
        let args = MessageArgs::five([1, 2, 3, 4, 5]);
        assert_eq!(args.count(), 5);
        assert_eq!(args.as_slice(), &[1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_six_variant() {
        let args = MessageArgs::Six([1, 2, 3, 4, 5, 6]);
        assert_eq!(args.count(), 6);
        assert_eq!(args.as_slice(), &[1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn test_six_convenience() {
        let args = MessageArgs::six([1, 2, 3, 4, 5, 6]);
        assert_eq!(args.count(), 6);
        assert_eq!(args.as_slice(), &[1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn test_seven_variant() {
        let args = MessageArgs::Seven([1, 2, 3, 4, 5, 6, 7]);
        assert_eq!(args.count(), 7);
        assert_eq!(args.as_slice(), &[1, 2, 3, 4, 5, 6, 7]);
    }

    #[test]
    fn test_seven_convenience() {
        let args = MessageArgs::seven([1, 2, 3, 4, 5, 6, 7]);
        assert_eq!(args.count(), 7);
        assert_eq!(args.as_slice(), &[1, 2, 3, 4, 5, 6, 7]);
    }

    #[test]
    fn test_eight_variant() {
        let args = MessageArgs::Eight([1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(args.count(), 8);
        assert_eq!(args.as_slice(), &[1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn test_eight_convenience() {
        let args = MessageArgs::eight([1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(args.count(), 8);
        assert_eq!(args.as_slice(), &[1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn test_many_variant() {
        static ARGS: [usize; 10] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let args = MessageArgs::many(&ARGS);
        assert_eq!(args.count(), 10);
        assert_eq!(args.as_slice(), &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn test_many_convenience() {
        static ARGS: [usize; 10] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let args = MessageArgs::many(&ARGS);
        assert_eq!(args.count(), 10);
        assert_eq!(args.as_slice(), &ARGS);
    }

    #[test]
    fn test_many_empty() {
        static ARGS: [usize; 0] = [];
        let args = MessageArgs::many(&ARGS);
        assert_eq!(args.count(), 0);
        assert_eq!(args.as_slice(), &[] as &[usize]);
    }

    #[test]
    fn test_clone() {
        let args1 = MessageArgs::two(1, 2);
        let args2 = args1.clone();
        assert_eq!(args1.count(), args2.count());
        assert_eq!(args1.as_slice(), args2.as_slice());
    }

    #[test]
    fn test_debug_output() {
        let args = MessageArgs::two(1, 2);
        let debug_str = format!("{args:?}");
        assert!(debug_str.contains("Two"));
    }
}
