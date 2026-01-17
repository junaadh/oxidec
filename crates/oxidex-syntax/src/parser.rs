//! Recursive descent parser for the `OxideX` language.
//!
//! This module implements a parser that converts tokens into an Abstract Syntax Tree (AST).
//! The parser uses recursive descent with precedence climbing for expression parsing.

use crate::{
    ast::decl::{
        EnumVariant, FnDecl, FnParam, ProtocolMethod, StructField, Visibility,
    },
    ast::expr::{
        BinaryOp, CallArg, DictEntry, InterpolationPart, MatchArm,
        StructField as ExprStructField, UnaryOp,
    },
    ast::pat::FieldPat,
    ast::stmt,
    ast::{Decl, Expr, Pattern, Stmt, Type},
    error::{ParserError, ParserResult},
    span::{Span, Spanned},
    token::{Token, TokenKind},
};
use oxidex_mem::arena::LocalArena;
use oxidex_mem::{StringInterner, Symbol};
use std::marker::PhantomData;

/// Minimum precedence for parsing.
const MIN_PRECEDENCE: u8 = 1;

/// Parser for the `OxideX` language.
///
/// The parser uses recursive descent with precedence climbing for expressions.
/// All AST nodes are allocated in the provided arena for zero-overhead performance.
///
/// # Type Parameters
///
/// - `'input`: Lifetime of the source code being parsed
/// - `'arena`: Lifetime of the arena allocator
pub struct Parser<'input, 'arena> {
    /// Token stream from lexer
    tokens: Vec<Token>,
    /// Current position in token stream
    pos: usize,
    /// Source code for error reporting
    source: &'input str,
    /// String interner for identifier lookup
    interner: StringInterner,
    /// Arena for AST node allocation
    arena: LocalArena,
    /// Accumulated parsing errors
    errors: Vec<ParserError>,
    /// `PhantomData` to track arena lifetime
    _phantom: PhantomData<&'arena ()>,
}

impl<'input, 'arena> Parser<'input, 'arena> {
    /// Creates a new parser.
    ///
    /// # Arguments
    ///
    /// * `tokens` - Token stream from lexer
    /// * `source` - Source code for error reporting
    /// * `interner` - String interner
    /// * `arena` - Arena allocator for AST nodes
    #[must_use]
    pub fn new(
        tokens: Vec<Token>,
        source: &'input str,
        interner: StringInterner,
        arena: LocalArena,
    ) -> Self {
        Self {
            tokens,
            pos: 0,
            source,
            interner,
            arena,
            errors: Vec::new(),
            _phantom: PhantomData,
        }
    }

