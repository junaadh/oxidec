//! Lexical analysis (tokenization) for the `OxideX` language.
//!
//! The lexer transforms source code text into a stream of tokens, which are
//! then consumed by the parser. It handles all lexical elements including:
//!
//! - Keywords and identifiers
//! - Numeric literals (integer and floating-point)
//! - String literals with escape sequences
//! - String interpolation with nested expressions
//! - Comments (line, block, nested, documentation)
//! - Operators and delimiters
//! - Unicode identifiers
//!
//! # Examples
//!
//! ```
//! use oxidex_syntax::lexer::Lexer;
//!
//! let source = r#"let x = 42"#;
//! let mut lexer = Lexer::new(source);
//! let tokens = lexer.lex().unwrap();
//!
//! assert_eq!(tokens.len(), 5); // let, x, =, 42, EOF
//! ```
//!
//! # Performance
//!
//! The lexer is designed for high performance:
//! - Single pass through source (no backtracking)
//! - Minimal allocations (uses `&str` slices where possible)
//! - Efficient character classification
//! - Target: > 100k LOC/sec

use crate::error::{LexerError, LexerResult};
use crate::keywords;
use crate::span::Span;
use crate::token::{Token, TokenKind};
use oxidex_mem::{StringInterner, Symbol};
use std::iter::Peekable;
use std::str::Chars;

/// Lexical analyzer for `OxideX` source code.
///
/// The lexer processes source code character by character, producing tokens
/// with source location information. It handles error recovery by skipping
/// invalid characters and continuing tokenization.
///
/// # Fields
///
/// * `input` - The source code being tokenized
/// * `chars` - Iterator over characters with lookahead capability
/// * `position` - Current byte offset in the source
/// * `line` - Current line number (1-indexed)
/// * `column` - Current column number in bytes (1-indexed)
/// * `tokens` - Accumulated tokens
/// * `errors` - Accumulated errors
/// * `interner` - String interner for deduplicating identifiers and literals
pub struct Lexer<'input> {
    /// The source code being tokenized
    input: &'input str,

    /// Character iterator with peek capability (initialized in `lex()`)
    chars: Option<Peekable<Chars<'input>>>,

    /// Current byte offset in the source
    position: usize,

    /// Current line number (1-indexed)
    line: usize,

    /// Current column number in bytes (1-indexed)
    column: usize,

    /// Accumulated tokens
    tokens: Vec<Token>,

    /// Accumulated errors
    errors: Vec<LexerError>,

    /// String interner for deduplicating identifiers and literals
    interner: StringInterner,
}

impl<'input> Lexer<'input> {
    /// Creates a new lexer for the given source code.
    ///
    /// # Arguments
    ///
    /// * `input` - The source code to tokenize
    ///
    /// # Examples
    ///
    /// ```
    /// use oxidex_syntax::lexer::Lexer;
    ///
    /// let source = "let x = 42";
    /// let lexer = Lexer::new(source);
    /// ```
    #[must_use]
    pub fn new(input: &'input str) -> Self {
        Self {
            input,
            // We'll initialize chars in lex() since we can't create a Peekable without the input
            chars: None,
            position: 0,
            line: 1,
            column: 1,
            tokens: Vec::new(),
            errors: Vec::new(),
            interner: StringInterner::with_pre_interned(keywords::KEYWORDS),
        }
    }

    /// Tokenizes the entire source code.
    ///
    /// Returns a vector of tokens or a vector of errors. The lexer attempts
    /// error recovery, so it may return both tokens and errors.
    ///
    /// # Examples
    ///
    /// ```
    /// use oxidex_syntax::lexer::Lexer;
    ///
    /// let source = "let x = 42";
    /// let mut lexer = Lexer::new(source);
    /// let result = lexer.lex();
    ///
    /// assert!(result.is_ok());
    /// ```
    ///
    /// # Panics
    ///
    /// This function does not panic.
    ///
    /// # Errors
    ///
    /// Returns a `LexerError` if the source contains invalid characters that
    /// cannot be recovered from.
    pub fn lex(mut self) -> LexerResult<Vec<Token>> {
        // Initialize the character iterator
        self.chars = Some(self.input.chars().peekable());

        // Main tokenization loop
        while self.peek().is_some() {
            // Skip whitespace
            self.skip_whitespace();

            // Check for EOF
            if self.peek().is_none() {
                break;
            }

            // Get next token
            match self.next_token() {
                Ok(token) => self.tokens.push(token),
                Err(err) => {
                    self.errors.push(err);
                    // Attempt recovery by skipping to next known token
                    self.recover();
                }
            }
        }

        // Add EOF token
        let eof_span = Span::point(self.position, self.line, self.column);
        self.tokens.push(Token::new(TokenKind::EOF, eof_span));

        // Return result
        if self.errors.is_empty() {
            Ok(self.tokens)
        } else {
            // Return first error for now (we could enhance this to return all errors)
            Err(self.errors.into_iter().next().unwrap())
        }
    }

    /// Resolves a Symbol to its string representation.
    ///
    /// This is useful for error reporting and debugging. Returns the string
    /// if the symbol is valid, or "<invalid>" if the symbol ID is unknown.
    ///
    /// # Arguments
    ///
    /// * `sym` - The Symbol to resolve
    ///
    /// # Examples
    ///
    /// ```
    /// use oxidex_syntax::lexer::Lexer;
    /// use oxidex_mem::Symbol;
    ///
    /// let source = "let x = 42";
    /// let lexer = Lexer::new(source);
    ///
    /// // Resolve a symbol (in practice, you'd get symbols from tokens)
    /// let sym = Symbol::new(0); // Pre-interned keyword "let"
    /// assert_eq!(lexer.resolve_symbol(sym), "let");
    /// ```
    #[must_use]
    pub fn resolve_symbol(&self, sym: Symbol) -> &str {
        self.interner.resolve(sym).unwrap_or("<invalid>")
    }

    /// Returns a reference to the string interner.
    ///
    /// This is primarily useful for testing, where you may want to
    /// intern expected values using the same interner state.
    #[must_use]
    pub fn interner(&self) -> &StringInterner {
        &self.interner
    }

    /// Consumes the lexer and returns the string interner.
    ///
    /// This is useful when you need to pass the interner to the parser
    /// after lexing is complete.
    #[must_use]
    pub fn into_interner(self) -> StringInterner {
        self.interner
    }

    /// Returns a clone of the string interner.
    ///
    /// This is useful when you need to pass the interner to the parser
    /// after lexing is complete but don't want to consume the lexer.
    #[must_use]
    pub fn clone_interner(&self) -> StringInterner {
        // Create a new interner and copy all interned strings
        let mut new_interner =
            StringInterner::with_pre_interned(keywords::KEYWORDS);
        for id in 0..self.interner.len() {
            let sym = Symbol::new(id as u32);
            if let Some(s) = self.interner.resolve(sym) {
                new_interner.intern(s);
            }
        }
        new_interner
    }

    /// Tokenizes the source code and returns both tokens and the interner.
    ///
    /// This is a convenience method that combines `lex()` and `into_interner()`.
    /// It's useful when you need both the tokens and the interner for parsing.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use oxidex_syntax::Lexer;
    ///
    /// let source = "let x = 42";
    /// let lexer = Lexer::new(source);
    /// let (tokens, interner) = lexer.lex_with_interner().unwrap();
    /// ```
    ///
    /// # Errors
    ///
    /// Returns a `LexerError` if the source contains invalid characters that
    /// cannot be recovered from.
    pub fn lex_with_interner(
        mut self,
    ) -> LexerResult<(Vec<Token>, StringInterner)> {
        // Initialize the character iterator
        self.chars = Some(self.input.chars().peekable());

        // Main tokenization loop
        while self.peek().is_some() {
            // Skip whitespace
            self.skip_whitespace();

            // Check for EOF
            if self.peek().is_none() {
                break;
            }

            // Get next token
            match self.next_token() {
                Ok(token) => self.tokens.push(token),
                Err(err) => {
                    self.errors.push(err);
                    // Attempt recovery by skipping to next known token
                    self.recover();
                }
            }
        }

        // Add EOF token
        let eof_span = Span::point(self.position, self.line, self.column);
        self.tokens.push(Token::new(TokenKind::EOF, eof_span));

        // Return result with interner
        if self.errors.is_empty() {
            Ok((self.tokens, self.interner))
        } else {
            // Return first error for now (we could enhance this to return all errors)
            Err(self.errors.into_iter().next().unwrap())
        }
    }