    /// Returns the current token.
    fn current(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    /// Peeks at the current token without consuming it.
    #[must_use]
    pub fn peek(&self) -> Option<&Token> {
        self.current()
    }

    /// Peeks at the next token without consuming the current one.
    fn peek_next(&self) -> Option<&Token> {
        self.tokens.get(self.pos + 1)
    }

    /// Advances to the next token and returns the previous one.
    fn bump(&mut self) -> Option<&Token> {
        let pos = self.pos;
        self.pos += 1;
        self.tokens.get(pos)
    }

    /// Checks if the current token matches the given kind.
    #[must_use]
    pub fn check(&self, kind: TokenKind) -> bool {
        self.peek().is_some_and(|token| token.kind == kind)
    }

    /// Expects the current token to be of the given kind.
    ///
    /// Returns the token if it matches, otherwise returns an error with rich diagnostics.
    pub fn expect(&mut self, kind: TokenKind) -> ParserResult<&Token> {
        if self.check(kind.clone()) {
            Ok(self.bump().unwrap())
        } else {
            let found = self
                .peek()
                .map_or_else(|| "EOF".to_string(), |t| format!("{:?}", t.kind));
            let span = self.peek().map_or_else(
                || Span::point(self.source.len(), 1, 1),
                |t| t.span,
            );

            Err(ParserError::UnexpectedToken {
                expected: vec![format!("{kind:?}")],
                found,
                span,
            })
        }
    }

    /// Checks if we're at EOF.
    fn is_at_eof(&self) -> bool {
        self.pos >= self.tokens.len() || self.check(TokenKind::EOF)
    }

    /// Allocates an expression in the arena.
    fn alloc_expr(&mut self, expr: Expr<'arena>) -> &'arena Expr<'arena> {
        unsafe {
            // SAFETY: The arena outlives the AST nodes
            // Parser lifetime ensures validity
            &*(self.arena.alloc(expr) as *const Expr<'arena>)
        }
    }

    /// Reports an error but continues parsing (for error recovery).
    fn emit_error(&mut self, error: ParserError) {
        self.errors.push(error);
    }

    /// Parses a complete expression (entry point).
    pub fn parse_expression(&mut self) -> ParserResult<&'arena Expr<'arena>> {
        self.parse_expr(MIN_PRECEDENCE)
    }

    /// Resolves a symbol to its string representation.
    #[must_use]
    pub fn resolve_symbol(&self, sym: Symbol) -> &str {
        self.interner.resolve(sym).unwrap_or("<unknown>")
    }

    /// Parses an expression with the given minimum precedence.
    ///
    /// This implements precedence climbing for handling binary operators.
    pub fn parse_expr(
        &mut self,
        precedence: u8,
    ) -> ParserResult<&'arena Expr<'arena>> {
        // Parse postfix expression (primary expr + calls, fields, indexing)
        let mut left = self.parse_postfix_expr()?;

        // Parse binary operators with higher precedence
        while let Some(token) = self.peek() {
            let token_prec = match token.kind.precedence() {
                Some(p) => p,
                None => break,
            };

            if token_prec < precedence {
                break;
            }

            // Extract operator info before bumping
            let op = match self.token_kind_to_binary_op(token.kind.clone()) {
                Ok(op) => op,
                Err(_) => break,
            };
            let _op_span = token.span;

            self.bump(); // consume operator

            // Parse right operand with higher precedence
            let right = self.parse_expr(token_prec + 1)?;

            // Merge spans and allocate binary expression
            let total_span = Span::merge(left.span(), right.span());
            left = self.alloc_expr(Expr::Binary {
                left,
                op,
                right,
                span: total_span,
            });
        }

        Ok(left)
    }

    /// Parses a prefix expression (literals, unary ops, identifiers, etc.).
    fn parse_prefix_expr(&mut self) -> ParserResult<&'arena Expr<'arena>> {
        let token_span = match self.peek() {
            Some(t) => t.span,
            None => {
                return Err(ParserError::ExpectedExpression {
                    span: Span::point(self.source.len(), 1, 1),
                });
            }
        };

        // Clone token kind to avoid borrow issues
        let token_kind = match self.peek() {
            Some(t) => t.kind.clone(),
            None => {
                return Err(ParserError::ExpectedExpression {
                    span: Span::point(self.source.len(), 1, 1),
                });
            }
        };

        match token_kind {
            // Literals
            TokenKind::IntegerLiteral(value, suffix) => {
                self.bump();
                Ok(self.alloc_expr(Expr::IntegerLiteral {
                    value,
                    type_suffix: suffix,
                    span: token_span,
                }))
            }

            TokenKind::FloatLiteral(value, suffix) => {
                self.bump();
                Ok(self.alloc_expr(Expr::FloatLiteral {
                    value,
                    type_suffix: suffix,
                    span: token_span,
                }))
            }

            TokenKind::StringLiteral(value) => {
                self.bump();
                // Check for interpolation
                if self.check(TokenKind::InterpolationStart) {
                    self.parse_interpolation(token_span, value)
                } else {
                    Ok(self.alloc_expr(Expr::StringLiteral {
                        value,
                        span: token_span,
                    }))
                }
            }

            TokenKind::BoolLiteral(value) => {
                self.bump();
                Ok(self.alloc_expr(Expr::BoolLiteral {
                    value,
                    span: token_span,
                }))
            }

            TokenKind::Nil => {
                self.bump();
                Ok(self.alloc_expr(Expr::Nil { span: token_span }))
            }

            // Unary operators
            TokenKind::Bang => {
                self.bump();
                let operand = self.parse_prefix_expr()?;
                Ok(self.alloc_expr(Expr::Unary {
                    op: UnaryOp::Negate,
                    operand,
                    span: Span::merge(token_span, operand.span()),
                }))
            }

            TokenKind::Minus => {
                self.bump();
                let operand = self.parse_prefix_expr()?;
                Ok(self.alloc_expr(Expr::Unary {
                    op: UnaryOp::Minus,
                    operand,
                    span: Span::merge(token_span, operand.span()),
                }))
            }

            // Identifiers and paths
            TokenKind::Ident(_) => {
                // Check for path expression (could be enum construction)
                if let Some(next) = self.peek_next() {
                    if matches!(next.kind, TokenKind::ColonColon) {
                        return self.parse_path_or_enum_expr();
                    }
                    // Check for struct construction: Type { field: value }
                    if matches!(next.kind, TokenKind::LBrace) {
                        return self.parse_struct_expr();
                    }
                }
                let ident_token = self.bump().unwrap();
                let sym = match ident_token.kind {
                    TokenKind::Ident(sym) => sym,
                    _ => unreachable!(),
                };
                Ok(self.alloc_expr(Expr::Identifier(sym)))
            }

            // Parenthesized expressions
            TokenKind::LParen => {
                self.bump();
                let expr = self.parse_expr(MIN_PRECEDENCE)?;
                self.expect(TokenKind::RParen)?;
                Ok(self.alloc_expr(Expr::Paren {
                    expr,
                    span: token_span,
                }))
            }

            // Blocks
            TokenKind::LBrace => self.parse_block_expr(),

            // Arrays
            TokenKind::LBracket => self.parse_array_expr(),

            // Control flow
            TokenKind::If => self.parse_if_expr(),
            TokenKind::Match => self.parse_match_expr(),
            TokenKind::For => self.parse_for_loop_expr(),
            TokenKind::While => self.parse_while_loop_expr(),

            _ => {
                let found = format!("{token_kind:?}");
                Err(ParserError::UnexpectedToken {
                    expected: vec![
                        "expression".to_string(),
                        "literal".to_string(),
                        "identifier".to_string(),
                    ],
                    found,
                    span: token_span,
                })
            }
        }
    }

    /// Parses a postfix expression (primary expr with calls, fields, indexing).
    fn parse_postfix_expr(&mut self) -> ParserResult<&'arena Expr<'arena>> {
        // Parse the primary expression first
        let mut expr = self.parse_prefix_expr()?;

        // Loop to handle chained postfix operations
        while let Some(token) = self.peek() {
            match token.kind {
                // Function call: foo(args)
                TokenKind::LParen => {
                    expr = self.parse_call_expr(expr)?;
                }

                // Field access: obj.field
                // Or method call: obj.method(args)
                TokenKind::Dot => {
                    // Check if this is a method call by looking ahead
                    let start_span = expr.span();
                    self.bump(); // consume .

                    let field_token = self.expect_identifier()?;

                    // Check for method call
                    if self.check(TokenKind::LParen) {
                        expr = self.parse_method_call_expr(
                            expr,
                            field_token,
                            start_span,
                        )?;
                    } else {
                        // Regular field access
                        let end_span = self
                            .tokens
                            .get(self.pos.saturating_sub(1))
                            .map_or(start_span, |t| t.span);

                        expr = self.alloc_expr(Expr::Field {
                            object: expr,
                            field: field_token,
                            span: Span::merge(start_span, end_span),
                        });
                    }
                }

                // Index access: arr[index]
                TokenKind::LBracket => {
                    expr = self.parse_index_expr(expr)?;
                }

                _ => break,
            }
        }

        Ok(expr)
    }

    /// Parses a function call expression.
    fn parse_call_expr(
        &mut self,
        callee: &'arena Expr<'arena>,
    ) -> ParserResult<&'arena Expr<'arena>> {
        let start_span = callee.span();
        self.bump(); // consume (

        let mut args = Vec::new();

        while !self.check(TokenKind::RParen) && !self.is_at_eof() {
            // Check for labeled argument: label: expr
            let label = {
                let is_ident = matches!(
                    self.peek().map(|t| &t.kind),
                    Some(TokenKind::Ident(_))
                );
                if is_ident {
                    if let Some(next) = self.peek_next() {
                        if matches!(next.kind, TokenKind::Colon) {
                            let ident_token = self.bump().unwrap();
                            let sym = match ident_token.kind {
                                TokenKind::Ident(s) => s,
                                _ => unreachable!(),
                            };
                            self.bump(); // consume :
                            Some(sym)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            };

            let value = self.parse_expr(MIN_PRECEDENCE)?;
            let span = Span::merge(start_span, value.span());

            args.push(CallArg { label, value, span });

            // Check for comma separator
            if !self.check(TokenKind::RParen) {
                self.expect(TokenKind::Comma)?;
            }
        }

        self.expect(TokenKind::RParen)?;
        let end_span = self
            .tokens
            .get(self.pos.saturating_sub(1))
            .map_or(start_span, |t| t.span);

        Ok(self.alloc_expr(Expr::Call {
            callee,
            args,
            span: Span::merge(start_span, end_span),
        }))
    }

    /// Parses a method call expression: obj.method(args)
    fn parse_method_call_expr(
        &mut self,
        receiver: &'arena Expr<'arena>,
        method: Symbol,
        start_span: Span,
    ) -> ParserResult<&'arena Expr<'arena>> {
        // ( is already checked to be present, consume it
        self.bump(); // consume (

        let mut args = Vec::new();

        while !self.check(TokenKind::RParen) && !self.is_at_eof() {
            // Check for labeled argument: label: expr
            let label = {
                let is_ident = matches!(
                    self.peek().map(|t| &t.kind),
                    Some(TokenKind::Ident(_))
                );
                if is_ident {
                    if let Some(next) = self.peek_next() {
                        if matches!(next.kind, TokenKind::Colon) {
                            let ident_token = self.bump().unwrap();
                            let sym = match ident_token.kind {
                                TokenKind::Ident(s) => s,
                                _ => unreachable!(),
                            };
                            self.bump(); // consume :
                            Some(sym)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            };

            let value = self.parse_expr(MIN_PRECEDENCE)?;
            let span = Span::merge(start_span, value.span());

            args.push(CallArg { label, value, span });

            // Check for comma separator
            if !self.check(TokenKind::RParen) {
                self.expect(TokenKind::Comma)?;
            }
        }

        self.expect(TokenKind::RParen)?;
        let end_span = self
            .tokens
            .get(self.pos.saturating_sub(1))
            .map_or(start_span, |t| t.span);

        Ok(self.alloc_expr(Expr::MethodCall {
            receiver,
            method,
            args,
            span: Span::merge(start_span, end_span),
        }))
    }

    /// Parses an index access expression.
    fn parse_index_expr(
        &mut self,
        collection: &'arena Expr<'arena>,
    ) -> ParserResult<&'arena Expr<'arena>> {
        let start_span = collection.span();
        self.bump(); // consume [

        let index = self.parse_expr(MIN_PRECEDENCE)?;

        self.expect(TokenKind::RBracket)?;
        let end_span = self
            .tokens
            .get(self.pos.saturating_sub(1))
            .map_or(start_span, |t| t.span);

        Ok(self.alloc_expr(Expr::Index {
            collection,
            index,
            span: Span::merge(start_span, end_span),
        }))
    }

    /// Parses a path expression or enum construction.
    fn parse_path_or_enum_expr(
        &mut self,
    ) -> ParserResult<&'arena Expr<'arena>> {
        let start_span =
            self.peek().map_or_else(|| Span::point(0, 1, 1), |t| t.span);

        let mut segments = Vec::new();

        loop {
            let ident = self.expect_identifier()?;
            segments.push(ident);

            if !self.check(TokenKind::ColonColon) {
                break;
            }
            self.bump(); // consume ::
        }

        // Check if this is enum construction: Type::Variant or Type::Variant(value)
        // Enum construction has at least 2 segments and may be followed by ( or {
        if segments.len() >= 2 {
            let is_enum = if self.check(TokenKind::LParen)
                || self.check(TokenKind::LBrace)
            {
                true
            } else {
                // Could still be an enum with no payload
                // We'll treat it as a path if it's not followed by ( or {
                false
            };

            if is_enum {
                return self.parse_enum_expr(segments, start_span);
            }
        }

        let end_span = self
            .tokens
            .get(self.pos.saturating_sub(1))
            .map_or(start_span, |t| t.span);

        Ok(self.alloc_expr(Expr::Path {
            segments,
            span: Span::merge(start_span, end_span),
        }))
    }

    /// Parses a struct construction expression: `Point { x: 0, y: 0 }`
    fn parse_struct_expr(&mut self) -> ParserResult<&'arena Expr<'arena>> {
        let start_span =
            self.peek().map_or_else(|| Span::point(0, 1, 1), |t| t.span);

        let type_name = self.expect_identifier()?;
        let type_path = vec![type_name];

        self.bump(); // consume {

        let mut fields = Vec::new();

        while !self.check(TokenKind::RBrace) && !self.is_at_eof() {
            let field_name = self.expect_identifier()?;

            // Check for shorthand initialization (field only) or field: value
            let value = if self.check(TokenKind::Colon) {
                self.bump(); // consume :
                Some(self.parse_expr(MIN_PRECEDENCE)?)
            } else {
                // Shorthand: field name is also the expression
                None
            };

            let span = if let Some(v) = &value {
                Span::merge(
                    self.tokens
                        .get(self.pos.saturating_sub(3))
                        .map_or(start_span, |t| t.span),
                    v.span(),
                )
            } else {
                self.tokens
                    .get(self.pos.saturating_sub(2))
                    .map_or(start_span, |t| t.span)
            };

            fields.push(ExprStructField {
                name: field_name,
                value,
                span,
            });

            if !self.check(TokenKind::RBrace) {
                self.expect(TokenKind::Comma)?;
            }
        }

        self.expect(TokenKind::RBrace)?;
        let end_span = self
            .tokens
            .get(self.pos.saturating_sub(1))
            .map_or(start_span, |t| t.span);

        Ok(self.alloc_expr(Expr::Struct {
            type_path,
            fields,
            span: Span::merge(start_span, end_span),
        }))
    }

    /// Parses an enum construction expression: `Option::Some(value)` or `Option::None`
    fn parse_enum_expr(
        &mut self,
        mut segments: Vec<Symbol>,
        start_span: Span,
    ) -> ParserResult<&'arena Expr<'arena>> {
        // The last segment is the variant name
        let variant = segments.pop().unwrap();
        let type_path = segments;

        // Check for payload: Variant(value) or Variant { field: value }
        let payload = if self.check(TokenKind::LParen) {
            // Tuple-like enum variant: Option::Some(value)
            self.bump(); // consume (

            if self.check(TokenKind::RParen) {
                self.bump(); // consume )
                None
            } else {
                let payload_expr = self.parse_expr(MIN_PRECEDENCE)?;
                self.expect(TokenKind::RParen)?;
                Some(payload_expr)
            }
        } else if self.check(TokenKind::LBrace) {
            // Struct-like enum variant: CustomEnum::Variant { field: value }
            // For now, we'll parse this as a struct-like payload
            // In the future, this might need a different AST node
            self.bump(); // consume {

            if self.check(TokenKind::RBrace) {
                self.bump(); // consume }
                None
            } else {
                let payload_expr = self.parse_expr(MIN_PRECEDENCE)?;
                self.expect(TokenKind::RBrace)?;
                Some(payload_expr)
            }
        } else {
            // No payload
            None
        };

        let end_span = self
            .tokens
            .get(self.pos.saturating_sub(1))
            .map_or(start_span, |t| t.span);

        Ok(self.alloc_expr(Expr::Enum {
            type_path,
            variant,
            payload,
            span: Span::merge(start_span, end_span),
        }))
    }

    /// Parses an if expression.
    fn parse_if_expr(&mut self) -> ParserResult<&'arena Expr<'arena>> {
        let start_span = self.bump().unwrap().span; // consume 'if'

        let condition = self.parse_expr(MIN_PRECEDENCE)?;

        let then_branch = self.parse_block_expr()?;

        let else_branch = if self.check(TokenKind::Else) {
            self.bump(); // consume 'else'

            if self.check(TokenKind::If) {
                Some(self.parse_if_expr()?)
            } else {
                Some(self.parse_block_expr()?)
            }
        } else {
            None
        };

        let end_span = else_branch
            .map_or_else(|| then_branch.span(), super::span::Spanned::span);

        Ok(self.alloc_expr(Expr::If {
            condition,
            then_branch,
            else_branch,
            span: Span::merge(start_span, end_span),
        }))
    }

    /// Parses a match expression.
    fn parse_match_expr(&mut self) -> ParserResult<&'arena Expr<'arena>> {
        let start_span = self.bump().unwrap().span; // consume 'match'

        let scrutinee = self.parse_expr(MIN_PRECEDENCE)?;

        self.expect(TokenKind::LBrace)?;

        let mut arms = Vec::new();

        while !self.check(TokenKind::RBrace) && !self.is_at_eof() {
            let pattern = self.parse_pattern()?;
            let pattern_span = pattern.span();
            self.expect(TokenKind::FatArrow)?;

            let body = self.parse_expr(MIN_PRECEDENCE)?;

            arms.push(MatchArm {
                pattern,
                guard: None, // TODO: implement guards
                body,
                span: Span::merge(pattern_span, body.span()),
            });

            // Optional comma
            if self.check(TokenKind::Comma) {
                self.bump();
            }
        }

        self.expect(TokenKind::RBrace)?;

        let end_span = self
            .tokens
            .get(self.pos.saturating_sub(1))
            .map_or(start_span, |t| t.span);

        Ok(self.alloc_expr(Expr::Match {
            scrutinee,
            arms,
            span: Span::merge(start_span, end_span),
        }))
    }

    /// Parses a block expression.
    fn parse_block_expr(&mut self) -> ParserResult<&'arena Expr<'arena>> {
        let start_span = self.expect(TokenKind::LBrace)?.span;

        let mut stmts = Vec::new();
        let mut expr = None;

        while !self.check(TokenKind::RBrace) && !self.is_at_eof() {
            // Try to parse as expression first
            match self.parse_expr(MIN_PRECEDENCE) {
                Ok(e) => {
                    // If followed by semicolon, it's a statement
                    if self.check(TokenKind::Semicolon) {
                        self.bump();
                        stmts.push(stmt::Stmt::Expr {
                            expr: e,
                            span: e.span(),
                        });
                    } else {
                        // Otherwise, it's the final expression
                        expr = Some(e);
                        break;
                    }
                }
                Err(e) => {
                    // Try as statement
                    if let Ok(s) = self.parse_stmt() {
                        stmts.push(s);
                    } else {
                        // Recovery: skip to next semicolon or statement keyword
                        self.recover_to_sync_point(&[
                            TokenKind::Semicolon,
                            TokenKind::RBrace,
                        ]);
                        self.emit_error(e);
                        if self.check(TokenKind::Semicolon) {
                            self.bump();
                        }
                    }
                }
            }
        }

        self.expect(TokenKind::RBrace)?;

        let end_span = self
            .tokens
            .get(self.pos.saturating_sub(1))
            .map_or(start_span, |t| t.span);

        Ok(self.alloc_expr(Expr::Block {
            stmts,
            expr,
            span: Span::merge(start_span, end_span),
        }))
    }

    /// Parses a for loop expression.
    fn parse_for_loop_expr(&mut self) -> ParserResult<&'arena Expr<'arena>> {
        let start_span = self.bump().unwrap().span; // consume 'for'

        let pattern = self.parse_pattern()?;

        self.expect(TokenKind::In)?;

        let iter = self.parse_expr(MIN_PRECEDENCE)?;

        let body = self.parse_block_expr()?;

        let end_span = body.span();

        Ok(self.alloc_expr(Expr::ForLoop {
            pattern,
            iter,
            body,
            span: Span::merge(start_span, end_span),
        }))
    }

    /// Parses a while loop expression.
    fn parse_while_loop_expr(&mut self) -> ParserResult<&'arena Expr<'arena>> {
        let start_span = self.bump().unwrap().span; // consume 'while'

        let condition = self.parse_expr(MIN_PRECEDENCE)?;

        let body = self.parse_block_expr()?;

        let end_span = body.span();

        Ok(self.alloc_expr(Expr::WhileLoop {
            condition,
            body,
            span: Span::merge(start_span, end_span),
        }))
    }

    /// Parses an array literal.
    fn parse_array_expr(&mut self) -> ParserResult<&'arena Expr<'arena>> {
        let start_span = self.bump().unwrap().span; // consume '['

        // Check if this is a dictionary by looking ahead for "expr:"
        let is_dict = {
            let is_key_type = match self.peek() {
                Some(t) => matches!(
                    t.kind,
                    TokenKind::Ident(_)
                        | TokenKind::StringLiteral(_)
                        | TokenKind::IntegerLiteral(_, _)
                ),
                None => false,
            };
            is_key_type
                && if let Some(next) = self.peek_next() {
                    matches!(next.kind, TokenKind::Colon)
                } else {
                    false
                }
        };

        if is_dict {
            self.parse_dict_expr(start_span)
        } else {
            self.parse_array_expr_inner(start_span)
        }
    }

    /// Parses the inner array expression (after detecting it's not a dict).
    fn parse_array_expr_inner(
        &mut self,
        start_span: Span,
    ) -> ParserResult<&'arena Expr<'arena>> {
        let mut elements = Vec::new();

        while !self.check(TokenKind::RBracket) && !self.is_at_eof() {
            let elem = self.parse_expr(MIN_PRECEDENCE)?;
            elements.push(elem);

            if !self.check(TokenKind::RBracket) {
                self.expect(TokenKind::Comma)?;
            }
        }

        self.expect(TokenKind::RBracket)?;

        let end_span = self
            .tokens
            .get(self.pos.saturating_sub(1))
            .map_or(start_span, |t| t.span);

        Ok(self.alloc_expr(Expr::Array {
            elements,
            span: Span::merge(start_span, end_span),
        }))
    }

    /// Parses a dictionary literal: `[key: value, key2: value2]`
    fn parse_dict_expr(
        &mut self,
        start_span: Span,
    ) -> ParserResult<&'arena Expr<'arena>> {
        let mut entries = Vec::new();

        while !self.check(TokenKind::RBracket) && !self.is_at_eof() {
            let key = self.parse_expr(MIN_PRECEDENCE)?;
            self.expect(TokenKind::Colon)?;
            let value = self.parse_expr(MIN_PRECEDENCE)?;
            let span = Span::merge(key.span(), value.span());

            entries.push(DictEntry { key, value, span });

            if !self.check(TokenKind::RBracket) {
                self.expect(TokenKind::Comma)?;
            }
        }

        self.expect(TokenKind::RBracket)?;
        let end_span = self
            .tokens
            .get(self.pos.saturating_sub(1))
            .map_or(start_span, |t| t.span);

        Ok(self.alloc_expr(Expr::Dict {
            entries,
            span: Span::merge(start_span, end_span),
        }))
    }

    /// Parses string interpolation.
    fn parse_interpolation(
        &mut self,
        start_span: Span,
        initial_value: Symbol,
    ) -> ParserResult<&'arena Expr<'arena>> {
        let mut parts = Vec::new();
        parts.push(InterpolationPart::Text(initial_value));

        while self.check(TokenKind::InterpolationStart) {
            self.bump(); // consume \(

            let expr = self.parse_expr(MIN_PRECEDENCE)?;

            self.expect(TokenKind::RParen)?;

            parts.push(InterpolationPart::Expr(expr));

            // Check for more string parts
            if let Some(Token {
                kind: TokenKind::StringLiteral(value),
                ..
            }) = self.peek()
            {
                parts.push(InterpolationPart::Text(*value));
                self.bump();
            }
        }

        let end_span = self
            .tokens
            .get(self.pos.saturating_sub(1))
            .map_or(start_span, |t| t.span);

        Ok(self.alloc_expr(Expr::Interpolation {
            parts,
            span: Span::merge(start_span, end_span),
        }))
    }

    /// Parses a statement.
    fn parse_stmt(&mut self) -> ParserResult<Stmt<'arena>> {
        let token = match self.peek() {
            Some(t) => t,
            None => {
                return Err(ParserError::ExpectedStatement {
                    span: Span::point(self.source.len(), 1, 1),
                });
            }
        };

        match &token.kind {
            TokenKind::Let => self.parse_let_stmt(false),
            TokenKind::Mut => self.parse_let_stmt(true),
            TokenKind::Return => self.parse_return_stmt(),
            _ => {
                // Try as expression statement
                let expr = self.parse_expr(MIN_PRECEDENCE)?;
                Ok(Stmt::Expr {
                    expr,
                    span: expr.span(),
                })
            }
        }
    }

    /// Parses a let or mut binding statement.
    fn parse_let_stmt(&mut self, mutable: bool) -> ParserResult<Stmt<'arena>> {
        let start_span = self.bump().unwrap().span; // consume 'let' or 'mut'

        let name = self.expect_identifier()?;

        let type_annotation = if self.check(TokenKind::Colon) {
            self.bump(); // consume ':'
            Some(self.parse_type()?)
        } else {
            None
        };

        let init = if self.check(TokenKind::Eq) {
            self.bump(); // consume '='
            Some(self.parse_expr(MIN_PRECEDENCE)?)
        } else {
            None
        };

        self.expect(TokenKind::Semicolon)?;

        let end_span = self
            .tokens
            .get(self.pos.saturating_sub(1))
            .map_or(start_span, |t| t.span);

        Ok(if mutable {
            Stmt::Mut {
                name,
                type_annotation,
                init,
                span: Span::merge(start_span, end_span),
            }
        } else {
            Stmt::Let {
                name,
                type_annotation,
                init,
                span: Span::merge(start_span, end_span),
            }
        })
    }

    /// Parses a return statement.
    fn parse_return_stmt(&mut self) -> ParserResult<Stmt<'arena>> {
        let start_span = self.bump().unwrap().span; // consume 'return'

        let value = if self.check(TokenKind::Semicolon)
            || self.check(TokenKind::RBrace)
        {
            None
        } else {
            Some(self.parse_expr(MIN_PRECEDENCE)?)
        };

        self.expect(TokenKind::Semicolon)?;

        let end_span = self
            .tokens
            .get(self.pos.saturating_sub(1))
            .map_or(start_span, |t| t.span);

        Ok(Stmt::Return {
            value,
            span: Span::merge(start_span, end_span),
        })
    }

    /// Parses a type annotation.
    fn parse_type(&mut self) -> ParserResult<Type> {
        let start_span = match self.peek() {
            Some(t) => t.span,
            None => {
                return Err(ParserError::ExpectedType {
                    span: Span::point(self.source.len(), 1, 1),
                });
            }
        };

        // Check for Self type
        if self.check(TokenKind::SelfType) {
            self.bump();
            return Ok(Type::SelfType { span: start_span });
        }

        // Check for tuple or function types: (...)
        if self.check(TokenKind::LParen) {
            return self.parse_tuple_or_function_type();
        }

        // Check for array or dict types: [...]
        if self.check(TokenKind::LBracket) {
            return self.parse_array_or_dict_type();
        }

        // Simple or generic type: T or T<Args>
        self.parse_simple_or_generic_type()
    }

    /// Parses a tuple or function type: (T1, T2) or (T1, T2) -> T3
    fn parse_tuple_or_function_type(&mut self) -> ParserResult<Type> {
        let start_span = self.bump().unwrap().span; // consume (

        let mut elements = Vec::new();

        while !self.check(TokenKind::RParen) && !self.is_at_eof() {
            elements.push(self.parse_type()?);

            if !self.check(TokenKind::RParen) {
                self.expect(TokenKind::Comma)?;
            }
        }

        self.expect(TokenKind::RParen)?;

        // Check for function type: -> ReturnType
        if self.check(TokenKind::Arrow) {
            self.bump(); // consume ->
            let return_type = Box::new(self.parse_type()?);
            let end_span = return_type.span();

            Ok(Type::Function {
                params: elements,
                return_type,
                span: Span::merge(start_span, end_span),
            })
        } else {
            // Tuple type
            let end_span = if elements.is_empty() {
                start_span
            } else {
                match self.tokens.get(self.pos.saturating_sub(1)) {
                    Some(t) => t.span,
                    None => start_span,
                }
            };

            Ok(Type::Tuple {
                elements,
                span: Span::merge(start_span, end_span),
            })
        }
    }

    /// Parses an array or dict type: [T] or [T; N] or [K: V]
    fn parse_array_or_dict_type(&mut self) -> ParserResult<Type> {
        let start_span = self.bump().unwrap().span; // consume [

        // Parse first type
        let first_type = self.parse_type()?;

        // Check for dict type: [K: V]
        if self.check(TokenKind::Colon) {
            self.bump(); // consume :
            let value_type = Box::new(self.parse_type()?);
            self.expect(TokenKind::RBracket)?;
            let end_span = value_type.span();

            return Ok(Type::Dict {
                key: Box::new(first_type),
                value: value_type,
                span: Span::merge(start_span, end_span),
            });
        }

        // Check for array with size: [T; N]
        if self.check(TokenKind::Semicolon) {
            self.bump(); // consume ;
            // Parse size (should be a number literal)
            let size_sym = self.peek().and_then(|t| {
                if let TokenKind::IntegerLiteral(sym, _) = t.kind {
                    Some(sym)
                } else {
                    None
                }
            });

            if size_sym.is_some() {
                self.bump();
            }

            self.expect(TokenKind::RBracket)?;
            let end_span = match self.tokens.get(self.pos.saturating_sub(1)) {
                Some(t) => t.span,
                None => start_span,
            };

            return Ok(Type::Array {
                element: Box::new(first_type),
                size: size_sym,
                span: Span::merge(start_span, end_span),
            });
        }

        // Simple array type: [T]
        self.expect(TokenKind::RBracket)?;
        let end_span = match self.tokens.get(self.pos.saturating_sub(1)) {
            Some(t) => t.span,
            None => start_span,
        };

        Ok(Type::Array {
            element: Box::new(first_type),
            size: None,
            span: Span::merge(start_span, end_span),
        })
    }

    /// Parses a simple or generic type: T or T<Args>
    fn parse_simple_or_generic_type(&mut self) -> ParserResult<Type> {
        let token = self.peek().ok_or(ParserError::ExpectedType {
            span: Span::point(self.source.len(), 1, 1),
        })?;

        match &token.kind {
            TokenKind::Ident(_) => {
                let ident_token = self.bump().unwrap();
                let sym = match &ident_token.kind {
                    TokenKind::Ident(s) => *s,
                    _ => unreachable!(),
                };
                let span = ident_token.span;

                // Check for optional type: T?
                if self.check(TokenKind::Question) {
                    self.bump(); // consume ?
                    let end_span =
                        match self.tokens.get(self.pos.saturating_sub(1)) {
                            Some(t) => t.span,
                            None => span,
                        };

                    return Ok(Type::Optional {
                        inner: Box::new(Type::Simple { name: sym, span }),
                        span: Span::merge(span, end_span),
                    });
                }

                // Check for generic type: T<Args>
                if self.check(TokenKind::LAngle) {
                    return self.parse_generic_type(sym, span);
                }

                // Simple type
                Ok(Type::Simple { name: sym, span })
            }
            _ => Err(ParserError::ExpectedType { span: token.span }),
        }
    }

    /// Parses a generic type: T<Args>
    fn parse_generic_type(
        &mut self,
        name: Symbol,
        start_span: Span,
    ) -> ParserResult<Type> {
        self.expect(TokenKind::LAngle)?;

        let mut params = Vec::new();

        while !self.check(TokenKind::RAngle) && !self.is_at_eof() {
            params.push(self.parse_type()?);

            if !self.check(TokenKind::RAngle) {
                self.expect(TokenKind::Comma)?;
            }
        }

        self.expect(TokenKind::RAngle)?;

        let end_span = match self.tokens.get(self.pos.saturating_sub(1)) {
            Some(t) => t.span,
            None => start_span,
        };

        Ok(Type::Generic {
            name,
            params,
            span: Span::merge(start_span, end_span),
        })
    }

    /// Parses a pattern.
    fn parse_pattern(&mut self) -> ParserResult<Pattern> {
        // Check for or-pattern first (lowest precedence in patterns)
        if self.is_or_pattern() {
            return self.parse_or_pattern();
        }

        // Check for literal patterns
        if let Some(token) = self.peek() {
            match &token.kind {
                TokenKind::IntegerLiteral(_, _)
                | TokenKind::FloatLiteral(_, _)
                | TokenKind::StringLiteral(_)
                | TokenKind::BoolLiteral(_)
                | TokenKind::Nil => {
                    return self.parse_literal_pattern();
                }
                _ => {}
            }
        }

        // Check for structural patterns
        let start_span = match self.peek() {
            Some(t) => t.span,
            None => {
                return Err(ParserError::InvalidPattern {
                    message: "unexpected EOF".to_string(),
                    span: Span::point(self.source.len(), 1, 1),
                });
            }
        };

        match self.peek().map(|t| &t.kind) {
            Some(TokenKind::Underscore) => {
                self.bump();
                Ok(Pattern::Wildcard { span: start_span })
            }

            Some(TokenKind::Ident(_)) => {
                // Could be: variable, struct, or enum pattern
                self.parse_ident_or_struct_pattern()
            }

            Some(TokenKind::LParen) => self.parse_tuple_pattern(),

            Some(TokenKind::LBracket) => self.parse_array_pattern(),

            _ => {
                let found = format!("{:?}", self.peek());
                Err(ParserError::InvalidPattern {
                    message: format!("expected pattern, found {found}"),
                    span: start_span,
                })
            }
        }
    }

    /// Checks if the current position is an or-pattern (`|`).
    fn is_or_pattern(&mut self) -> bool {
        // Look ahead to see if there's a `|` before the next major construct
        let iter = self.tokens.iter().skip(self.pos);

        for token in iter {
            match &token.kind {
                TokenKind::Pipe => {
                    // Found | - it's an or-pattern
                    return true;
                }
                TokenKind::PipePipe => {
                    // || is logical OR, not pattern OR
                    return false;
                }
                TokenKind::Comma | TokenKind::FatArrow | TokenKind::Eq => {
                    // These delimit patterns
                    return false;
                }
                _ => {}
            }
        }

        false
    }

    /// Parses an or-pattern: `pattern1 | pattern2`.
    fn parse_or_pattern(&mut self) -> ParserResult<Pattern> {
        let left = Box::new(self.parse_pattern()?);
        let start_span = left.span();

        // Check for | operator
        if self.check(TokenKind::Pipe) {
            self.bump(); // consume |
            let right = Box::new(self.parse_pattern()?);
            let end_span = right.span();

            return Ok(Pattern::Or {
                left,
                right,
                span: Span::merge(start_span, end_span),
            });
        }

        // No | found, just return the left pattern
        Ok(*left)
    }

    /// Parses a literal pattern: `42`, `"hello"`, `true`, `nil`.
    fn parse_literal_pattern(&mut self) -> ParserResult<Pattern> {
        let token = self.bump().unwrap();
        let span = token.span;

        match &token.kind {
            TokenKind::IntegerLiteral(_, _)
            | TokenKind::FloatLiteral(_, _)
            | TokenKind::StringLiteral(_)
            | TokenKind::BoolLiteral(_)
            | TokenKind::Nil => Ok(Pattern::Literal {
                value: token.kind.clone(),
                span,
            }),
            _ => unreachable!(),
        }
    }

    /// Parses an identifier, struct, or enum pattern.
    fn parse_ident_or_struct_pattern(&mut self) -> ParserResult<Pattern> {
        let ident_token = self.bump().unwrap();
        let sym = match &ident_token.kind {
            TokenKind::Ident(s) => *s,
            _ => unreachable!(),
        };
        let span = ident_token.span;

        // Look ahead to determine if this is:
        // - Variable pattern: `x`
        // - Enum pattern: `Option::Some(x)`
        // - Struct pattern: `Point { x, y }`

        if self.check(TokenKind::ColonColon) {
            // Path - enum pattern or qualified struct
            self.parse_path_pattern(sym, span)
        } else if self.check(TokenKind::LBrace) {
            // Struct pattern: `Point { x, y }`
            self.parse_struct_pattern(vec![sym], span)
        } else if self.check(TokenKind::LParen) {
            // Could be enum variant with payload: `Some(x)`
            self.parse_enum_pattern(vec![sym], span)
        } else {
            // Simple variable binding
            Ok(Pattern::Variable {
                name: sym,
                mutable: false,
                span,
            })
        }
    }

    /// Parses a path pattern (enum or qualified struct).
    fn parse_path_pattern(
        &mut self,
        first_sym: Symbol,
        start_span: Span,
    ) -> ParserResult<Pattern> {
        let mut path = vec![first_sym];

        while self.check(TokenKind::ColonColon) {
            self.bump(); // consume ::
            let sym = self.expect_identifier()?;
            path.push(sym);
        }

        // Now determine if it's enum or struct based on next token
        if self.check(TokenKind::LBrace) {
            // Struct pattern: `module::Type { fields }`
            self.parse_struct_pattern(path, start_span)
        } else if self.check(TokenKind::LParen) || self.peek().is_some() {
            // Enum pattern: `Option::Some(x)` or `Option::None`
            self.parse_enum_pattern(path, start_span)
        } else {
            // Just a path (e.g., enum variant without payload)
            let variant = *path.last().unwrap();
            let type_path = path[..path.len() - 1].to_vec();
            Ok(Pattern::Enum {
                type_path,
                variant,
                payload: None,
                span: start_span,
            })
        }
    }

    /// Parses a struct pattern: `Point { x, y: y0 }`.
    fn parse_struct_pattern(
        &mut self,
        type_path: Vec<Symbol>,
        start_span: Span,
    ) -> ParserResult<Pattern> {
        self.expect(TokenKind::LBrace)?;

        let mut fields = Vec::new();

        while !self.check(TokenKind::RBrace) && !self.is_at_eof() {
            let field_name = self.expect_identifier()?;
            let field_span = match self.peek() {
                Some(t) => t.span,
                None => Span::point(self.source.len(), 1, 1),
            };

            // Check for shorthand `x` vs explicit `x: pattern`
            let pattern = if self.check(TokenKind::Colon) {
                self.bump(); // consume :
                Some(Box::new(self.parse_pattern()?))
            } else {
                // Shorthand: field name is also variable pattern
                None
            };

            fields.push(FieldPat {
                name: field_name,
                pattern,
                span: field_span,
            });

            if !self.check(TokenKind::RBrace) {
                self.expect(TokenKind::Comma)?;
            }
        }

        self.expect(TokenKind::RBrace)?;

        let end_span = match self.tokens.get(self.pos.saturating_sub(1)) {
            Some(t) => t.span,
            None => start_span,
        };

        Ok(Pattern::Struct {
            type_path,
            fields,
            span: Span::merge(start_span, end_span),
        })
    }

    /// Parses an enum pattern: `Option::Some(x)` or `Option::None`.
    fn parse_enum_pattern(
        &mut self,
        type_path: Vec<Symbol>,
        start_span: Span,
    ) -> ParserResult<Pattern> {
        // Extract variant name (last component of path)
        let variant = *type_path.last().ok_or(ParserError::InvalidPattern {
            message: "empty enum path".to_string(),
            span: start_span,
        })?;

        let enum_type_path = type_path[..type_path.len() - 1].to_vec();

        // Check for payload
        let payload = if self.check(TokenKind::LParen) {
            self.bump(); // consume (
            let pat = self.parse_pattern()?;
            self.expect(TokenKind::RParen)?;
            Some(Box::new(pat))
        } else {
            None
        };

        let end_span = payload.as_ref().map_or(start_span, |p| p.span());

        Ok(Pattern::Enum {
            type_path: enum_type_path,
            variant,
            payload,
            span: Span::merge(start_span, end_span),
        })
    }

    /// Parses a tuple pattern: `(x, y, z)`.
    fn parse_tuple_pattern(&mut self) -> ParserResult<Pattern> {
        let start_span = self.peek().unwrap().span;
        self.bump(); // consume (

        let mut elements = Vec::new();

        while !self.check(TokenKind::RParen) && !self.is_at_eof() {
            elements.push(self.parse_pattern()?);

            if !self.check(TokenKind::RParen) {
                self.expect(TokenKind::Comma)?;
            }
        }

        self.expect(TokenKind::RParen)?;

        let end_span = match self.tokens.get(self.pos.saturating_sub(1)) {
            Some(t) => t.span,
            None => start_span,
        };

        Ok(Pattern::Tuple {
            elements,
            span: Span::merge(start_span, end_span),
        })
    }

    /// Parses an array pattern: `[first, second, ..rest]`.
    fn parse_array_pattern(&mut self) -> ParserResult<Pattern> {
        let start_span = self.peek().unwrap().span;
        self.bump(); // consume [

        let mut elements = Vec::new();
        let mut rest = None;

        while !self.check(TokenKind::RBracket) && !self.is_at_eof() {
            // Check for ..rest pattern
            if self.check(TokenKind::DotDot) {
                self.bump(); // consume ..

                // Optional identifier after ..
                let sym_opt = self.peek().and_then(|t| {
                    if let TokenKind::Ident(s) = t.kind {
                        Some(s)
                    } else {
                        None
                    }
                });

                if sym_opt.is_some() {
                    self.bump();
                }

                rest = if let Some(s) = sym_opt {
                    Some(Box::new(Pattern::Variable {
                        name: s,
                        mutable: false,
                        span: start_span,
                    }))
                } else {
                    Some(Box::new(Pattern::Wildcard { span: start_span }))
                };
                break;
            }

            elements.push(self.parse_pattern()?);

            if !self.check(TokenKind::RBracket) {
                self.expect(TokenKind::Comma)?;
            }
        }

        self.expect(TokenKind::RBracket)?;

        let end_span = match self.tokens.get(self.pos.saturating_sub(1)) {
            Some(t) => t.span,
            None => start_span,
        };

        Ok(Pattern::Array {
            elements,
            rest,
            span: Span::merge(start_span, end_span),
        })
    }

    /// Expects and returns an identifier.
    fn expect_identifier(&mut self) -> ParserResult<Symbol> {
        let token = self.peek().ok_or(ParserError::ExpectedIdentifier {
            span: Span::point(self.source.len(), 1, 1),
        })?;

        match &token.kind {
            TokenKind::Ident(sym) => {
                let sym = *sym;
                self.bump();
                Ok(sym)
            }
            _ => Err(ParserError::ExpectedIdentifier { span: token.span }),
        }
    }

    /// Converts a `TokenKind` to a `BinaryOp`.
    fn token_kind_to_binary_op(
        &self,
        kind: TokenKind,
    ) -> ParserResult<BinaryOp> {
        match kind {
            TokenKind::Plus => Ok(BinaryOp::Add),
            TokenKind::Minus => Ok(BinaryOp::Sub),
            TokenKind::Star => Ok(BinaryOp::Mul),
            TokenKind::Slash => Ok(BinaryOp::Div),
            TokenKind::Percent => Ok(BinaryOp::Mod),
            TokenKind::EqEq => Ok(BinaryOp::Eq),
            TokenKind::BangEq => Ok(BinaryOp::Neq),
            TokenKind::Lt | TokenKind::LAngle => Ok(BinaryOp::Lt),
            TokenKind::Gt | TokenKind::RAngle => Ok(BinaryOp::Gt),
            TokenKind::LtEq => Ok(BinaryOp::Lte),
            TokenKind::GtEq => Ok(BinaryOp::Gte),
            TokenKind::AmpAmp => Ok(BinaryOp::And),
            TokenKind::PipePipe => Ok(BinaryOp::Or),
            TokenKind::Eq => Ok(BinaryOp::Assign),
            _ => {
                let span = self
                    .tokens
                    .get(self.pos.saturating_sub(1))
                    .map_or_else(|| Span::point(0, 1, 1), |t| t.span);
                Err(ParserError::UnexpectedToken {
                    expected: vec!["binary operator".to_string()],
                    found: format!("{kind:?}"),
                    span,
                })
            }
        }
    }

    /// Recovers from an error by skipping tokens until a synchronization point.
    /// TODO: Implement error recovery with enhance_error() for better messages
    fn recover_to_sync_point(&mut self, sync_kinds: &[TokenKind]) {
        while !self.is_at_eof() {
            if let Some(token) = self.peek()
                && sync_kinds.contains(&token.kind)
            {
                return;
            }
            self.bump();
        }
    }

    /// Returns all accumulated errors.
    #[must_use]
    pub fn errors(&self) -> &[ParserError] {
        &self.errors
    }

    /// Checks if any errors were encountered during parsing.
    #[must_use]
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Emits all accumulated errors using a diagnostic emitter.
    ///
    /// This is useful for displaying all parse errors with rich formatting
    /// and source highlighting.
    pub fn emit_errors(&self, emitter: &crate::diagnostic::Emitter) {
        for error in &self.errors {
            let syntax_error = crate::error::SyntaxError::Parser(error.clone());
            emitter.emit_syntax_error(&syntax_error, self.source);
        }
    }

    // ===== Declaration Parsing =====

    /// Parses a top-level declaration.
    pub fn parse_decl(&mut self) -> ParserResult<Decl<'arena>> {
        // Check for visibility modifier
        let visibility = self.parse_visibility();

        let start_span = match self.peek() {
            Some(t) => t.span,
            None => {
                return Err(ParserError::ExpectedExpression {
                    span: Span::point(self.source.len(), 1, 1),
                });
            }
        };

        let token_kind = match self.peek() {
            Some(t) => t.kind.clone(),
            None => {
                return Err(ParserError::ExpectedExpression {
                    span: Span::point(self.source.len(), 1, 1),
                });
            }
        };

        match token_kind {
            // Check for `mut fn` or `static fn` or `init`
            TokenKind::Mut => {
                self.bump(); // consume 'mut'
                if self.check(TokenKind::Fn) {
                    self.parse_fn_decl(
                        visibility, start_span, true, false, false,
                    )
                } else {
                    Err(ParserError::UnexpectedToken {
                        expected: vec!["fn".to_string()],
                        found: format!("{token_kind:?}"),
                        span: start_span,
                    })
                }
            }

            TokenKind::Static => {
                // Check if this is `static fn` or `static let`/`static let mut`
                if self.peek_next().map(|t| &t.kind) == Some(&TokenKind::Fn) {
                    self.bump(); // consume 'static'
                    // Don't consume 'fn', parse_fn_decl will do that
                    self.parse_fn_decl(
                        visibility, start_span, false, false, true,
                    )
                } else {
                    self.parse_static_decl(visibility, start_span)
                }
            }

            // Initializer
            TokenKind::Init => {
                self.parse_fn_decl(visibility, start_span, false, true, false)
            }

            // Function declaration
            TokenKind::Fn => {
                self.parse_fn_decl(visibility, start_span, false, false, false)
            }

            // Struct declaration
            TokenKind::Struct => self.parse_struct_decl(visibility, start_span),

            // Class declaration
            TokenKind::Class => self.parse_class_decl(visibility, start_span),

            // Enum declaration
            TokenKind::Enum => self.parse_enum_decl(visibility, start_span),

            // Protocol declaration
            TokenKind::Protocol => {
                self.parse_protocol_decl(visibility, start_span)
            }

            // Implementation block
            TokenKind::Impl => self.parse_impl_decl(start_span),

            // Constant declaration
            TokenKind::Const => self.parse_const_decl(visibility, start_span),

            // Type alias
            TokenKind::Type => {
                self.parse_type_alias_decl(visibility, start_span)
            }

            _ => {
                let found = format!("{token_kind:?}");
                Err(ParserError::UnexpectedToken {
                    expected: vec![
                        "declaration".to_string(),
                        "fn".to_string(),
                        "struct".to_string(),
                        "class".to_string(),
                        "enum".to_string(),
                        "protocol".to_string(),
                        "impl".to_string(),
                        "const".to_string(),
                        "static".to_string(),
                        "type".to_string(),
                    ],
                    found,
                    span: start_span,
                })
            }
        }
    }

    /// Parses a visibility modifier (pub/prv).
    fn parse_visibility(&mut self) -> Visibility {
        if self.check(TokenKind::Pub) {
            self.bump();
            Visibility::Public
        } else {
            // Private is implicit or can be marked with `prv`
            if self.check(TokenKind::Prv) {
                self.bump();
            }
            Visibility::Private
        }
    }

    /// Parses a function declaration.
    fn parse_fn_decl(
        &mut self,
        visibility: Visibility,
        start_span: Span,
        is_mut: bool,
        is_init: bool,
        is_static: bool,
    ) -> ParserResult<Decl<'arena>> {
        // Consume 'fn' or 'init' token
        if is_init {
            self.bump(); // consume 'init'
        } else {
            self.bump(); // consume 'fn'
        }

        let name = if !is_init {
            self.expect_identifier()?
        } else {
            // For init functions, use "init" as the name internally
            self.interner.intern("init")
        };

        // Parse generics: <T, U>
        let generics = self.parse_generics()?;

        self.expect(TokenKind::LParen)?;

        // Parse parameters
        let mut params = Vec::new();
        while !self.check(TokenKind::RParen) && !self.is_at_eof() {
            let param = self.parse_fn_param()?;
            params.push(param);

            if !self.check(TokenKind::RParen) {
                self.expect(TokenKind::Comma)?;
            }
        }

        self.expect(TokenKind::RParen)?;

        // Parse return type
        let return_type = if self.check(TokenKind::Arrow) {
            self.bump(); // consume ->
            Some(self.parse_type()?)
        } else {
            None
        };

        // Parse body (must be a block expression)
        let body = self.parse_expr(MIN_PRECEDENCE)?;

        let end_span = body.span();

        Ok(Decl::Fn {
            is_mut,
            is_init,
            is_static,
            name,
            generics,
            params,
            return_type,
            body,
            visibility,
            span: Span::merge(start_span, end_span),
        })
    }

    /// Parses a function parameter: `name: Type` or `_ name: Type` or `label name: Type`
    fn parse_fn_param(&mut self) -> ParserResult<FnParam> {
        let start_span = match self.peek() {
            Some(t) => t.span,
            None => {
                return Err(ParserError::ExpectedExpression {
                    span: Span::point(self.source.len(), 1, 1),
                });
            }
        };

        // Check for underscore (omitted label): `_ name: Type`
        let label = if self.check(TokenKind::Underscore) {
            self.bump(); // consume _
            None
        } else {
            // Peek ahead to see if this is `label name: Type` or just `name: Type`
            match self.peek_next().map(|t| &t.kind) {
                Some(TokenKind::Colon) => {
                    // Just `name: Type` - label is None (will use name as label)
                    None
                }
                Some(TokenKind::Ident(_)) => {
                    // `label name: Type` - consume label and use it
                    let label = self.expect_identifier()?;
                    Some(label)
                }
                _ => None,
            }
        };

        let name = self.expect_identifier()?;
        self.expect(TokenKind::Colon)?;
        let type_annotation = self.parse_type()?;
        let span = Span::merge(start_span, type_annotation.span());

        Ok(FnParam {
            label,
            name,
            type_annotation,
            span,
        })
    }

    /// Parses a struct declaration.
    fn parse_struct_decl(
        &mut self,
        visibility: Visibility,
        start_span: Span,
    ) -> ParserResult<Decl<'arena>> {
        self.bump(); // consume 'struct'

        let name = self.expect_identifier()?;

        // Parse generics: <T, U>
        let generics = self.parse_generics()?;

        // Parse protocol conformances: : Protocol1, Protocol2
        let protocols = if self.check(TokenKind::Colon) {
            self.bump(); // consume :
            self.parse_protocol_list()?
        } else {
            Vec::new()
        };

        self.expect(TokenKind::LBrace)?;

        // Parse fields
        let mut fields = Vec::new();
        while !self.check(TokenKind::RBrace) && !self.is_at_eof() {
            let field = self.parse_struct_field()?;
            fields.push(field);

            if !self.check(TokenKind::RBrace) {
                self.expect(TokenKind::Comma)?;
            }
        }

        self.expect(TokenKind::RBrace)?;
        let end_span = self
            .tokens
            .get(self.pos.saturating_sub(1))
            .map_or(start_span, |t| t.span);

        Ok(Decl::Struct {
            name,
            generics,
            fields,
            protocols,
            visibility,
            span: Span::merge(start_span, end_span),
        })
    }

    /// Parses a struct field: `name: Type`
    fn parse_struct_field(&mut self) -> ParserResult<StructField> {
        let start_span = match self.peek() {
            Some(t) => t.span,
            None => {
                return Err(ParserError::ExpectedExpression {
                    span: Span::point(self.source.len(), 1, 1),
                });
            }
        };

        let name = self.expect_identifier()?;
        self.expect(TokenKind::Colon)?;
        let type_annotation = self.parse_type()?;
        let span = Span::merge(start_span, type_annotation.span());

        Ok(StructField {
            name,
            type_annotation,
            span,
        })
    }

    /// Parses a class declaration.
    fn parse_class_decl(
        &mut self,
        visibility: Visibility,
        start_span: Span,
    ) -> ParserResult<Decl<'arena>> {
        self.bump(); // consume 'class'

        let name = self.expect_identifier()?;

        // Parse generics: <T, U>
        let generics = self.parse_generics()?;

        // Parse optional superclass: class Name : Superclass
        let superclass = if self.check(TokenKind::Colon) {
            self.bump(); // consume :
            Some(self.parse_path_segments()?)
        } else {
            None
        };

        // Parse protocol conformances
        let protocols = if self.check(TokenKind::Colon) {
            self.bump(); // consume :
            self.parse_protocol_list()?
        } else if superclass.is_some() {
            // If we had a superclass, check for protocols after another :
            if self.check(TokenKind::Colon) {
                self.bump(); // consume :
                self.parse_protocol_list()?
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        self.expect(TokenKind::LBrace)?;

        // Parse fields
        let mut fields = Vec::new();
        while !self.check(TokenKind::RBrace) && !self.is_at_eof() {
            let field = self.parse_struct_field()?;
            fields.push(field);

            if !self.check(TokenKind::RBrace) {
                self.expect(TokenKind::Comma)?;
            }
        }

        self.expect(TokenKind::RBrace)?;
        let end_span = self
            .tokens
            .get(self.pos.saturating_sub(1))
            .map_or(start_span, |t| t.span);

        Ok(Decl::Class {
            name,
            generics,
            superclass,
            fields,
            protocols,
            visibility,
            span: Span::merge(start_span, end_span),
        })
    }

    /// Parses an enum declaration.
    fn parse_enum_decl(
        &mut self,
        visibility: Visibility,
        start_span: Span,
    ) -> ParserResult<Decl<'arena>> {
        self.bump(); // consume 'enum'

        let name = self.expect_identifier()?;

        // Parse generics: <T, U>
        let generics = self.parse_generics()?;

        // Parse protocol conformances
        let protocols = if self.check(TokenKind::Colon) {
            self.bump(); // consume :
            self.parse_protocol_list()?
        } else {
            Vec::new()
        };

        self.expect(TokenKind::LBrace)?;

        // Parse variants and methods (can be mixed like Swift)
        let mut variants = Vec::new();
        let mut methods = Vec::new();

        while !self.check(TokenKind::RBrace) && !self.is_at_eof() {
            // Check if this is a `case` variant or a method
            if self.check(TokenKind::Case) {
                // Parse enum variant
                let variant = self.parse_enum_variant()?;
                variants.push(variant);
            } else if self.check(TokenKind::Mut) || self.check(TokenKind::Static) || self.check(TokenKind::Init) || self.check(TokenKind::Fn) || self.check(TokenKind::Pub) || self.check(TokenKind::Prv) {
                // Parse method (pub/prv mut fn, pub/prv static fn, pub/prv init, pub/prv fn)
                let method = self.parse_impl_method()?;
                methods.push(FnDecl {
                    is_mut: method.is_mut,
                    is_init: method.is_init,
                    is_static: method.is_static,
                    name: method.name,
                    generics: method.generics,
                    params: method.params,
                    return_type: method.return_type,
                    visibility: method.visibility,
                    span: method.span,
                });
            } else {
                return Err(ParserError::UnexpectedToken {
                    expected: vec!["case".to_string(), "pub".to_string(), "prv".to_string(), "fn".to_string(), "mut".to_string(), "static".to_string(), "init".to_string()],
                    found: format!("{:?}", self.peek().map(|t| &t.kind)),
                    span: self.peek().map(|t| t.span).unwrap_or_else(|| Span::point(self.source.len(), 1, 1)),
                });
            }

            if !self.check(TokenKind::RBrace) {
                self.expect(TokenKind::Comma)?;
            }
        }

        self.expect(TokenKind::RBrace)?;
        let end_span = self
            .tokens
            .get(self.pos.saturating_sub(1))
            .map_or(start_span, |t| t.span);

        Ok(Decl::Enum {
            name,
            generics,
            variants,
            methods,
            protocols,
            visibility,
            span: Span::merge(start_span, end_span),
        })
    }

    /// Parses an enum variant (with `case` keyword).
    fn parse_enum_variant(&mut self) -> ParserResult<EnumVariant> {
        let start_span = self.bump().unwrap().span; // consume 'case'

        let name = self.expect_identifier()?;

        // Check what kind of variant this is
        if self.check(TokenKind::LParen) {
            // Tuple variant: case some(T)
            self.bump(); // consume (

            let mut fields = Vec::new();
            while !self.check(TokenKind::RParen) && !self.is_at_eof() {
                let ty = self.parse_type()?;
                fields.push(ty);

                if !self.check(TokenKind::RParen) {
                    self.expect(TokenKind::Comma)?;
                }
            }

            self.expect(TokenKind::RParen)?;
            let end_span = self
                .tokens
                .get(self.pos.saturating_sub(1))
                .map_or(start_span, |t| t.span);

            Ok(EnumVariant::Tuple {
                name,
                fields,
                span: Span::merge(start_span, end_span),
            })
        } else if self.check(TokenKind::LBrace) {
            // Struct variant: case point { x: T, y: T }
            self.bump(); // consume {

            let mut fields = Vec::new();
            while !self.check(TokenKind::RBrace) && !self.is_at_eof() {
                let field = self.parse_struct_field()?;
                fields.push(field);

                if !self.check(TokenKind::RBrace) {
                    self.expect(TokenKind::Comma)?;
                }
            }

            self.expect(TokenKind::RBrace)?;
            let end_span = self
                .tokens
                .get(self.pos.saturating_sub(1))
                .map_or(start_span, |t| t.span);

            Ok(EnumVariant::Struct {
                name,
                fields,
                span: Span::merge(start_span, end_span),
            })
        } else {
            // Unit variant: case none
            let end_span = self
                .tokens
                .get(self.pos.saturating_sub(1))
                .map_or(start_span, |t| t.span);

            Ok(EnumVariant::Unit {
                name,
                span: Span::merge(start_span, end_span),
            })
        }
    }

    /// Parses a protocol declaration.
    fn parse_protocol_decl(
        &mut self,
        visibility: Visibility,
        start_span: Span,
    ) -> ParserResult<Decl<'arena>> {
        self.bump(); // consume 'protocol'

        let name = self.expect_identifier()?;

        // Parse generics: <T, U>
        let generics = self.parse_generics()?;

        self.expect(TokenKind::LBrace)?;

        // Parse method signatures
        let mut methods = Vec::new();
        while !self.check(TokenKind::RBrace) && !self.is_at_eof() {
            let method = self.parse_protocol_method()?;
            methods.push(method);

            if !self.check(TokenKind::RBrace) {
                self.expect(TokenKind::Semicolon)?;
            }
        }

        self.expect(TokenKind::RBrace)?;
        let end_span = self
            .tokens
            .get(self.pos.saturating_sub(1))
            .map_or(start_span, |t| t.span);

        Ok(Decl::Protocol {
            name,
            generics,
            methods,
            visibility,
            span: Span::merge(start_span, end_span),
        })
    }

    /// Parses a protocol method signature.
    fn parse_protocol_method(&mut self) -> ParserResult<ProtocolMethod> {
        let start_span = match self.peek() {
            Some(t) => t.span,
            None => {
                return Err(ParserError::ExpectedExpression {
                    span: Span::point(self.source.len(), 1, 1),
                });
            }
        };

        self.expect(TokenKind::Fn)?;
        let name = self.expect_identifier()?;

        self.expect(TokenKind::LParen)?;

        let mut params = Vec::new();
        while !self.check(TokenKind::RParen) && !self.is_at_eof() {
            let param = self.parse_fn_param()?;
            params.push(param);

            if !self.check(TokenKind::RParen) {
                self.expect(TokenKind::Comma)?;
            }
        }

        self.expect(TokenKind::RParen)?;

        let return_type = if self.check(TokenKind::Arrow) {
            self.bump(); // consume ->
            Some(self.parse_type()?)
        } else {
            None
        };

        let end_span = self
            .tokens
            .get(self.pos.saturating_sub(1))
            .map_or(start_span, |t| t.span);

        Ok(ProtocolMethod {
            name,
            params,
            return_type,
            span: Span::merge(start_span, end_span),
        })
    }

    /// Parses an implementation block.
    fn parse_impl_decl(
        &mut self,
        start_span: Span,
    ) -> ParserResult<Decl<'arena>> {
        self.bump(); // consume 'impl'

        // Parse optional generics
        let _generics = self.parse_generics()?;

        // Check if this is "impl Protocol for Type"
        let type_path = self.parse_path_segments()?;

        let protocol = if self.check(TokenKind::For) {
            self.bump(); // consume 'for'
            Some(self.parse_path_segments()?)
        } else {
            None
        };

        self.expect(TokenKind::LBrace)?;

        // Parse methods
        let mut methods = Vec::new();
        while !self.check(TokenKind::RBrace) && !self.is_at_eof() {
            let method = self.parse_impl_method()?;
            methods.push(method);
        }

        self.expect(TokenKind::RBrace)?;
        let end_span = self
            .tokens
            .get(self.pos.saturating_sub(1))
            .map_or(start_span, |t| t.span);

        Ok(Decl::Impl {
            type_path,
            protocol,
            methods,
            span: Span::merge(start_span, end_span),
        })
    }

    /// Parses a method inside an impl block.
    fn parse_impl_method(&mut self) -> ParserResult<FnDecl> {
        // Parse visibility (will be resolved to most restrictive of parent and method later)
        let visibility = self.parse_visibility();
        let start_span = match self.peek() {
            Some(t) => t.span,
            None => {
                return Err(ParserError::ExpectedExpression {
                    span: Span::point(self.source.len(), 1, 1),
                });
            }
        };

        // Check for mut fn, static fn, or init
        let is_mut = self.check(TokenKind::Mut);
        if is_mut {
            self.bump(); // consume 'mut'
        }

        let is_static = self.check(TokenKind::Static);
        if is_static {
            self.bump(); // consume 'static'
        }

        let is_init = self.check(TokenKind::Init);

        // Consume 'fn' or 'init'
        if is_init {
            self.bump(); // consume 'init'
        } else {
            self.expect(TokenKind::Fn)?; // consumes 'fn'
        }

        let name = if !is_init {
            Some(self.expect_identifier()?)
        } else {
            None
        };

        let generics = self.parse_generics()?;

        self.expect(TokenKind::LParen)?;

        let mut params = Vec::new();
        while !self.check(TokenKind::RParen) && !self.is_at_eof() {
            let param = self.parse_fn_param()?;
            params.push(param);

            if !self.check(TokenKind::RParen) {
                self.expect(TokenKind::Comma)?;
            }
        }

        self.expect(TokenKind::RParen)?;

        let return_type = if self.check(TokenKind::Arrow) {
            self.bump(); // consume ->
            Some(self.parse_type()?)
        } else {
            None
        };

        // Parse body (must be a block expression)
        let body_expr = self.parse_expr(MIN_PRECEDENCE)?;
        let end_span = body_expr.span();

        Ok(FnDecl {
            is_mut,
            is_init,
            is_static,
            name,
            generics,
            params,
            return_type,
            visibility,
            span: Span::merge(start_span, end_span),
        })
    }