    /// Peeks at the next character without consuming it.
    fn peek(&mut self) -> Option<char> {
        self.chars.as_mut()?.peek().copied()
    }

    /// Peeks two characters ahead.
    fn peek2(&mut self) -> Option<char> {
        let chars = self.chars.as_mut()?;
        let mut iter = chars.clone();
        iter.next(); // Skip first
        iter.next() // Get second
    }

    /// Consumes and returns the next character.
    ///
    /// Updates position, line, and column tracking.
    fn bump(&mut self) -> Option<char> {
        let ch = self.chars.as_mut()?.next()?;

        // Update position tracking
        if ch == '\n' {
            self.line += 1;
            self.column = 1;
            self.position += 1;
        } else {
            self.column += 1;
            self.position += ch.len_utf8();
        }

        Some(ch)
    }

    /// Skips whitespace characters (spaces, tabs, newlines).
    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.peek() {
            if ch.is_whitespace() {
                self.bump();
            } else {
                break;
            }
        }
    }

    /// Reads the next token from the source.
    #[allow(clippy::too_many_lines)]
    fn next_token(&mut self) -> LexerResult<Token> {
        let start = self.position;
        let start_line = self.line;
        let start_col = self.column;

        // Dispatch based on first character
        let ch = self.peek().ok_or_else(|| LexerError::UnknownChar {
            ch: '\0',
            span: Span::point(start, start_line, start_col),
        })?;

        let kind = match ch {
            // Underscore (wildcard pattern)
            '_' => {
                self.bump();
                // Check if it's just "_" or the start of an identifier
                if let Some(next_ch) = self.peek() {
                    if next_ch.is_alphanumeric() || next_ch == '_' {
                        // It's the start of an identifier
                        self.read_identifier()
                    } else {
                        // Just a standalone underscore
                        TokenKind::Underscore
                    }
                } else {
                    // Just underscore at EOF
                    TokenKind::Underscore
                }
            }

            // Identifiers (start with letter)
            'a'..='z' | 'A'..='Z' => self.read_identifier(),

            // Numeric literals (start with digit)
            '0'..='9' => self.read_number(),

            // String literals
            '"' => self.read_string(),

            // Character literals (if we support them)
            // '\'' => self.read_char(),

            // Operators and delimiters
            '+' => {
                self.bump();
                TokenKind::Plus
            }
            '-' => {
                self.bump();
                if let Some('>') = self.peek() {
                    self.bump();
                    TokenKind::Arrow
                } else {
                    TokenKind::Minus
                }
            }
            '*' => {
                self.bump();
                TokenKind::Star
            }
            '/' => {
                self.bump();
                if let Some('/') = self.peek() {
                    // Line comment
                    self.read_line_comment();
                    // Skip whitespace including the newline after comment
                    self.skip_whitespace();
                    // Return a token after the comment
                    return self.next_token();
                } else if let Some('*') = self.peek() {
                    // Block comment
                    self.read_block_comment();
                    // Skip whitespace after comment
                    self.skip_whitespace();
                    // Return a token after the comment
                    return self.next_token();
                }
                TokenKind::Slash
            }
            '%' => {
                self.bump();
                TokenKind::Percent
            }
            '=' => {
                self.bump();
                if let Some('=') = self.peek() {
                    self.bump();
                    TokenKind::EqEq
                } else if let Some('>') = self.peek() {
                    self.bump();
                    TokenKind::FatArrow
                } else {
                    TokenKind::Eq
                }
            }
            '!' => {
                self.bump();
                if let Some('=') = self.peek() {
                    self.bump();
                    TokenKind::BangEq
                } else {
                    TokenKind::Bang
                }
            }
            '<' => {
                self.bump();
                if let Some('=') = self.peek() {
                    self.bump();
                    TokenKind::LtEq
                } else {
                    TokenKind::LAngle
                }
            }
            '>' => {
                self.bump();
                if let Some('=') = self.peek() {
                    self.bump();
                    TokenKind::GtEq
                } else {
                    TokenKind::RAngle
                }
            }
            '&' => {
                self.bump();
                if let Some('&') = self.peek() {
                    self.bump();
                    TokenKind::AmpAmp
                } else {
                    TokenKind::Amp
                }
            }
            '|' => {
                self.bump();
                if let Some('|') = self.peek() {
                    self.bump();
                    TokenKind::PipePipe
                } else {
                    TokenKind::Pipe
                }
            }
            '(' => {
                self.bump();
                TokenKind::LParen
            }
            ')' => {
                self.bump();
                TokenKind::RParen
            }
            '{' => {
                self.bump();
                TokenKind::LBrace
            }
            '}' => {
                self.bump();
                TokenKind::RBrace
            }
            '[' => {
                self.bump();
                TokenKind::LBracket
            }
            ']' => {
                self.bump();
                TokenKind::RBracket
            }
            '.' => {
                self.bump();
                if let Some('.') = self.peek() {
                    self.bump();
                    TokenKind::DotDot
                } else {
                    TokenKind::Dot
                }
            }
            ':' => {
                self.bump();
                if let Some(':') = self.peek() {
                    self.bump();
                    TokenKind::ColonColon
                } else {
                    TokenKind::Colon
                }
            }
            ',' => {
                self.bump();
                TokenKind::Comma
            }
            ';' => {
                self.bump();
                TokenKind::Semicolon
            }
            '?' => {
                self.bump();
                TokenKind::Question
            }
            _ => {
                // Unknown character
                return Err(LexerError::UnknownChar {
                    ch,
                    span: Span::new(
                        start,
                        self.position,
                        start_line,
                        start_col,
                        self.line,
                        self.column,
                    ),
                });
            }
        };

        let end = self.position;
        let span = Span::new(
            start,
            end,
            start_line,
            start_col,
            self.line,
            self.column,
        );

        Ok(Token::new(kind, span))
    }

    /// Reads an identifier or keyword.
    fn read_identifier(&mut self) -> TokenKind {
        let start = self.position;
        self.bump(); // Consume first character

        // Consume remaining characters
        while let Some(ch) = self.peek() {
            if ch.is_alphanumeric() || ch == '_' {
                self.bump();
            } else {
                break;
            }
        }

        // Get the identifier text
        let text = &self.input[start..self.position];

        // Intern the string to get a Symbol
        let sym = self.interner.intern(text);

        // Check if it's a keyword by Symbol ID (keywords are 0-22)
        // Also check for boolean literals and nil
        match text {
            "let" => TokenKind::Let,
            "mut" => TokenKind::Mut,
            "fn" => TokenKind::Fn,
            "struct" => TokenKind::Struct,
            "class" => TokenKind::Class,
            "enum" => TokenKind::Enum,
            "protocol" => TokenKind::Protocol,
            "impl" => TokenKind::Impl,
            "return" => TokenKind::Return,
            "if" => TokenKind::If,
            "guard" => TokenKind::Guard,
            "match" => TokenKind::Match,
            "for" => TokenKind::For,
            "while" => TokenKind::While,
            "comptime" => TokenKind::Comptime,
            "const" => TokenKind::Const,
            "static" => TokenKind::Static,
            "type" => TokenKind::Type,
            "pub" => TokenKind::Pub,
            "prv" => TokenKind::Prv,
            "self" => TokenKind::SelfValue,
            "Self" => TokenKind::SelfType,
            "init" => TokenKind::Init,
            "case" => TokenKind::Case,
            "true" => TokenKind::BoolLiteral(true),
            "false" => TokenKind::BoolLiteral(false),
            "nil" => TokenKind::Nil,
            _ => TokenKind::Ident(sym),
        }
    }

    /// Reads a numeric literal (integer or float).
    fn read_number(&mut self) -> TokenKind {
        let start = self.position;
        self.bump(); // Consume first digit

        // Check for hex (0x) or binary (0b) prefix
        if self.position - start == 1 && self.input.as_bytes()[start] == b'0' {
            match self.peek() {
                Some('x' | 'X') => {
                    // Hexadecimal literal
                    self.bump(); // Consume 'x'
                    while let Some(ch) = self.peek() {
                        if ch.is_ascii_hexdigit() || ch == '_' {
                            self.bump();
                        } else {
                            break;
                        }
                    }

                    // Check for type suffix
                    let suffix = self.read_type_suffix();
                    let value = &self.input[start..self.position];
                    let value_sym = self.interner.intern(value);

                    return TokenKind::IntegerLiteral(value_sym, suffix);
                }
                Some('b' | 'B') => {
                    // Binary literal
                    self.bump(); // Consume 'b'
                    while let Some(ch) = self.peek() {
                        if ch == '0' || ch == '1' || ch == '_' {
                            self.bump();
                        } else {
                            break;
                        }
                    }

                    // Check for type suffix
                    let suffix = self.read_type_suffix();
                    let value = &self.input[start..self.position];
                    let value_sym = self.interner.intern(value);

                    return TokenKind::IntegerLiteral(value_sym, suffix);
                }
                _ => {
                    // Just 0 or starts with 0 (decimal)
                }
            }
        }

        // Decimal integer or float
        let mut has_dot = false;
        let mut has_exponent = false;

        while let Some(ch) = self.peek() {
            if ch.is_ascii_digit() {
                self.bump();
            } else if ch == '_' {
                // Underscore separator (allowed in numbers)
                self.bump();
            } else if ch == '.' && !has_dot {
                // Could be float literal
                // Look ahead to check if next char is digit (for float) or operator/identifier
                self.bump(); // Consume '.'
                if let Some(next_ch) = self.peek() {
                    if next_ch.is_ascii_digit() {
                        has_dot = true;
                    } else {
                        // Not a float, this is field access or something else
                        // Put the dot back by returning early
                        let value = &self.input[start..self.position - 1];
                        let value_sym = self.interner.intern(value);
                        return TokenKind::IntegerLiteral(value_sym, None);
                    }
                }
            } else if (ch == 'e' || ch == 'E') && !has_exponent {
                // Float exponent
                self.bump(); // Consume 'e' or 'E'
                has_exponent = true;

                // Optional sign after exponent
                if let Some('+' | '-') = self.peek() {
                    self.bump();
                }
            } else {
                break;
            }
        }

        // Check for type suffix (f32, f64, etc.)
        if has_dot || has_exponent {
            // Float literal
            let suffix = self.read_type_suffix();
            let value = &self.input[start..self.position];
            let value_sym = self.interner.intern(value);
            TokenKind::FloatLiteral(value_sym, suffix)
        } else {
            // Integer literal
            let suffix = self.read_type_suffix();
            let value = &self.input[start..self.position];
            let value_sym = self.interner.intern(value);
            TokenKind::IntegerLiteral(value_sym, suffix)
        }
    }

    /// Reads a type suffix (e.g., u32, i64, f32, f64) if present.
    fn read_type_suffix(&mut self) -> Option<Symbol> {
        let start = self.position;

        // Check for type suffix
        if let Some(ch) = self.peek()
            && ch.is_alphabetic()
        {
            self.bump(); // Consume first letter

            // Consume rest of identifier
            while let Some(ch) = self.peek() {
                if ch.is_alphanumeric() {
                    self.bump();
                } else {
                    break;
                }
            }

            let suffix = &self.input[start..self.position];

            // Validate it's a known type suffix
            match suffix {
                "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "u8"
                | "u16" | "u32" | "u64" | "u128" | "usize" | "f32" | "f64" => {
                    return Some(self.interner.intern(suffix));
                }
                _ => {
                    // Not a valid type suffix - this is an error
                    // For now, just return None and treat it as part of the number
                    // In a full implementation, we'd return an error here
                    return None;
                }
            }
        }

        None
    }

    /// Reads a string literal, potentially with interpolation.
    fn read_string(&mut self) -> TokenKind {
        let start = self.position;
        self.bump(); // Consume opening quote

        // Check if this is an empty string (next char is closing quote or EOF)
        if let Some(ch) = self.peek() {
            if ch == '"' {
                // Empty string ""
                self.bump(); // Consume closing quote
                let sym =
                    self.interner.intern(&self.input[start..self.position]);
                return TokenKind::StringLiteral(sym);
            }
        } else {
            // EOF - unterminated string
            let sym = self.interner.intern(&self.input[start..self.position]);
            return TokenKind::StringLiteral(sym);
        }

        // Non-empty string - parse contents
        while let Some(ch) = self.peek() {
            if ch == '"' {
                self.bump();
                break;
            } else if ch == '\\' {
                // Escape sequence
                self.bump();
                if self.peek().is_some() {
                    self.bump();
                }
            } else if ch == '(' && self.position > start {
                // Check for interpolation start: "\("
                // Look back to see if previous char was '\'
                let prev_byte = self.input.as_bytes()[self.position - 1];
                if prev_byte == b'\\' {
                    // It's an escaped '('
                    self.bump();
                } else {
                    // String interpolation start!
                    // For now, we'll handle this as multiple tokens
                    // Return the string part so far as a StringLiteral token
                    // The parser will handle combining interpolation parts

                    // For simplicity in the current implementation, just consume it
                    // In a full implementation, this would break into:
                    // - StringEnd token
                    // - InterpolationStart token
                    // - expression tokens
                    // - InterpolationEnd token
                    // - StringStart token

                    // For now, just continue and treat it as part of the string
                    self.bump(); // Consume '('

                    // Consume until closing ')'
                    let mut depth = 1;
                    while depth > 0 && self.peek().is_some() {
                        if let Some('(') = self.peek() {
                            depth += 1;
                            self.bump();
                        } else if let Some(')') = self.peek() {
                            depth -= 1;
                            self.bump();
                        } else {
                            self.bump();
                        }
                    }
                }
            } else if ch == '\n' {
                // Unterminated string
                // We'll still consume it and return what we have
                self.bump();
                break;
            } else {
                self.bump();
            }
        }

        let sym = self.interner.intern(&self.input[start..self.position]);
        TokenKind::StringLiteral(sym)
    }

    /// Reads a line comment (consumes to end of line).
    fn read_line_comment(&mut self) {
        while let Some(ch) = self.peek() {
            if ch == '\n' {
                break;
            }
            self.bump();
        }
    }

    /// Reads a block comment (supports nesting).
    fn read_block_comment(&mut self) {
        let mut depth = 1;
        self.bump(); // Consume '*'

        while depth > 0 {
            match (self.peek(), self.peek2()) {
                (Some('/'), Some('*')) => {
                    self.bump(); // '/'
                    self.bump(); // '*'
                    depth += 1;
                }
                (Some('*'), Some('/')) => {
                    self.bump(); // '*'
                    self.bump(); // '/'
                    depth -= 1;
                }
                (Some(_), _) => {
                    self.bump();
                }
                (None, _) => {
                    // EOF - unterminated comment
                    break;
                }
            }
        }
    }

    /// Attempts error recovery by skipping to the next known token.
    fn recover(&mut self) {
        // Skip until we find a known token start
        while let Some(ch) = self.peek() {
            match ch {
                'a'..='z'
                | 'A'..='Z'
                | '_'
                | '0'..='9'
                | '"'
                | '('
                | ')'
                | '{'
                | '}'
                | '['
                | ']' => {
                    break; // Found a safe restart point
                }
                '\n' => {
                    self.bump();
                    break; // Newline is also safe
                }
                _ => {
                    self.bump(); // Skip unknown character
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test helper for interning strings with a shared interner.
    /// This ensures Symbol IDs are consistent within a test.
    struct TestInterner {
        interner: StringInterner,
    }

    impl TestInterner {
        fn new() -> Self {
            Self {
                interner: StringInterner::with_pre_interned(keywords::KEYWORDS),
            }
        }

        fn intern(&mut self, s: &str) -> Symbol {
            self.interner.intern(s)
        }
    }

    /// Helper function to intern a string and get its Symbol for test assertions.
    /// This ensures consistent Symbol IDs between test expectations and actual tokens.
    /// All StringInterner instances start with the same 19 keywords (IDs 0-18),
    /// so Symbol IDs are assigned deterministically within a single TestInterner instance.
    fn intern_for_test(s: &str) -> Symbol {
        let mut interner = TestInterner::new();
        interner.intern(s)
    }

    /// Helper function to intern multiple strings with consistent IDs.
    fn intern_for_test_many(strs: &[&str]) -> Vec<Symbol> {
        let mut interner = TestInterner::new();
        strs.iter().map(|s| interner.intern(s)).collect()
    }

    #[test]
    fn test_lexer_empty() {
        let lexer = Lexer::new("");
        let result = lexer.lex();
        assert!(result.is_ok());

        let tokens = result.unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, TokenKind::EOF);
    }

    #[test]
    fn test_lexer_keywords() {
        let source = "let mut fn";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert_eq!(result[0].kind, TokenKind::Let);
        assert_eq!(result[1].kind, TokenKind::Mut);
        assert_eq!(result[2].kind, TokenKind::Fn);
        assert_eq!(result[3].kind, TokenKind::EOF);
    }

    #[test]
    fn test_lexer_identifier() {
        let source = "myVariable";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert_eq!(
            result[0].kind,
            TokenKind::Ident(intern_for_test("myVariable"))
        );
    }

    #[test]
    fn test_lexer_integer_literal() {
        let source = "42";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert_eq!(
            result[0].kind,
            TokenKind::IntegerLiteral(intern_for_test("42"), None)
        );
    }

    #[test]
    fn test_lexer_operators() {
        let source = "+ - * / %";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert_eq!(result[0].kind, TokenKind::Plus);
        assert_eq!(result[1].kind, TokenKind::Minus);
        assert_eq!(result[2].kind, TokenKind::Star);
        assert_eq!(result[3].kind, TokenKind::Slash);
        assert_eq!(result[4].kind, TokenKind::Percent);
    }

    #[test]
    fn test_lexer_comparison_operators() {
        let source = "== != < > <=";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert_eq!(result[0].kind, TokenKind::EqEq);
        assert_eq!(result[1].kind, TokenKind::BangEq);
        assert_eq!(result[2].kind, TokenKind::LAngle);
        assert_eq!(result[3].kind, TokenKind::RAngle);
        assert_eq!(result[4].kind, TokenKind::LtEq);
    }

    #[test]
    fn test_lexer_delimiters() {
        let source = "() {} []";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert_eq!(result[0].kind, TokenKind::LParen);
        assert_eq!(result[1].kind, TokenKind::RParen);
        assert_eq!(result[2].kind, TokenKind::LBrace);
        assert_eq!(result[3].kind, TokenKind::RBrace);
        assert_eq!(result[4].kind, TokenKind::LBracket);
        assert_eq!(result[5].kind, TokenKind::RBracket);
    }

    #[test]
    fn test_lexer_line_comment() {
        let source = "let x = 42 // this is a comment\nlet y";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        // Should have: let, x, =, 42, let, y, EOF
        assert_eq!(result.len(), 7);
        assert_eq!(result[0].kind, TokenKind::Let);
        assert_eq!(result[4].kind, TokenKind::Let);
    }

    #[test]
    fn test_lexer_bool_literals() {
        let source = "true false nil";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert_eq!(result[0].kind, TokenKind::BoolLiteral(true));
        assert_eq!(result[1].kind, TokenKind::BoolLiteral(false));
        assert_eq!(result[2].kind, TokenKind::Nil);
    }

    // ===== Numeric Literal Tests =====

    #[test]
    fn test_lexer_hex_literal() {
        let source = "0xFF 0xff 0xabCD 0xDEAD_BEEF";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        let syms =
            intern_for_test_many(&["0xFF", "0xff", "0xabCD", "0xDEAD_BEEF"]);
        assert_eq!(result[0].kind, TokenKind::IntegerLiteral(syms[0], None));
        assert_eq!(result[1].kind, TokenKind::IntegerLiteral(syms[1], None));
        assert_eq!(result[2].kind, TokenKind::IntegerLiteral(syms[2], None));
        assert_eq!(result[3].kind, TokenKind::IntegerLiteral(syms[3], None));
    }

    #[test]
    fn test_lexer_binary_literal() {
        let source = "0b1010 0b0011 0b1111_0000";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        let syms = intern_for_test_many(&["0b1010", "0b0011", "0b1111_0000"]);
        assert_eq!(result[0].kind, TokenKind::IntegerLiteral(syms[0], None));
        assert_eq!(result[1].kind, TokenKind::IntegerLiteral(syms[1], None));
        assert_eq!(result[2].kind, TokenKind::IntegerLiteral(syms[2], None));
    }

    #[test]
    fn test_lexer_float_literal() {
        let source = "3.14 0.5 1.0 10.0";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        let syms = intern_for_test_many(&["3.14", "0.5", "1.0", "10.0"]);
        assert_eq!(result[0].kind, TokenKind::FloatLiteral(syms[0], None));
        assert_eq!(result[1].kind, TokenKind::FloatLiteral(syms[1], None));
        assert_eq!(result[2].kind, TokenKind::FloatLiteral(syms[2], None));
        assert_eq!(result[3].kind, TokenKind::FloatLiteral(syms[3], None));
    }

    #[test]
    fn test_lexer_float_exponent() {
        let source = "1e10 1.5e-5 3.14e+2";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        let syms = intern_for_test_many(&["1e10", "1.5e-5", "3.14e+2"]);
        assert_eq!(result[0].kind, TokenKind::FloatLiteral(syms[0], None));
        assert_eq!(result[1].kind, TokenKind::FloatLiteral(syms[1], None));
        assert_eq!(result[2].kind, TokenKind::FloatLiteral(syms[2], None));
    }

    #[test]
    fn test_lexer_type_suffix() {
        let source = "42u32 1_000i64 3.14f32";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        // Lexer interns suffixes BEFORE values, so order is: u32, 42u32, i64, 1_000i64, f32, 3.14f32
        let syms = intern_for_test_many(&[
            "u32", "42u32", "i64", "1_000i64", "f32", "3.14f32",
        ]);
        assert_eq!(
            result[0].kind,
            TokenKind::IntegerLiteral(syms[1], Some(syms[0]))
        );
        assert_eq!(
            result[1].kind,
            TokenKind::IntegerLiteral(syms[3], Some(syms[2]))
        );
        assert_eq!(
            result[2].kind,
            TokenKind::FloatLiteral(syms[5], Some(syms[4]))
        );
    }

    #[test]
    fn test_lexer_hex_with_suffix() {
        let source = "0xFFu8 0xFFFFu16";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        // Lexer interns suffix BEFORE value, so order is: u8, 0xFFu8, u16, 0xFFFFu16
        let syms = intern_for_test_many(&["u8", "0xFFu8", "u16", "0xFFFFu16"]);
        assert_eq!(
            result[0].kind,
            TokenKind::IntegerLiteral(syms[1], Some(syms[0]))
        );
        assert_eq!(
            result[1].kind,
            TokenKind::IntegerLiteral(syms[3], Some(syms[2]))
        );
    }

    #[test]
    fn test_lexer_number_underscores() {
        let source = "1_000_000 0xFFFF_FFFF 3.141_592";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        let syms =
            intern_for_test_many(&["1_000_000", "0xFFFF_FFFF", "3.141_592"]);
        assert_eq!(result[0].kind, TokenKind::IntegerLiteral(syms[0], None));
        assert_eq!(result[1].kind, TokenKind::IntegerLiteral(syms[1], None));
        assert_eq!(result[2].kind, TokenKind::FloatLiteral(syms[2], None));
    }

    // ===== String Literal Tests =====

    #[test]
    fn test_lexer_string_literal() {
        // Note: The input has a single quote at the end (unterminated string), not two quotes
        // This is because in Rust raw strings, r#"..."# contains the literal content between #" and "#
        // So r#""hello" "world" ""# contains "hello" "world" " (only one " at the end)
        let source = r#""hello" "world" ""#;
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert_eq!(result.len(), 4); // 3 strings + EOF
        let syms = intern_for_test_many(&[r#""hello""#, r#""world""#, r#"""#]);
        assert_eq!(result[0].kind, TokenKind::StringLiteral(syms[0]));
        assert_eq!(result[1].kind, TokenKind::StringLiteral(syms[1]));
        assert_eq!(result[2].kind, TokenKind::StringLiteral(syms[2])); // Unterminated
    }

    #[test]
    fn test_lexer_string_with_empty_string() {
        // To test an actual empty string, we need to escape properly or use different quotes
        let source = "\"hello\" \"world\" \"\"";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert_eq!(result.len(), 4); // 3 strings + EOF
        let syms = intern_for_test_many(&["\"hello\"", "\"world\"", "\"\""]);
        assert_eq!(result[0].kind, TokenKind::StringLiteral(syms[0]));
        assert_eq!(result[1].kind, TokenKind::StringLiteral(syms[1]));
        assert_eq!(result[2].kind, TokenKind::StringLiteral(syms[2])); // Empty string
    }

    #[test]
    fn test_lexer_empty_string_only() {
        let source = r#""""#;
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert_eq!(result.len(), 2); // Empty string + EOF
        assert_eq!(
            result[0].kind,
            TokenKind::StringLiteral(intern_for_test(r#""""#))
        );
        assert_eq!(result[1].kind, TokenKind::EOF);
    }

    #[test]
    fn test_lexer_string_escape() {
        let source = r#""hello\n" "tab\t" "quote\"" "backslash\\"" "#;
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        let syms = intern_for_test_many(&[
            r#""hello\n""#,
            r#""tab\t""#,
            r#""quote\"""#,
            r#""backslash\\""#,
        ]);
        assert_eq!(result[0].kind, TokenKind::StringLiteral(syms[0]));
        assert_eq!(result[1].kind, TokenKind::StringLiteral(syms[1]));
        assert_eq!(result[2].kind, TokenKind::StringLiteral(syms[2]));
        assert_eq!(result[3].kind, TokenKind::StringLiteral(syms[3]));
    }

    #[test]
    fn test_lexer_string_interpolation_simple() {
        let source = r#""Hello \(name)!""#;
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        // Should parse as a single string literal with interpolation
        assert_eq!(
            result[0].kind,
            TokenKind::StringLiteral(intern_for_test(r#""Hello \(name)!""#))
        );
    }

    #[test]
    fn test_lexer_string_interpolation_nested() {
        let source = r#""Value: \(x + \(y))""#;
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        // Should parse nested interpolation
        assert_eq!(
            result[0].kind,
            TokenKind::StringLiteral(intern_for_test(
                r#""Value: \(x + \(y))""#
            ))
        );
    }

    // ===== Operator Tests =====

    #[test]
    fn test_lexer_arrow_operators() {
        let source = "-> =>";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert_eq!(result[0].kind, TokenKind::Arrow);
        assert_eq!(result[1].kind, TokenKind::FatArrow);
    }

    #[test]
    fn test_lexer_colon_colon() {
        let source = "::";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert_eq!(result[0].kind, TokenKind::ColonColon);
    }

    #[test]
    fn test_lexer_logical_operators() {
        let source = "&& || !";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert_eq!(result[0].kind, TokenKind::AmpAmp);
        assert_eq!(result[1].kind, TokenKind::PipePipe);
        assert_eq!(result[2].kind, TokenKind::Bang);
    }

    // ===== Comment Tests =====

    #[test]
    fn test_lexer_block_comment() {
        let source = "let x /* comment */ = 42";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        // Should have: let, x, =, 42, EOF
        assert_eq!(result.len(), 5);
        assert_eq!(result[0].kind, TokenKind::Let);
        let syms = intern_for_test_many(&["x", "42"]);
        assert_eq!(result[1].kind, TokenKind::Ident(syms[0]));
        assert_eq!(result[2].kind, TokenKind::Eq);
        assert_eq!(result[3].kind, TokenKind::IntegerLiteral(syms[1], None));
    }

    #[test]
    fn test_lexer_nested_block_comment() {
        let source = "let x /* outer /* inner */ still outer */ = 42";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        // Should handle nested comments
        assert_eq!(result.len(), 5);
        assert_eq!(result[0].kind, TokenKind::Let);
        let syms = intern_for_test_many(&["x", "42"]);
        assert_eq!(result[1].kind, TokenKind::Ident(syms[0]));
        assert_eq!(result[3].kind, TokenKind::IntegerLiteral(syms[1], None));
    }

    // ===== Edge Cases =====

    #[test]
    fn test_lexer_zero() {
        let source = "0 00 0.0";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        let syms = intern_for_test_many(&["0", "00", "0.0"]);
        assert_eq!(result[0].kind, TokenKind::IntegerLiteral(syms[0], None));
        assert_eq!(result[1].kind, TokenKind::IntegerLiteral(syms[1], None));
        assert_eq!(result[2].kind, TokenKind::FloatLiteral(syms[2], None));
    }

    #[test]
    fn test_lexer_all_keywords() {
        let source = "let mut fn struct class enum protocol impl return if guard match for while comptime const static pub prv self Self init case";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert_eq!(result[0].kind, TokenKind::Let);
        assert_eq!(result[1].kind, TokenKind::Mut);
        assert_eq!(result[2].kind, TokenKind::Fn);
        assert_eq!(result[3].kind, TokenKind::Struct);
        assert_eq!(result[4].kind, TokenKind::Class);
        assert_eq!(result[5].kind, TokenKind::Enum);
        assert_eq!(result[6].kind, TokenKind::Protocol);
        assert_eq!(result[7].kind, TokenKind::Impl);
        assert_eq!(result[8].kind, TokenKind::Return);
        assert_eq!(result[9].kind, TokenKind::If);
        assert_eq!(result[10].kind, TokenKind::Guard);
        assert_eq!(result[11].kind, TokenKind::Match);
        assert_eq!(result[12].kind, TokenKind::For);
        assert_eq!(result[13].kind, TokenKind::While);
        assert_eq!(result[14].kind, TokenKind::Comptime);
        assert_eq!(result[15].kind, TokenKind::Const);
        assert_eq!(result[16].kind, TokenKind::Static);
        assert_eq!(result[17].kind, TokenKind::Pub);
        assert_eq!(result[18].kind, TokenKind::Prv);
        assert_eq!(result[19].kind, TokenKind::SelfValue);
        assert_eq!(result[20].kind, TokenKind::SelfType);
        assert_eq!(result[21].kind, TokenKind::Init);
        assert_eq!(result[22].kind, TokenKind::Case);
    }

    #[test]
    fn test_lexer_complex_source() {
        let source = r#"
            // Calculate the area of a circle
            fn calculateArea(radius: Float64) -> Float64 {
                let pi = 3.141_592f64
                let squared = radius * radius
                return pi * squared
            }
        "#;

        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        // Should tokenize successfully
        assert!(result.len() > 20);

        // Check for key tokens
        let has_fn = result.iter().any(|t| t.kind == TokenKind::Fn);
        let has_let = result.iter().any(|t| t.kind == TokenKind::Let);
        let has_return = result.iter().any(|t| t.kind == TokenKind::Return);

        assert!(has_fn);
        assert!(has_let);
        assert!(has_return);
    }

    // ===== Additional Keyword Tests =====

    #[test]
    fn test_lexer_keyword_self() {
        let source = "self";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert_eq!(result.len(), 2); // self + EOF
        assert_eq!(result[0].kind, TokenKind::SelfValue);
        assert_eq!(result[1].kind, TokenKind::EOF);
    }

    #[test]
    #[allow(non_snake_case)]
    fn test_lexer_keyword_Self() {
        let source = "Self";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert_eq!(result.len(), 2); // Self + EOF
        assert_eq!(result[0].kind, TokenKind::SelfType);
        assert_eq!(result[1].kind, TokenKind::EOF);
    }

    #[test]
    fn test_lexer_keyword_init() {
        let source = "init";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert_eq!(result.len(), 2); // init + EOF
        assert_eq!(result[0].kind, TokenKind::Init);
        assert_eq!(result[1].kind, TokenKind::EOF);
    }

    #[test]
    fn test_lexer_keyword_case() {
        let source = "case";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert_eq!(result.len(), 2); // case + EOF
        assert_eq!(result[0].kind, TokenKind::Case);
        assert_eq!(result[1].kind, TokenKind::EOF);
    }

    // ===== Additional Numeric Tests =====

    #[test]
    fn test_lexer_max_int64() {
        let source = "9223372036854775807";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert_eq!(
            result[0].kind,
            TokenKind::IntegerLiteral(intern_for_test(source), None)
        );
    }

    #[test]
    fn test_lexer_hex_uppercase() {
        let source = "0XFFAB";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert_eq!(
            result[0].kind,
            TokenKind::IntegerLiteral(intern_for_test(source), None)
        );
    }

    #[test]
    fn test_lexer_binary_uppercase() {
        let source = "0B10101";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert_eq!(
            result[0].kind,
            TokenKind::IntegerLiteral(intern_for_test(source), None)
        );
    }

    #[test]
    fn test_lexer_float_with_exponent_positive() {
        let source = "1.5e+10";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert_eq!(
            result[0].kind,
            TokenKind::FloatLiteral(intern_for_test(source), None)
        );
    }

    #[test]
    fn test_lexer_float_with_exponent_negative() {
        let source = "1.5e-10";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert_eq!(
            result[0].kind,
            TokenKind::FloatLiteral(intern_for_test(source), None)
        );
    }

    #[test]
    fn test_lexer_integer_all_type_suffixes() {
        let sources = vec![
            ("42u8", "u8"),
            ("42u16", "u16"),
            ("42u32", "u32"),
            ("42u64", "u64"),
            ("42i8", "i8"),
            ("42i16", "i16"),
            ("42i32", "i32"),
            ("42i64", "i64"),
        ];

        for (source, suffix) in sources {
            let lexer = Lexer::new(source);
            let result = lexer.lex().unwrap();

            // Lexer interns suffix BEFORE value
            let syms = intern_for_test_many(&[suffix, source]);
            assert_eq!(
                result[0].kind,
                TokenKind::IntegerLiteral(syms[1], Some(syms[0]))
            );
        }
    }

    #[test]
    fn test_lexer_float_all_type_suffixes() {
        let sources = vec![
            ("3.14f32", "f32"),
            ("3.14f64", "f64"),
            ("1e10f32", "f32"),
            ("1e10f64", "f64"),
        ];

        for (source, suffix) in sources {
            let lexer = Lexer::new(source);
            let result = lexer.lex().unwrap();

            // Lexer interns suffix BEFORE value
            let syms = intern_for_test_many(&[suffix, source]);
            assert_eq!(
                result[0].kind,
                TokenKind::FloatLiteral(syms[1], Some(syms[0]))
            );
        }
    }

    // ===== Additional String Tests =====

    #[test]
    fn test_lexer_string_all_escape_sequences() {
        let source = r#""\n\t\r\\0""#;
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert_eq!(
            result[0].kind,
            TokenKind::StringLiteral(intern_for_test(source))
        );
    }

    #[test]
    fn test_lexer_string_with_newline() {
        let source = "\"hello\nworld\"";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        // Should tokenize despite unterminated string
        assert!(matches!(result[0].kind, TokenKind::StringLiteral(_)));
    }

    #[test]
    fn test_lexer_multiple_strings() {
        let source = "\"a\" \"b\" \"c\" \"d\" \"e\"";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert_eq!(result.len(), 6); // 5 strings + EOF
        let syms = intern_for_test_many(&[
            "\"a\"", "\"b\"", "\"c\"", "\"d\"", "\"e\"",
        ]);
        assert_eq!(result[0].kind, TokenKind::StringLiteral(syms[0]));
        assert_eq!(result[1].kind, TokenKind::StringLiteral(syms[1]));
        assert_eq!(result[2].kind, TokenKind::StringLiteral(syms[2]));
        assert_eq!(result[3].kind, TokenKind::StringLiteral(syms[3]));
        assert_eq!(result[4].kind, TokenKind::StringLiteral(syms[4]));
    }

    // ===== Operator Combination Tests =====

    #[test]
    fn test_lexer_all_comparison_operators() {
        let source = "== != < > <= >=";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert_eq!(result[0].kind, TokenKind::EqEq);
        assert_eq!(result[1].kind, TokenKind::BangEq);
        assert_eq!(result[2].kind, TokenKind::LAngle);
        assert_eq!(result[3].kind, TokenKind::RAngle);
        assert_eq!(result[4].kind, TokenKind::LtEq);
        assert_eq!(result[5].kind, TokenKind::GtEq);
    }

    #[test]
    fn test_lexer_all_arithmetic_operators() {
        let source = "+ - * / %";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert_eq!(result[0].kind, TokenKind::Plus);
        assert_eq!(result[1].kind, TokenKind::Minus);
        assert_eq!(result[2].kind, TokenKind::Star);
        assert_eq!(result[3].kind, TokenKind::Slash);
        assert_eq!(result[4].kind, TokenKind::Percent);
    }

    #[test]
    fn test_lexer_all_delimiters() {
        let source = "( ) { } [ ] . , :";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert_eq!(result[0].kind, TokenKind::LParen);
        assert_eq!(result[1].kind, TokenKind::RParen);
        assert_eq!(result[2].kind, TokenKind::LBrace);
        assert_eq!(result[3].kind, TokenKind::RBrace);
        assert_eq!(result[4].kind, TokenKind::LBracket);
        assert_eq!(result[5].kind, TokenKind::RBracket);
        assert_eq!(result[6].kind, TokenKind::Dot);
        assert_eq!(result[7].kind, TokenKind::Comma);
        assert_eq!(result[8].kind, TokenKind::Colon);
    }

    // ===== Whitespace and Newline Tests =====

    #[test]
    fn test_lexer_tabs() {
        let source = "let\t\tx\t=\t42";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert_eq!(result[0].kind, TokenKind::Let);
        assert!(matches!(result[1].kind, TokenKind::Ident(_)));
        assert_eq!(result[2].kind, TokenKind::Eq);
        assert!(matches!(result[3].kind, TokenKind::IntegerLiteral(_, _)));
    }

    #[test]
    fn test_lexer_mixed_whitespace() {
        let source = "let \t x \n = \r\n 42";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert_eq!(result[0].kind, TokenKind::Let);
        assert!(matches!(result[1].kind, TokenKind::Ident(_)));
        assert_eq!(result[2].kind, TokenKind::Eq);
        assert!(matches!(result[3].kind, TokenKind::IntegerLiteral(_, _)));
    }

    #[test]
    fn test_lexer_leading_trailing_whitespace() {
        let source = "   \n\t  let x = 42  \t\n ";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert_eq!(result[0].kind, TokenKind::Let);
    }

    // ===== Identifier Tests =====

    #[test]
    fn test_lexer_underscore_identifier() {
        let source = "_";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        // Underscore is now tokenized as Underscore, not Ident
        assert_eq!(result[0].kind, TokenKind::Underscore);
    }

    #[test]
    fn test_lexer_identifier_with_underscore() {
        let source = "my_variable_name";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert_eq!(result[0].kind, TokenKind::Ident(intern_for_test(source)));
    }

    #[test]
    fn test_lexer_identifier_starting_with_underscore() {
        let sources = vec!["_private", "__internal", "_123"];
        for source in sources {
            let lexer = Lexer::new(source);
            let result = lexer.lex().unwrap();

            assert_eq!(
                result[0].kind,
                TokenKind::Ident(intern_for_test(source))
            );
        }
    }

    #[test]
    fn test_lexer_camel_case_identifier() {
        let source = "CamelCaseIdentifier";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert_eq!(result[0].kind, TokenKind::Ident(intern_for_test(source)));
    }

    #[test]
    fn test_lexer_snake_case_identifier() {
        let source = "snake_case_identifier";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert_eq!(result[0].kind, TokenKind::Ident(intern_for_test(source)));
    }

    // ===== Keyword Individual Tests =====

    #[test]
    fn test_lexer_keyword_let() {
        let lexer = Lexer::new("let");
        assert_eq!(lexer.lex().unwrap()[0].kind, TokenKind::Let);
    }

    #[test]
    fn test_lexer_keyword_mut() {
        let lexer = Lexer::new("mut");
        assert_eq!(lexer.lex().unwrap()[0].kind, TokenKind::Mut);
    }

    #[test]
    fn test_lexer_keyword_fn() {
        let lexer = Lexer::new("fn");
        assert_eq!(lexer.lex().unwrap()[0].kind, TokenKind::Fn);
    }

    #[test]
    fn test_lexer_keyword_struct() {
        let lexer = Lexer::new("struct");
        assert_eq!(lexer.lex().unwrap()[0].kind, TokenKind::Struct);
    }

    #[test]
    fn test_lexer_keyword_class() {
        let lexer = Lexer::new("class");
        assert_eq!(lexer.lex().unwrap()[0].kind, TokenKind::Class);
    }

    #[test]
    fn test_lexer_keyword_return() {
        let lexer = Lexer::new("return");
        assert_eq!(lexer.lex().unwrap()[0].kind, TokenKind::Return);
    }

    #[test]
    fn test_lexer_keyword_if() {
        let lexer = Lexer::new("if");
        assert_eq!(lexer.lex().unwrap()[0].kind, TokenKind::If);
    }

    #[test]
    fn test_lexer_keyword_match() {
        let lexer = Lexer::new("match");
        assert_eq!(lexer.lex().unwrap()[0].kind, TokenKind::Match);
    }

    #[test]
    fn test_lexer_keyword_for() {
        let lexer = Lexer::new("for");
        assert_eq!(lexer.lex().unwrap()[0].kind, TokenKind::For);
    }

    #[test]
    fn test_lexer_keyword_while() {
        let lexer = Lexer::new("while");
        assert_eq!(lexer.lex().unwrap()[0].kind, TokenKind::While);
    }

    // ===== Complex Expression Tests =====

    #[test]
    fn test_lexer_function_call() {
        let source = "functionName(arg1, arg2)";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert!(matches!(result[0].kind, TokenKind::Ident(_)));
        assert_eq!(result[1].kind, TokenKind::LParen);
        assert!(matches!(result[2].kind, TokenKind::Ident(_)));
        assert_eq!(result[3].kind, TokenKind::Comma);
        assert!(matches!(result[4].kind, TokenKind::Ident(_)));
        assert_eq!(result[5].kind, TokenKind::RParen);
    }

    #[test]
    fn test_lexer_array_access() {
        let source = "array[index]";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert!(matches!(result[0].kind, TokenKind::Ident(_)));
        assert_eq!(result[1].kind, TokenKind::LBracket);
        assert!(matches!(result[2].kind, TokenKind::Ident(_)));
        assert_eq!(result[3].kind, TokenKind::RBracket);
    }

    #[test]
    fn test_lexer_field_access() {
        let source = "object.field";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert!(matches!(result[0].kind, TokenKind::Ident(_)));
        assert_eq!(result[1].kind, TokenKind::Dot);
        assert!(matches!(result[2].kind, TokenKind::Ident(_)));
    }

    #[test]
    fn test_lexer_binary_expression() {
        let source = "a + b * c - d / e";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert!(matches!(result[0].kind, TokenKind::Ident(_)));
        assert_eq!(result[1].kind, TokenKind::Plus);
        assert!(matches!(result[2].kind, TokenKind::Ident(_)));
        assert_eq!(result[3].kind, TokenKind::Star);
        assert!(matches!(result[4].kind, TokenKind::Ident(_)));
        assert_eq!(result[5].kind, TokenKind::Minus);
        assert!(matches!(result[6].kind, TokenKind::Ident(_)));
        assert_eq!(result[7].kind, TokenKind::Slash);
        assert!(matches!(result[8].kind, TokenKind::Ident(_)));
    }

    #[test]
    fn test_lexer_comparison_chain() {
        let source = "a == b && c != d";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert!(matches!(result[0].kind, TokenKind::Ident(_)));
        assert_eq!(result[1].kind, TokenKind::EqEq);
        assert!(matches!(result[2].kind, TokenKind::Ident(_)));
        assert_eq!(result[3].kind, TokenKind::AmpAmp);
        assert!(matches!(result[4].kind, TokenKind::Ident(_)));
        assert_eq!(result[5].kind, TokenKind::BangEq);
        assert!(matches!(result[6].kind, TokenKind::Ident(_)));
    }

    #[test]
    fn test_lexer_assignment_with_type() {
        let source = "let x: Int32 = 42";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert_eq!(result[0].kind, TokenKind::Let);
        assert!(matches!(result[1].kind, TokenKind::Ident(_)));
        assert_eq!(result[2].kind, TokenKind::Colon);
        assert!(matches!(result[3].kind, TokenKind::Ident(_)));
        assert_eq!(result[4].kind, TokenKind::Eq);
        assert!(matches!(result[5].kind, TokenKind::IntegerLiteral(_, _)));
    }

    // ===== Edge Case Tests =====

    #[test]
    fn test_lexer_number_followed_by_identifier() {
        let source = "123identifier";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        // Should treat as number followed by identifier (error recovery)
        assert!(result.len() > 1);
    }

    #[test]
    fn test_lexer_only_whitespace() {
        let source = "   \t\n\r\n   ";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert_eq!(result.len(), 1); // Only EOF
        assert_eq!(result[0].kind, TokenKind::EOF);
    }

    #[test]
    fn test_lexer_nil_literal() {
        let lexer = Lexer::new("nil");
        assert_eq!(lexer.lex().unwrap()[0].kind, TokenKind::Nil);
    }

    #[test]
    fn test_lexer_true_literal() {
        let lexer = Lexer::new("true");
        assert_eq!(lexer.lex().unwrap()[0].kind, TokenKind::BoolLiteral(true));
    }

    #[test]
    fn test_lexer_false_literal() {
        let lexer = Lexer::new("false");
        assert_eq!(lexer.lex().unwrap()[0].kind, TokenKind::BoolLiteral(false));
    }

    #[test]
    fn test_lexer_double_colon_path() {
        let source = "Module::Type::method";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert!(matches!(result[0].kind, TokenKind::Ident(_)));
        assert_eq!(result[1].kind, TokenKind::ColonColon);
        assert!(matches!(result[2].kind, TokenKind::Ident(_)));
        assert_eq!(result[3].kind, TokenKind::ColonColon);
        assert!(matches!(result[4].kind, TokenKind::Ident(_)));
    }

    #[test]
    fn test_lexer_fat_arrow() {
        let source = "key => value";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert!(matches!(result[0].kind, TokenKind::Ident(_)));
        assert_eq!(result[1].kind, TokenKind::FatArrow);
        assert!(matches!(result[2].kind, TokenKind::Ident(_)));
    }

    #[test]
    fn test_lexer_thin_arrow() {
        let source = "fn -> return";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert_eq!(result[0].kind, TokenKind::Fn);
        assert_eq!(result[1].kind, TokenKind::Arrow);
        assert_eq!(result[2].kind, TokenKind::Return);
    }

    #[test]
    fn test_lexer_zero_hex() {
        let source = "0x0";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert_eq!(
            result[0].kind,
            TokenKind::IntegerLiteral(intern_for_test(source), None)
        );
    }

    #[test]
    fn test_lexer_zero_binary() {
        let source = "0b0";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert_eq!(
            result[0].kind,
            TokenKind::IntegerLiteral(intern_for_test(source), None)
        );
    }

    #[test]
    fn test_lexer_large_hex() {
        let source = "0xFFFFFFFFFFFFFFFF";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert_eq!(
            result[0].kind,
            TokenKind::IntegerLiteral(intern_for_test(source), None)
        );
    }

    #[test]
    fn test_lexer_negative_number() {
        let source = "-42";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        // Negative sign is separate token
        assert_eq!(result[0].kind, TokenKind::Minus);
        assert!(matches!(result[1].kind, TokenKind::IntegerLiteral(_, _)));
    }

    #[test]
    fn test_lexer_negated_expression() {
        let source = "!true";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert_eq!(result[0].kind, TokenKind::Bang);
        assert_eq!(result[1].kind, TokenKind::BoolLiteral(true));
    }

    #[test]
    fn test_lexer_parenthesized_expression() {
        let source = "(a + b)";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert_eq!(result[0].kind, TokenKind::LParen);
        assert!(matches!(result[1].kind, TokenKind::Ident(_)));
        assert_eq!(result[2].kind, TokenKind::Plus);
        assert!(matches!(result[3].kind, TokenKind::Ident(_)));
        assert_eq!(result[4].kind, TokenKind::RParen);
    }

    #[test]
    fn test_lexer_empty_block() {
        let source = "{}";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert_eq!(result[0].kind, TokenKind::LBrace);
        assert_eq!(result[1].kind, TokenKind::RBrace);
    }

    #[test]
    fn test_lexer_empty_array() {
        let source = "[]";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert_eq!(result[0].kind, TokenKind::LBracket);
        assert_eq!(result[1].kind, TokenKind::RBracket);
    }

    #[test]
    fn test_lexer_array_with_multiple_elements() {
        let source = "[1, 2, 3, 4, 5]";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert_eq!(result[0].kind, TokenKind::LBracket);
        assert!(matches!(result[1].kind, TokenKind::IntegerLiteral(_, _)));
        assert_eq!(result[2].kind, TokenKind::Comma);
        assert!(matches!(result[3].kind, TokenKind::IntegerLiteral(_, _)));
        assert_eq!(result[4].kind, TokenKind::Comma);
        assert!(matches!(result[5].kind, TokenKind::IntegerLiteral(_, _)));
        assert_eq!(result[6].kind, TokenKind::Comma);
        assert!(matches!(result[7].kind, TokenKind::IntegerLiteral(_, _)));
        assert_eq!(result[8].kind, TokenKind::Comma);
        assert!(matches!(result[9].kind, TokenKind::IntegerLiteral(_, _)));
        assert_eq!(result[10].kind, TokenKind::RBracket);
    }

    #[test]
    fn test_lexer_tuple() {
        let source = "(1, \"hello\", true)";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert_eq!(result[0].kind, TokenKind::LParen);
        assert!(matches!(result[1].kind, TokenKind::IntegerLiteral(_, _)));
        assert_eq!(result[2].kind, TokenKind::Comma);
        assert!(matches!(result[3].kind, TokenKind::StringLiteral(_)));
        assert_eq!(result[4].kind, TokenKind::Comma);
        assert_eq!(result[5].kind, TokenKind::BoolLiteral(true));
        assert_eq!(result[6].kind, TokenKind::RParen);
    }

    #[test]
    fn test_lexer_guard_keyword() {
        let lexer = Lexer::new("guard");
        assert_eq!(lexer.lex().unwrap()[0].kind, TokenKind::Guard);
    }

    #[test]
    fn test_lexer_comptime_keyword() {
        let lexer = Lexer::new("comptime");
        assert_eq!(lexer.lex().unwrap()[0].kind, TokenKind::Comptime);
    }

    #[test]
    fn test_lexer_const_keyword() {
        let lexer = Lexer::new("const");
        assert_eq!(lexer.lex().unwrap()[0].kind, TokenKind::Const);
    }

    #[test]
    fn test_lexer_static_keyword() {
        let lexer = Lexer::new("static");
        assert_eq!(lexer.lex().unwrap()[0].kind, TokenKind::Static);
    }

    #[test]
    fn test_lexer_pub_keyword() {
        let lexer = Lexer::new("pub");
        assert_eq!(lexer.lex().unwrap()[0].kind, TokenKind::Pub);
    }

    #[test]
    fn test_lexer_prv_keyword() {
        let lexer = Lexer::new("prv");
        assert_eq!(lexer.lex().unwrap()[0].kind, TokenKind::Prv);
    }

    #[test]
    fn test_lexer_enum_keyword() {
        let lexer = Lexer::new("enum");
        assert_eq!(lexer.lex().unwrap()[0].kind, TokenKind::Enum);
    }

    #[test]
    fn test_lexer_protocol_keyword() {
        let lexer = Lexer::new("protocol");
        assert_eq!(lexer.lex().unwrap()[0].kind, TokenKind::Protocol);
    }

    #[test]
    fn test_lexer_impl_keyword() {
        let lexer = Lexer::new("impl");
        assert_eq!(lexer.lex().unwrap()[0].kind, TokenKind::Impl);
    }

    // ===== Additional Edge Cases =====

    #[test]
    fn test_lexer_float_with_underscores() {
        let source = "3.141_592_653";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert_eq!(
            result[0].kind,
            TokenKind::FloatLiteral(intern_for_test(source), None)
        );
    }

    #[test]
    fn test_lexer_hex_with_underscores() {
        let source = "0xFFFF_FFFF";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert_eq!(
            result[0].kind,
            TokenKind::IntegerLiteral(intern_for_test(source), None)
        );
    }

    #[test]
    fn test_lexer_binary_with_underscores() {
        let source = "0b1010_1010";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert_eq!(
            result[0].kind,
            TokenKind::IntegerLiteral(intern_for_test(source), None)
        );
    }

    #[test]
    fn test_lexer_complex_expression() {
        let source = "((a + b) * (c - d)) / e";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        // Should handle nested parentheses
        // Source is: ((a + b) * (c - d)) / e
        // Which has: 3 left parens, 3 right parens
        let lparens = result
            .iter()
            .filter(|t| t.kind == TokenKind::LParen)
            .count();
        let rparens = result
            .iter()
            .filter(|t| t.kind == TokenKind::RParen)
            .count();
        assert_eq!(lparens, 3);
        assert_eq!(rparens, 3);
    }

    #[test]
    fn test_lexer_very_long_identifier() {
        let source =
            "this_is_a_very_long_identifier_name_with_many_underscores";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert_eq!(result[0].kind, TokenKind::Ident(intern_for_test(source)));
    }

    #[test]
    fn test_lexer_string_with_quotes() {
        let source = r#""He said \"hello\"""#;
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert_eq!(
            result[0].kind,
            TokenKind::StringLiteral(intern_for_test(source))
        );
    }

    #[test]
    fn test_lexer_scientific_notation_uppercase() {
        let source = "1.5E10";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert_eq!(
            result[0].kind,
            TokenKind::FloatLiteral(intern_for_test(source), None)
        );
    }

    #[test]
    fn test_lexer_zero_dot_zero() {
        let source = "0.0";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert_eq!(
            result[0].kind,
            TokenKind::FloatLiteral(intern_for_test(source), None)
        );
    }

    #[test]
    fn test_lexer_dot_as_field_access() {
        let source = "obj . field";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert!(matches!(result[0].kind, TokenKind::Ident(_)));
        assert_eq!(result[1].kind, TokenKind::Dot);
        assert!(matches!(result[2].kind, TokenKind::Ident(_)));
    }

    #[test]
    fn test_lexer_comment_before_code() {
        let source = "// comment\nlet x = 42";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        // Should skip comment and tokenize let statement
        assert_eq!(result[0].kind, TokenKind::Let);
    }

    #[test]
    fn test_lexer_multiple_keywords_in_sequence() {
        let source = "let mut fn return";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        assert_eq!(result[0].kind, TokenKind::Let);
        assert_eq!(result[1].kind, TokenKind::Mut);
        assert_eq!(result[2].kind, TokenKind::Fn);
        assert_eq!(result[3].kind, TokenKind::Return);
    }

    #[test]
    fn test_lexer_all_keywords_combined() {
        let source = "let mut fn struct class enum protocol impl return if guard match for while comptime const static pub prv";
        let lexer = Lexer::new(source);
        let result = lexer.lex().unwrap();

        // Should tokenize all keywords
        let keyword_count = result
            .iter()
            .take(19)
            .filter(|t| t.kind.is_keyword())
            .count();
        assert_eq!(keyword_count, 19);
    }
}