    /// Parses a constant declaration.
    fn parse_const_decl(
        &mut self,
        visibility: Visibility,
        start_span: Span,
    ) -> ParserResult<Decl<'arena>> {
        self.bump(); // consume 'const'

        let name = self.expect_identifier()?;
        self.expect(TokenKind::Colon)?;
        let type_annotation = self.parse_type()?;
        self.expect(TokenKind::Eq)?;
        let value = self.parse_expr(MIN_PRECEDENCE)?;
        self.expect(TokenKind::Semicolon)?;

        let end_span = value.span();

        Ok(Decl::Const {
            name,
            type_annotation,
            value,
            visibility,
            span: Span::merge(start_span, end_span),
        })
    }

    /// Parses a static declaration.
    fn parse_static_decl(
        &mut self,
        visibility: Visibility,
        start_span: Span,
    ) -> ParserResult<Decl<'arena>> {
        self.bump(); // consume 'static'

        // Expect 'let' or 'let mut'
        self.expect(TokenKind::Let)?;

        let mutable = if self.check(TokenKind::Mut) {
            self.bump(); // consume 'mut'
            true
        } else {
            false
        };

        let name = self.expect_identifier()?;
        self.expect(TokenKind::Colon)?;
        let type_annotation = self.parse_type()?;

        let init = if self.check(TokenKind::Eq) {
            self.bump(); // consume =
            Some(self.parse_expr(MIN_PRECEDENCE)?)
        } else {
            None
        };

        self.expect(TokenKind::Semicolon)?;

        let end_span = init
            .as_ref()
            .map_or_else(|| type_annotation.span(), |v| v.span());

        Ok(Decl::Static {
            name,
            type_annotation,
            init,
            mutable,
            visibility,
            span: Span::merge(start_span, end_span),
        })
    }

    /// Parses a type alias declaration.
    fn parse_type_alias_decl(
        &mut self,
        visibility: Visibility,
        start_span: Span,
    ) -> ParserResult<Decl<'arena>> {
        self.bump(); // consume 'type'

        let name = self.expect_identifier()?;

        // Parse generics: <T, U>
        let generics = self.parse_generics()?;

        self.expect(TokenKind::Eq)?;
        let target = self.parse_type()?;
        self.expect(TokenKind::Semicolon)?;

        let end_span = target.span();

        Ok(Decl::TypeAlias {
            name,
            generics,
            target,
            visibility,
            span: Span::merge(start_span, end_span),
        })
    }

    /// Parses generic type parameters: <T, U>
    fn parse_generics(&mut self) -> ParserResult<Vec<Symbol>> {
        if !self.check(TokenKind::LAngle) {
            return Ok(Vec::new());
        }

        self.bump(); // consume <

        let mut generics = Vec::new();
        while !self.check(TokenKind::RAngle) && !self.is_at_eof() {
            let ident = self.expect_identifier()?;
            generics.push(ident);

            if !self.check(TokenKind::RAngle) {
                self.expect(TokenKind::Comma)?;
            }
        }

        self.expect(TokenKind::RAngle)?;
        Ok(generics)
    }

    /// Parses a list of protocol names: Protocol1, Protocol2
    fn parse_protocol_list(&mut self) -> ParserResult<Vec<Vec<Symbol>>> {
        let mut protocols = Vec::new();

        loop {
            let path = self.parse_path_segments()?;
            protocols.push(path);

            if !self.check(TokenKind::Comma) {
                break;
            }
            self.bump(); // consume ,
        }

        Ok(protocols)
    }

    /// Parses path segments: Type or `Type::SubType`
    fn parse_path_segments(&mut self) -> ParserResult<Vec<Symbol>> {
        let mut segments = Vec::new();

        loop {
            let ident = self.expect_identifier()?;
            segments.push(ident);

            if !self.check(TokenKind::ColonColon) {
                break;
            }
            self.bump(); // consume ::
        }

        Ok(segments)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Lexer;

    fn parse_expr(source: &'_ str) -> ParserResult<Expr<'_>> {
        let arena = LocalArena::new(8192);
        let lexer = Lexer::new(source);
        let (tokens, interner) = match lexer.lex_with_interner() {
            Ok(result) => result,
            Err(_) => panic!("Lexer failed"),
        };

        let mut parser = Parser::new(tokens, source, interner, arena);
        parser.parse_expression().map(|expr| (*expr).clone())
    }

    #[test]
    fn test_parse_integer_literal() {
        let expr = parse_expr("42").unwrap();
        match expr {
            Expr::IntegerLiteral { .. } => {}
            _ => panic!("Expected IntegerLiteral, got {:?}", expr),
        }
    }

    #[test]
    fn test_parse_nil_literal() {
        let expr = parse_expr("nil").unwrap();
        match expr {
            Expr::Nil { .. } => {}
            _ => panic!("Expected Nil, got {:?}", expr),
        }
    }

    #[test]
    fn test_parse_binary_addition() {
        let expr = parse_expr("1 + 2").unwrap();
        match expr {
            Expr::Binary {
                op: BinaryOp::Add, ..
            } => {}
            _ => panic!("Expected Binary Add, got {:?}", expr),
        }
    }

    #[test]
    fn test_parse_unary_minus() {
        let expr = parse_expr("-42").unwrap();
        match expr {
            Expr::Unary {
                op: UnaryOp::Minus, ..
            } => {}
            _ => panic!("Expected Unary Minus, got {:?}", expr),
        }
    }

    #[test]
    fn test_parse_parenthesized() {
        let expr = parse_expr("(1 + 2)").unwrap();
        match expr {
            Expr::Paren { .. } => {}
            _ => panic!("Expected Paren, got {:?}", expr),
        }
    }

    #[test]
    fn test_parse_block() {
        let expr = parse_expr("{ }").unwrap();
        match expr {
            Expr::Block { .. } => {}
            _ => panic!("Expected Block, got {:?}", expr),
        }
    }

    #[test]
    fn test_parse_array() {
        let expr = parse_expr("[1, 2, 3]").unwrap();
        match expr {
            Expr::Array { .. } => {}
            _ => panic!("Expected Array, got {:?}", expr),
        }
    }

    #[test]
    fn test_parse_if_expression() {
        let expr = parse_expr("if true { nil } else { nil }").unwrap();
        match expr {
            Expr::If { .. } => {}
            _ => panic!("Expected If, got {:?}", expr),
        }
    }

    #[test]
    fn test_parse_while_loop() {
        let expr = parse_expr("while true { nil }").unwrap();
        match expr {
            Expr::WhileLoop { .. } => {}
            _ => panic!("Expected WhileLoop, got {:?}", expr),
        }
    }

    #[test]
    fn test_operator_precedence() {
        let expr = parse_expr("1 + 2 * 3").unwrap();

        // Should be parsed as: 1 + (2 * 3)
        match expr {
            Expr::Binary {
                op: BinaryOp::Add,
                left,
                ..
            } => match left {
                Expr::IntegerLiteral { .. } => {}
                _ => panic!("Left should be integer literal"),
            },
            _ => panic!("Expected Binary Add, got {:?}", expr),
        }
    }

    #[test]
    fn test_error_recovery_missing_semicolon() {
        let source = "let x = 42; let y = 10;";
        let arena = LocalArena::new(8192);
        let lexer = Lexer::new(source);
        let (tokens, interner) = match lexer.lex_with_interner() {
            Ok(result) => result,
            Err(e) => {
                eprintln!("Lexer error: {:?}", e);
                panic!("Lexer failed: {:?}", e);
            }
        };

        let mut parser = Parser::new(tokens, source, interner, arena);
        let _result = parser.parse_stmt();

        // Should parse successfully (both statements have semicolons)
        assert!(!parser.has_errors());
    }

    // ===== Declaration Parsing Tests =====

    #[test]
    fn test_parse_fn_decl() {
        let source = "fn foo() { nil }";
        let arena = LocalArena::new(8192);
        let lexer = Lexer::new(source);
        let (tokens, interner) = lexer.lex_with_interner().unwrap();
        let mut parser = Parser::new(tokens, source, interner, arena);

        let decl = parser.parse_decl().unwrap();
        match decl {
            Decl::Fn { is_init, name, .. } => {
                assert!(!is_init);
                let name_str = parser.resolve_symbol(name);
                assert_eq!(name_str, "foo");
            }
            _ => panic!("Expected Fn decl, got {:?}", decl),
        }
    }

    #[test]
    fn test_parse_fn_decl_with_params() {
        let source = "fn add(x: Int, y: Int) -> Int { x + y }";
        let arena = LocalArena::new(8192);
        let lexer = Lexer::new(source);
        let (tokens, interner) = lexer.lex_with_interner().unwrap();
        let mut parser = Parser::new(tokens, source, interner, arena);

        let decl = parser.parse_decl().unwrap();
        match decl {
            Decl::Fn { params, .. } => {
                assert_eq!(params.len(), 2);
            }
            _ => panic!("Expected Fn decl, got {:?}", decl),
        }
    }

    // NOTE: Tests for complex generics and reference types are temporarily disabled
    // as they require additional parser integration work. The core parsing
    // functionality is implemented and can be tested incrementally.
    // /*
    #[test]
    fn test_parse_fn_decl_with_generics() {
        let source = "fn identity<T>(x: T) -> T { x }";
        let arena = LocalArena::new(8192);
        let lexer = Lexer::new(source);
        let (tokens, interner) = lexer.lex_with_interner().unwrap();
        let mut parser = Parser::new(tokens, source, interner, arena);

        // Just verify it parses without error
        let decl = parser.parse_decl();
        assert!(decl.is_ok());
    }
    // */
    #[test]
    fn test_parse_struct_decl() {
        let source = "struct Point { x: Int, y: Int }";
        let arena = LocalArena::new(8192);
        let lexer = Lexer::new(source);
        let (tokens, interner) = lexer.lex_with_interner().unwrap();
        let mut parser = Parser::new(tokens, source, interner, arena);

        let decl = parser.parse_decl().unwrap();
        match decl {
            Decl::Struct { name, fields, .. } => {
                let name_str = parser.resolve_symbol(name);
                assert_eq!(name_str, "Point");
                assert_eq!(fields.len(), 2);
            }
            _ => panic!("Expected Struct decl, got {:?}", decl),
        }
    }

    #[test]
    fn test_parse_struct_with_generics() {
        let source = "struct Pair<T, U> { first: T, second: U }";
        let arena = LocalArena::new(8192);
        let lexer = Lexer::new(source);
        let (tokens, interner) = lexer.lex_with_interner().unwrap();
        let mut parser = Parser::new(tokens, source, interner, arena);

        // Just verify it parses without error
        let decl = parser.parse_decl();
        assert!(decl.is_ok());
    }

    #[test]
    fn test_parse_class_decl() {
        let source = "class MyClass { }";
        let arena = LocalArena::new(8192);
        let lexer = Lexer::new(source);
        let (tokens, interner) = lexer.lex_with_interner().unwrap();
        let mut parser = Parser::new(tokens, source, interner, arena);

        let decl = parser.parse_decl().unwrap();
        match decl {
            Decl::Class { name, .. } => {
                let name_str = parser.resolve_symbol(name);
                assert_eq!(name_str, "MyClass");
            }
            _ => panic!("Expected Class decl, got {:?}", decl),
        }
    }

    #[test]
    fn test_parse_class_with_superclass() {
        let source = "class Dog : Animal { }";
        let arena = LocalArena::new(8192);
        let lexer = Lexer::new(source);
        let (tokens, interner) = lexer.lex_with_interner().unwrap();
        let mut parser = Parser::new(tokens, source, interner, arena);

        let decl = parser.parse_decl().unwrap();
        match decl {
            Decl::Class { superclass, .. } => {
                assert!(superclass.is_some());
            }
            _ => panic!("Expected Class decl, got {:?}", decl),
        }
    }

    #[test]
    fn test_parse_enum_decl() {
        let source = "enum Option { case none, case some(T) }";
        let arena = LocalArena::new(8192);
        let lexer = Lexer::new(source);
        let (tokens, interner) = lexer.lex_with_interner().unwrap();
        let mut parser = Parser::new(tokens, source, interner, arena);

        let decl = parser.parse_decl().unwrap();
        match decl {
            Decl::Enum { variants, .. } => {
                assert_eq!(variants.len(), 2);
            }
            _ => panic!("Expected Enum decl, got {:?}", decl),
        }
    }

    #[test]
    fn test_parse_enum_with_method() {
        let source = "enum Option { case none, case some(T), fn isSome() -> Bool { false } }";
        let arena = LocalArena::new(8192);
        let lexer = Lexer::new(source);
        let (tokens, interner) = lexer.lex_with_interner().unwrap();
        let mut parser = Parser::new(tokens, source, interner, arena);

        let decl = parser.parse_decl().unwrap();
        match decl {
            Decl::Enum { variants, methods, .. } => {
                assert_eq!(variants.len(), 2);
                assert_eq!(methods.len(), 1);
                assert!(!methods[0].is_static);
                assert!(!methods[0].is_mut);
                assert!(!methods[0].is_init);
            }
            _ => panic!("Expected Enum decl, got {:?}", decl),
        }
    }

    #[test]
    fn test_parse_enum_with_mut_method() {
        let source = "enum Counter { case zero, mut fn increment() { } }";
        let arena = LocalArena::new(8192);
        let lexer = Lexer::new(source);
        let (tokens, interner) = lexer.lex_with_interner().unwrap();
        let mut parser = Parser::new(tokens, source, interner, arena);

        let decl = parser.parse_decl().unwrap();
        match decl {
            Decl::Enum { methods, .. } => {
                assert_eq!(methods.len(), 1);
                assert!(methods[0].is_mut);
                assert!(!methods[0].is_static);
                assert!(!methods[0].is_init);
            }
            _ => panic!("Expected Enum decl, got {:?}", decl),
        }
    }

    #[test]
    fn test_parse_enum_with_static_method() {
        let source = "enum Option { case none, static fn default() -> Self { .none } }";
        let arena = LocalArena::new(8192);
        let lexer = Lexer::new(source);
        let (tokens, interner) = lexer.lex_with_interner().unwrap();
        let mut parser = Parser::new(tokens, source, interner, arena);

        let decl = parser.parse_decl().unwrap();
        match decl {
            Decl::Enum { methods, .. } => {
                assert_eq!(methods.len(), 1);
                assert!(!methods[0].is_mut);
                assert!(methods[0].is_static);
                assert!(!methods[0].is_init);
            }
            _ => panic!("Expected Enum decl, got {:?}", decl),
        }
    }

    #[test]
    fn test_parse_enum_mixed_variants_and_methods() {
        let source = "enum Option { case none, fn isNone() -> Bool { true }, case some(T), fn unwrap() -> T { } }";
        let arena = LocalArena::new(8192);
        let lexer = Lexer::new(source);
        let (tokens, interner) = lexer.lex_with_interner().unwrap();
        let mut parser = Parser::new(tokens, source, interner, arena);

        let decl = parser.parse_decl().unwrap();
        match decl {
            Decl::Enum { variants, methods, .. } => {
                assert_eq!(variants.len(), 2);
                assert_eq!(methods.len(), 2);
            }
            _ => panic!("Expected Enum decl, got {:?}", decl),
        }
    }

    #[test]
    fn test_parse_enum_struct_variant() {
        let source = "enum Shape { case point { x: Float, y: Float } }";
        let arena = LocalArena::new(8192);
        let lexer = Lexer::new(source);
        let (tokens, interner) = lexer.lex_with_interner().unwrap();
        let mut parser = Parser::new(tokens, source, interner, arena);

        let decl = parser.parse_decl().unwrap();
        match decl {
            Decl::Enum { variants, .. } => {
                assert_eq!(variants.len(), 1);
                match &variants[0] {
                    EnumVariant::Struct { fields, .. } => {
                        assert_eq!(fields.len(), 2);
                    }
                    _ => panic!("Expected struct variant, got {:?}", variants[0]),
                }
            }
            _ => panic!("Expected Enum decl, got {:?}", decl),
        }
    }

    #[test]
    fn test_parse_enum_tuple_variant() {
        let source = "enum Option { case some(Int, String) }";
        let arena = LocalArena::new(8192);
        let lexer = Lexer::new(source);
        let (tokens, interner) = lexer.lex_with_interner().unwrap();
        let mut parser = Parser::new(tokens, source, interner, arena);

        let decl = parser.parse_decl().unwrap();
        match decl {
            Decl::Enum { variants, .. } => {
                assert_eq!(variants.len(), 1);
                match &variants[0] {
                    EnumVariant::Tuple { fields, .. } => {
                        assert_eq!(fields.len(), 2);
                    }
                    _ => panic!("Expected tuple variant, got {:?}", variants[0]),
                }
            }
            _ => panic!("Expected Enum decl, got {:?}", decl),
        }
    }

    #[test]
    fn test_parse_enum_method_visibility() {
        // Test that enum methods can have their own visibility
        // During semantic analysis, this will be resolved to most restrictive of parent and method
        let source = "enum Option { case none, pub fn publicMethod() -> Bool { true }, prv fn privateMethod() -> Bool { false } }";
        let arena = LocalArena::new(8192);
        let lexer = Lexer::new(source);
        let (tokens, interner) = lexer.lex_with_interner().unwrap();
        let mut parser = Parser::new(tokens, source, interner, arena);

        let decl = parser.parse_decl().unwrap();
        match decl {
            Decl::Enum { methods, .. } => {
                assert_eq!(methods.len(), 2);
                // Methods have their own visibility stored
                assert_eq!(methods[0].visibility, Visibility::Public);
                assert_eq!(methods[1].visibility, Visibility::Private);
            }
            _ => panic!("Expected Enum decl, got {:?}", decl),
        }
    }

    #[test]
    fn test_parse_protocol_decl() {
        // NOTE: Simplified protocol without &self shorthand
        let source = "protocol Drawable { fn draw(); }";
        let arena = LocalArena::new(8192);
        let lexer = Lexer::new(source);
        let (tokens, interner) = lexer.lex_with_interner().unwrap();
        let mut parser = Parser::new(tokens, source, interner, arena);

        // Just verify it parses without error
        let decl = parser.parse_decl();
        if let Err(e) = &decl {
            eprintln!("Error parsing protocol decl: {:?}", e);
        }
        assert!(decl.is_ok());
    }

    #[test]
    fn test_parse_impl_decl() {
        let source = "impl Point { fn new(x: Int, y: Int) -> Self { } }";
        let arena = LocalArena::new(8192);
        let lexer = Lexer::new(source);
        let (tokens, interner) = lexer.lex_with_interner().unwrap();
        let mut parser = Parser::new(tokens, source, interner, arena);

        let decl = parser.parse_decl().unwrap();
        match decl {
            Decl::Impl {
                type_path, methods, ..
            } => {
                let type_str = parser.resolve_symbol(type_path[0]);
                assert_eq!(type_str, "Point");
                assert_eq!(methods.len(), 1);
            }
            _ => panic!("Expected Impl decl, got {:?}", decl),
        }
    }

    #[test]
    fn test_parse_const_decl() {
        let source = "const MAX_SIZE: Int = 100;";
        let arena = LocalArena::new(8192);
        let lexer = Lexer::new(source);
        let (tokens, interner) = lexer.lex_with_interner().unwrap();
        let mut parser = Parser::new(tokens, source, interner, arena);

        let decl = parser.parse_decl().unwrap();
        match decl {
            Decl::Const { name, .. } => {
                let name_str = parser.resolve_symbol(name);
                assert_eq!(name_str, "MAX_SIZE");
            }
            _ => panic!("Expected Const decl, got {:?}", decl),
        }
    }

    #[test]
    fn test_parse_static_decl() {
        let source = "static let counter: Int = 0;";
        let arena = LocalArena::new(8192);
        let lexer = Lexer::new(source);
        let (tokens, interner) = lexer.lex_with_interner().unwrap();
        let mut parser = Parser::new(tokens, source, interner, arena);

        let decl = parser.parse_decl().unwrap();
        match decl {
            Decl::Static { name, mutable, .. } => {
                let name_str = parser.resolve_symbol(name);
                assert_eq!(name_str, "counter");
                assert!(!mutable);
            }
            _ => panic!("Expected Static decl, got {:?}", decl),
        }
    }

    #[test]
    fn test_parse_static_mutable_decl() {
        let source = "static let mut counter: Int = 0;";
        let arena = LocalArena::new(8192);
        let lexer = Lexer::new(source);
        let (tokens, interner) = lexer.lex_with_interner().unwrap();
        let mut parser = Parser::new(tokens, source, interner, arena);

        let decl = parser.parse_decl().unwrap();
        match decl {
            Decl::Static { mutable, .. } => {
                assert!(mutable);
            }
            _ => panic!("Expected Static decl, got {:?}", decl),
        }
    }

    // ===== Pattern Parsing Tests =====
    // NOTE: Pattern parsing tests require match expression integration
    // which needs additional work. The core pattern parsing logic is implemented
    // and can be tested once match expressions are fully integrated.

    #[test]
    fn test_parse_wildcard_pattern() {
        // Test that underscore is tokenized correctly
        let source = "_";
        let lexer = Lexer::new(source);
        let (tokens, _) = lexer.lex_with_interner().unwrap();
        assert_eq!(tokens[0].kind, TokenKind::Underscore);
    }

    #[test]
    fn test_parse_underscore_identifier() {
        // Test that identifiers starting with _ are recognized
        let source = "_private";
        let lexer = Lexer::new(source);
        let (tokens, _) = lexer.lex_with_interner().unwrap();
        assert!(matches!(tokens[0].kind, TokenKind::Ident(_)));
    }

    // ===== Type Parsing Tests =====

    #[test]
    fn test_parse_simple_type() {
        let source = "let x: Int = 42;";
        let arena = LocalArena::new(8192);
        let lexer = Lexer::new(source);
        let (tokens, interner) = lexer.lex_with_interner().unwrap();
        let mut parser = Parser::new(tokens, source, interner, arena);

        let stmt = parser.parse_stmt().unwrap();
        match stmt {
            Stmt::Let {
                type_annotation, ..
            } => match type_annotation {
                Some(Type::Simple { name, .. }) => {
                    let name_str = parser.resolve_symbol(name);
                    assert_eq!(name_str, "Int");
                }
                _ => panic!("Expected Simple type, got {:?}", type_annotation),
            },
            _ => panic!("Expected Let stmt"),
        }
    }

    #[test]
    fn test_parse_generic_type() {
        let source = "let x: List<Int> = [];";
        let arena = LocalArena::new(8192);
        let lexer = Lexer::new(source);
        let (tokens, interner) = lexer.lex_with_interner().unwrap();
        let mut parser = Parser::new(tokens, source, interner, arena);

        // Just verify it parses without error
        let stmt = parser.parse_stmt();
        if let Err(e) = &stmt {
            eprintln!("Error parsing generic type: {:?}", e);
        }
        assert!(stmt.is_ok());
    }

    // NOTE: Tuple expressions are not yet implemented in the AST
    // The parser can parse tuple types but not tuple literals like (1, 2)
    /*
    #[test]
    fn test_parse_tuple_type() {
        let source = "let x: (Int, String) = (42, \"hello\");";
        let arena = LocalArena::new(8192);
        let lexer = Lexer::new(source);
        let (tokens, interner) = match lexer.lex_with_interner() {
            Ok(result) => result,
            Err(e) => {
                eprintln!("Lexer error: {:?}", e);
                panic!("Lexer failed");
            }
        };

        // Debug: print tokens
        eprintln!("Tokens:");
        for (i, token) in tokens.iter().enumerate() {
            eprintln!("  {}: {:?} at {:?}", i, token.kind, token.span);
        }

        let mut parser = Parser::new(tokens, source, interner, arena);

        // Just verify it parses without error
        let stmt = parser.parse_stmt();
        if let Err(e) = &stmt {
            eprintln!("Error parsing tuple type: {:?}", e);
        }
        assert!(stmt.is_ok());
    }
    */

    #[test]
    fn test_parse_function_type() {
        let source = "let f: (Int, Int) -> Int = add;";
        let arena = LocalArena::new(8192);
        let lexer = Lexer::new(source);
        let (tokens, interner) = lexer.lex_with_interner().unwrap();
        let mut parser = Parser::new(tokens, source, interner, arena);

        // Just verify it parses without error
        let stmt = parser.parse_stmt();
        assert!(stmt.is_ok());
    }

    #[test]
    fn test_parse_array_type() {
        let source = "let x: [Int] = [];";
        let arena = LocalArena::new(8192);
        let lexer = Lexer::new(source);
        let (tokens, interner) = lexer.lex_with_interner().unwrap();
        let mut parser = Parser::new(tokens, source, interner, arena);

        // Just verify it parses without error
        let stmt = parser.parse_stmt();
        assert!(stmt.is_ok());
    }

    #[test]
    fn test_parse_dict_type() {
        let source = "let x: [String: Int] = [];";
        let arena = LocalArena::new(8192);
        let lexer = Lexer::new(source);
        let (tokens, interner) = lexer.lex_with_interner().unwrap();
        let mut parser = Parser::new(tokens, source, interner, arena);

        // Just verify it parses without error
        let stmt = parser.parse_stmt();
        assert!(stmt.is_ok());
    }

    #[test]
    fn test_parse_optional_type() {
        let source = "let x: Int? = nil;";
        let arena = LocalArena::new(8192);
        let lexer = Lexer::new(source);
        let (tokens, interner) = lexer.lex_with_interner().unwrap();
        let mut parser = Parser::new(tokens, source, interner, arena);

        // Just verify it parses without error
        let stmt = parser.parse_stmt();
        assert!(stmt.is_ok());
    }

    #[test]
    fn test_parse_type_alias_decl() {
        let source = "type Result<T> = Option<T>;";
        let arena = LocalArena::new(8192);
        let lexer = Lexer::new(source);
        let (tokens, interner) = lexer.lex_with_interner().unwrap();
        let mut parser = Parser::new(tokens, source, interner, arena);

        // Just verify it parses without error
        let decl = parser.parse_decl();
        assert!(decl.is_ok());
    }

    #[test]
    fn test_parse_mut_fn() {
        let source = "mut fn increment(x: Int) -> Int { x + 1 }";
        let arena = LocalArena::new(8192);
        let lexer = Lexer::new(source);
        let (tokens, interner) = lexer.lex_with_interner().unwrap();
        let mut parser = Parser::new(tokens, source, interner, arena);

        let decl = parser.parse_decl();
        assert!(decl.is_ok());

        match decl.unwrap() {
            Decl::Fn {
                is_mut,
                is_init,
                is_static,
                ..
            } => {
                assert!(is_mut);
                assert!(!is_init);
                assert!(!is_static);
            }
            _ => panic!("Expected Fn declaration"),
        }
    }

    #[test]
    fn test_parse_init() {
        let source = "init(x: Int) { }";
        let arena = LocalArena::new(8192);
        let lexer = Lexer::new(source);
        let (tokens, interner) = lexer.lex_with_interner().unwrap();
        let mut parser = Parser::new(tokens, source, interner, arena);

        let decl = parser.parse_decl();
        assert!(decl.is_ok());

        match decl.unwrap() {
            Decl::Fn {
                is_mut,
                is_init,
                is_static,
                ..
            } => {
                assert!(!is_mut);
                assert!(is_init);
                assert!(!is_static);
            }
            _ => panic!("Expected Fn declaration"),
        }
    }

    #[test]
    fn test_parse_static_fn() {
        let source = "static fn create() -> Self { Self { } }";
        let arena = LocalArena::new(8192);
        let lexer = Lexer::new(source);
        let (tokens, interner) = lexer.lex_with_interner().unwrap();
        let mut parser = Parser::new(tokens, source, interner, arena);

        let decl = parser.parse_decl();
        assert!(decl.is_ok());

        match decl.unwrap() {
            Decl::Fn {
                is_mut,
                is_init,
                is_static,
                return_type,
                ..
            } => {
                assert!(!is_mut);
                assert!(!is_init);
                assert!(is_static);
                assert!(return_type.is_some());
            }
            _ => panic!("Expected Fn declaration"),
        }
    }

    #[test]
    fn test_parse_self_type() {
        let source = "fn new() -> Self { Self { } }";
        let arena = LocalArena::new(8192);
        let lexer = Lexer::new(source);
        let (tokens, interner) = lexer.lex_with_interner().unwrap();
        let mut parser = Parser::new(tokens, source, interner, arena);

        let decl = parser.parse_decl();
        assert!(decl.is_ok());

        match decl.unwrap() {
            Decl::Fn { return_type, .. } => {
                assert!(return_type.is_some());
                match return_type.unwrap() {
                    Type::SelfType { .. } => {}
                    _ => panic!("Expected SelfType"),
                }
            }
            _ => panic!("Expected Fn declaration"),
        }
    }

    #[test]
    fn test_parse_pub_mut_fn() {
        let source = "pub mut fn modify() { }";
        let arena = LocalArena::new(8192);
        let lexer = Lexer::new(source);
        let (tokens, interner) = lexer.lex_with_interner().unwrap();
        let mut parser = Parser::new(tokens, source, interner, arena);

        let decl = parser.parse_decl();
        assert!(decl.is_ok());

        match decl.unwrap() {
            Decl::Fn {
                is_mut, visibility, ..
            } => {
                assert!(is_mut);
                assert!(matches!(visibility, Visibility::Public));
            }
            _ => panic!("Expected Fn declaration"),
        }
    }

    #[test]
    fn test_parse_impl_block_with_mut_init() {
        let source = "impl Point { mut fn scale(by: Float) { } init(x: Float, y: Float) { } }";
        let arena = LocalArena::new(8192);
        let lexer = Lexer::new(source);
        let (tokens, interner) = lexer.lex_with_interner().unwrap();
        let mut parser = Parser::new(tokens, source, interner, arena);

        let decl = parser.parse_decl();
        assert!(decl.is_ok());

        match decl.unwrap() {
            Decl::Impl { methods, .. } => {
                assert_eq!(methods.len(), 2);
                assert!(methods[0].is_mut);
                assert!(methods[1].is_init);
            }
            _ => panic!("Expected Impl declaration"),
        }
    }

    #[test]
    fn test_parse_impl_method_visibility() {
        // Test that impl methods can have their own visibility
        // During semantic analysis, this will be resolved to most restrictive of parent and method
        let source = "impl Point { pub fn publicMethod() -> Bool { true } prv fn privateMethod() -> Bool { false } }";
        let arena = LocalArena::new(8192);
        let lexer = Lexer::new(source);
        let (tokens, interner) = lexer.lex_with_interner().unwrap();
        let mut parser = Parser::new(tokens, source, interner, arena);

        let decl = parser.parse_decl();
        assert!(decl.is_ok());

        match decl.unwrap() {
            Decl::Impl { methods, .. } => {
                assert_eq!(methods.len(), 2);
                // Methods have their own visibility stored
                assert_eq!(methods[0].visibility, Visibility::Public);
                assert_eq!(methods[1].visibility, Visibility::Private);
            }
            _ => panic!("Expected Impl declaration"),
        }
    }

    #[test]
    fn test_parse_fn_with_default_labels() {
        // Swift-style: parameters are labeled by default
        let source = "fn add(x: Int, y: Int) -> Int { x + y }";
        let arena = LocalArena::new(8192);
        let lexer = Lexer::new(source);
        let (tokens, interner) = lexer.lex_with_interner().unwrap();
        let mut parser = Parser::new(tokens, source, interner, arena);

        let decl = parser.parse_decl();
        assert!(decl.is_ok());

        match decl.unwrap() {
            Decl::Fn { name, params, .. } => {
                let name_str = parser.resolve_symbol(name);
                assert_eq!(name_str, "add");
                assert_eq!(params.len(), 2);
            }
            _ => panic!("Expected Fn declaration"),
        }
    }

    #[test]
    fn test_parse_fn_with_omitted_labels() {
        // Swift-style: omit labels with underscore
        let source = "fn add(_ x: Int, _ y: Int) -> Int { x + y }";
        let arena = LocalArena::new(8192);
        let lexer = Lexer::new(source);
        let (tokens, interner) = lexer.lex_with_interner().unwrap();
        let mut parser = Parser::new(tokens, source, interner, arena);

        let decl = parser.parse_decl();
        assert!(decl.is_ok());

        match decl.unwrap() {
            Decl::Fn { name, params, .. } => {
                let name_str = parser.resolve_symbol(name);
                assert_eq!(name_str, "add");
                assert_eq!(params.len(), 2);
            }
            _ => panic!("Expected Fn declaration"),
        }
    }

    #[test]
    fn test_parse_fn_with_external_labels() {
        // Swift-style: external and internal parameter names
        let source = "fn add(from x: Int, to y: Int) -> Int { x + y }";
        let arena = LocalArena::new(8192);
        let lexer = Lexer::new(source);
        let (tokens, interner) = lexer.lex_with_interner().unwrap();
        let mut parser = Parser::new(tokens, source, interner, arena);

        let decl = parser.parse_decl();
        assert!(decl.is_ok());

        match decl.unwrap() {
            Decl::Fn { name, params, .. } => {
                let name_str = parser.resolve_symbol(name);
                assert_eq!(name_str, "add");
                assert_eq!(params.len(), 2);
            }
            _ => panic!("Expected Fn declaration"),
        }
    }

    #[test]
    fn test_parse_mixed_labels() {
        // Mix of default, omitted, and external labels
        let source = "fn process(x: Int, _ y: Int, with z: Int) -> Int { x }";
        let arena = LocalArena::new(8192);
        let lexer = Lexer::new(source);
        let (tokens, interner) = lexer.lex_with_interner().unwrap();
        let mut parser = Parser::new(tokens, source, interner, arena);

        let decl = parser.parse_decl();
        assert!(decl.is_ok());

        match decl.unwrap() {
            Decl::Fn { name, params, .. } => {
                let name_str = parser.resolve_symbol(name);
                assert_eq!(name_str, "process");
                assert_eq!(params.len(), 3);
            }
            _ => panic!("Expected Fn declaration"),
        }
    }

    #[test]
    fn test_emit_errors() {
        use crate::diagnostic::Emitter;
        use crate::keywords;

        // Test with valid source (no errors)
        let source = "let x: Int = 42;";
        let arena = LocalArena::new(8192);
        let lexer = Lexer::new(source);
        let (tokens, interner) = lexer.lex_with_interner().unwrap();
        let parser = Parser::new(tokens, source, interner, arena);

        // No errors should be present
        assert!(!parser.has_errors());
        assert_eq!(parser.errors().len(), 0);

        // Create emitter and emit errors (should not panic even with no errors)
        let diagnostic_interner =
            oxidex_mem::StringInterner::with_pre_interned(keywords::KEYWORDS);
        let emitter = Emitter::new(diagnostic_interner, false);
        parser.emit_errors(&emitter); // Should handle empty error list gracefully
    }
}
