//! Pretty-printer for `OxideX` AST.
//!
//! This module converts AST nodes back to source code, enabling:
//! - Debugging and validation
//! - Code formatting
//! - Round-trip testing (parse → print → parse)
//! - AST inspection

use crate::ast::{Expr, Stmt, Type};
use crate::ast::expr::InterpolationPart;
use oxidex_mem::StringInterner;
use std::fmt;

/// Configuration for pretty-printing.
#[derive(Debug, Clone)]
pub struct PrettyConfig {
    /// Indentation string (e.g., "  " or "\t")
    pub indent: String,
    /// Maximum line width before wrapping
    pub width: usize,
    /// Use trailing commas in multi-line constructs
    pub trailing_commas: bool,
}

impl Default for PrettyConfig {
    fn default() -> Self {
        Self {
            indent: "  ".to_string(),
            width: 80,
            trailing_commas: true,
        }
    }
}

/// Pretty-printer for `OxideX` AST.
pub struct PrettyPrinter {
    /// String interner for resolving symbols
    interner: StringInterner,
    /// Configuration
    config: PrettyConfig,
    /// Current indentation level
    indent_level: usize,
}

impl PrettyPrinter {
    /// Creates a new pretty-printer.
    #[must_use]
    pub fn new(interner: StringInterner) -> Self {
        Self {
            interner,
            config: PrettyConfig::default(),
            indent_level: 0,
        }
    }

    /// Sets the configuration.
    #[must_use]
    pub fn with_config(mut self, config: PrettyConfig) -> Self {
        self.config = config;
        self
    }

    /// Returns the current indentation string.
    fn current_indent(&self) -> String {
        self.config.indent.repeat(self.indent_level)
    }

    /// Pretty-prints an expression.
    pub fn print_expr(&mut self, expr: &Expr) -> String {
        match expr {
            Expr::IntegerLiteral {
                value, type_suffix, ..
            } => {
                let text = self.interner.resolve(*value).unwrap_or("<unknown>");
                if let Some(suffix) = type_suffix {
                    let suffix_text =
                        self.interner.resolve(*suffix).unwrap_or("");
                    format!("{text}{suffix_text}")
                } else {
                    text.to_string()
                }
            }

            Expr::FloatLiteral {
                value, type_suffix, ..
            } => {
                let text = self.interner.resolve(*value).unwrap_or("<unknown>");
                if let Some(suffix) = type_suffix {
                    let suffix_text =
                        self.interner.resolve(*suffix).unwrap_or("");
                    format!("{text}{suffix_text}")
                } else {
                    text.to_string()
                }
            }

            Expr::StringLiteral { value, .. } => {
                let text = self.interner.resolve(*value).unwrap_or("<unknown>");
                format!("\"{text}\"")
            }

            Expr::BoolLiteral { value, .. } => value.to_string(),

            Expr::Nil { .. } => "nil".to_string(),

            Expr::Identifier(sym) => self
                .interner
                .resolve(*sym)
                .unwrap_or("<unknown>")
                .to_string(),

            Expr::Path { segments, .. } => segments
                .iter()
                .map(|sym| self.interner.resolve(*sym).unwrap_or("<unknown>"))
                .collect::<Vec<_>>()
                .join("::"),

            Expr::Unary { op, operand, .. } => {
                let operand_str = self.print_expr(operand);
                format!("{op}{operand_str}")
            }

            Expr::Binary {
                left, op, right, ..
            } => {
                let left_str = self.print_expr(left);
                let right_str = self.print_expr(right);
                format!("{left_str} {op} {right_str}")
            }

            Expr::Paren { expr, .. } => {
                let inner = self.print_expr(expr);
                format!("({inner})")
            }

            Expr::Block { stmts, expr, .. } => {
                self.indent_level += 1;
                let mut result = "{\n".to_string();

                for stmt in stmts {
                    result.push_str(&self.current_indent());
                    result.push_str(&self.print_stmt(stmt));
                    result.push('\n');
                }

                if let Some(e) = expr {
                    result.push_str(&self.current_indent());
                    result.push_str(&self.print_expr(e));
                    result.push('\n');
                }

                self.indent_level -= 1;
                result.push_str(&self.current_indent());
                result.push('}');

                result
            }

            Expr::Call { callee, args, .. } => {
                let callee_str = self.print_expr(callee);
                let mut args_strings = Vec::new();
                for arg in args {
                    if let Some(label) = arg.label {
                        let label_text: String = self
                            .interner
                            .resolve(label)
                            .unwrap_or("<unknown>")
                            .to_string();
                        let value_str = self.print_expr(arg.value);
                        args_strings.push(format!("{label_text}: {value_str}"));
                    } else {
                        args_strings.push(self.print_expr(arg.value));
                    }
                }
                let args_str = args_strings.join(", ");
                format!("{callee_str}({args_str})")
            }

            Expr::Array { elements, .. } => {
                let elems_str = elements
                    .iter()
                    .map(|e| self.print_expr(e))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("[{elems_str}]")
            }

            Expr::Field { object, field, .. } => {
                let object_str = self.print_expr(object);
                let field_text =
                    self.interner.resolve(*field).unwrap_or("<unknown>");
                format!("{object_str}.{field_text}")
            }

            Expr::Index {
                collection, index, ..
            } => {
                let collection_str = self.print_expr(collection);
                let index_str = self.print_expr(index);
                format!("{collection_str}[{index_str}]")
            }

            Expr::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                let condition_str = self.print_expr(condition);
                let then_str = self.print_expr(then_branch);
                match else_branch {
                    Some(else_expr) => {
                        let else_str = self.print_expr(else_expr);
                        format!("if {condition_str} {then_str} else {else_str}")
                    }
                    None => format!("if {condition_str} {then_str}"),
                }
            }

            Expr::Match {
                scrutinee,
                arms,
                ..
            } => {
                let scrutinee_str = self.print_expr(scrutinee);
                self.indent_level += 1;
                let mut arms_str = String::from("match {\n");
                arms_str.push_str(&self.current_indent());
                arms_str.push_str(&scrutinee_str);
                arms_str.push_str(" {\n");

                for arm in arms {
                    self.indent_level += 1;
                    arms_str.push_str(&self.current_indent());
                    arms_str.push_str(&self.print_pattern(&arm.pattern));
                    if let Some(guard) = &arm.guard {
                        arms_str.push_str(" if ");
                        arms_str.push_str(&self.print_expr(guard));
                    }
                    arms_str.push_str(" => ");
                    arms_str.push_str(&self.print_expr(arm.body));
                    arms_str.push_str(",\n");
                    self.indent_level -= 1;
                }

                self.indent_level -= 1;
                arms_str.push_str(&self.current_indent());
                arms_str.push_str("}\n");
                arms_str.push_str(&self.current_indent());
                arms_str.push('}');
                arms_str
            }

            Expr::ForLoop {
                pattern,
                iter,
                body,
                ..
            } => {
                let pattern_str = self.print_pattern(pattern);
                let iter_str = self.print_expr(iter);
                let body_str = self.print_expr(body);
                format!("for {pattern_str} in {iter_str} {body_str}")
            }

            Expr::WhileLoop {
                condition, body, ..
            } => {
                let condition_str = self.print_expr(condition);
                let body_str = self.print_expr(body);
                format!("while {condition_str} {body_str}")
            }

            Expr::MethodCall {
                receiver,
                method,
                args,
                ..
            } => {
                let receiver_str = self.print_expr(receiver);
                let method_text: String =
                    self.interner.resolve(*method).unwrap_or("<unknown>").to_string();
                let mut args_strings = Vec::new();
                for arg in args {
                    if let Some(label) = arg.label {
                        let label_text: String = self
                            .interner
                            .resolve(label)
                            .unwrap_or("<unknown>")
                            .to_string();
                        let value_str = self.print_expr(arg.value);
                        args_strings.push(format!("{label_text}: {value_str}"));
                    } else {
                        args_strings.push(self.print_expr(arg.value));
                    }
                }
                let args_str = args_strings.join(", ");
                format!("{receiver_str}.{method_text}({args_str})")
            }

            Expr::Struct {
                type_path,
                fields,
                ..
            } => {
                let type_str = type_path
                    .iter()
                    .map(|sym| self.interner.resolve(*sym).unwrap_or("<unknown>"))
                    .collect::<Vec<_>>()
                    .join("::");
                let mut fields_strings = Vec::new();
                for field in fields {
                    let name_text: String =
                        self.interner.resolve(field.name).unwrap_or("<unknown>").to_string();
                    match field.value {
                        Some(value) => {
                            let value_str = self.print_expr(value);
                            fields_strings.push(format!("{name_text}: {value_str}"));
                        }
                        None => {
                            // Shorthand initialization
                            fields_strings.push(name_text);
                        }
                    }
                }
                let fields_str = fields_strings.join(", ");
                format!("{type_str} {{ {fields_str} }}")
            }

            Expr::Enum {
                type_path,
                variant,
                payload,
                ..
            } => {
                let type_str = type_path
                    .iter()
                    .map(|sym| self.interner.resolve(*sym).unwrap_or("<unknown>"))
                    .collect::<Vec<_>>()
                    .join("::");
                let variant_text: String =
                    self.interner.resolve(*variant).unwrap_or("<unknown>").to_string();
                match payload {
                    Some(inner) => {
                        let inner_str = self.print_expr(inner);
                        format!("{type_str}::{variant_text}({inner_str})")
                    }
                    None => format!("{type_str}::{variant_text}"),
                }
            }

            Expr::Dict { entries, .. } => {
                let entries_str = entries
                    .iter()
                    .map(|entry| {
                        let key_str = self.print_expr(entry.key);
                        let value_str = self.print_expr(entry.value);
                        format!("{key_str}: {value_str}")
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{{{entries_str}}}")
            }

            Expr::Interpolation { parts, .. } => {
                let mut result = String::from("\"");
                for part in parts {
                    match part {
                        InterpolationPart::Text(sym) => {
                            let text =
                                self.interner.resolve(*sym).unwrap_or("<unknown>");
                            result.push_str(text);
                        }
                        InterpolationPart::Expr(expr) => {
                            result.push_str("\\(");
                            result.push_str(&self.print_expr(expr));
                            result.push(')');
                        }
                    }
                }
                result.push('"');
                result
            },
        }
    }

    /// Pretty-prints a statement.
    pub fn print_stmt(&mut self, stmt: &Stmt) -> String {
        match stmt {
            Stmt::Expr { expr, .. } => {
                format!("{};", self.print_expr(expr))
            }

            Stmt::Return { value, .. } => {
                if let Some(v) = value {
                    format!("return {};", self.print_expr(v))
                } else {
                    "return;".to_string()
                }
            }

            Stmt::Let {
                name,
                type_annotation,
                init,
                ..
            } => {
                let name_text: String =
                    self.interner.resolve(*name).unwrap_or("<unknown>").to_string();
                let type_str = type_annotation
                    .as_ref()
                    .map(|t| format!(": {}", self.print_type(t)))
                    .unwrap_or_default();
                let init_str = if let Some(e) = init {
                    format!(" = {}", self.print_expr(e))
                } else {
                    String::new()
                };
                format!("let {name_text}{type_str}{init_str};")
            }

            Stmt::Mut {
                name,
                type_annotation,
                init,
                ..
            } => {
                let name_text: String =
                    self.interner.resolve(*name).unwrap_or("<unknown>").to_string();
                let type_str = type_annotation
                    .as_ref()
                    .map(|t| format!(": {}", self.print_type(t)))
                    .unwrap_or_default();
                let init_str = if let Some(e) = init {
                    format!(" = {}", self.print_expr(e))
                } else {
                    String::new()
                };
                format!("mut {name_text}{type_str}{init_str};")
            }

            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                let condition_str = self.print_expr(condition);
                let then_str = self.print_expr(then_branch);
                match else_branch {
                    Some(else_expr) => {
                        let else_str = self.print_expr(else_expr);
                        format!(
                            "if {condition_str} {then_str} else {else_str}"
                        )
                    }
                    None => format!("if {condition_str} {then_str}"),
                }
            }

            Stmt::Guard {
                condition,
                else_branch,
                ..
            } => {
                let condition_str = self.print_expr(condition);
                let else_str = self.print_expr(else_branch);
                format!("guard {condition_str} else {else_str}")
            }

            Stmt::Match {
                scrutinee,
                arms,
                ..
            } => {
                let scrutinee_str = self.print_expr(scrutinee);
                self.indent_level += 1;
                let mut arms_str = String::from("match ");
                arms_str.push_str(&scrutinee_str);
                arms_str.push_str(" {\n");

                for arm in arms {
                    self.indent_level += 1;
                    arms_str.push_str(&self.current_indent());
                    arms_str.push_str(&self.print_pattern(&arm.pattern));
                    if let Some(guard) = &arm.guard {
                        arms_str.push_str(" if ");
                        arms_str.push_str(&self.print_expr(guard));
                    }
                    arms_str.push_str(" => ");
                    arms_str.push_str(&self.print_expr(arm.body));
                    arms_str.push_str(",\n");
                    self.indent_level -= 1;
                }

                self.indent_level -= 1;
                arms_str.push_str(&self.current_indent());
                arms_str.push('}');
                arms_str
            }

            Stmt::ForLoop {
                pattern,
                iter,
                body,
                ..
            } => {
                let pattern_str = self.print_pattern(pattern);
                let iter_str = self.print_expr(iter);
                let body_str = self.print_expr(body);
                format!("for {pattern_str} in {iter_str} {body_str}")
            }

            Stmt::WhileLoop {
                condition, body, ..
            } => {
                let condition_str = self.print_expr(condition);
                let body_str = self.print_expr(body);
                format!("while {condition_str} {body_str}")
            }

            Stmt::Assign {
                target, value, ..
            } => {
                let target_str = self.print_expr(target);
                let value_str = self.print_expr(value);
                format!("{target_str} = {value_str};")
            }
        }
    }

    /// Pretty-prints a type annotation.
    #[must_use]
    pub fn print_type(&self, ty: &Type) -> String {
        match ty {
            Type::Simple { name, .. } => self
                .interner
                .resolve(*name)
                .unwrap_or("<unknown>")
                .to_string(),

            Type::Tuple { elements, .. } => {
                let elems_str = elements
                    .iter()
                    .map(|t| self.print_type(t))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("({elems_str})")
            }

            Type::Generic { name, params, .. } => {
                let name_text =
                    self.interner.resolve(*name).unwrap_or("<unknown>");
                let params_str = params
                    .iter()
                    .map(|t| self.print_type(t))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{name_text}<{params_str}>")
            }

            Type::Function {
                params, return_type, ..
            } => {
                let params_str = params
                    .iter()
                    .map(|t| self.print_type(t))
                    .collect::<Vec<_>>()
                    .join(", ");
                let return_str = self.print_type(return_type);
                format!("fn({params_str}) -> {return_str}")
            }

            Type::Array {
                element, size, ..
            } => {
                let elem_str = self.print_type(element);
                match size {
                    Some(sym) => {
                        let size_text =
                            self.interner.resolve(*sym).unwrap_or("<unknown>");
                        format!("[{elem_str}; {size_text}]")
                    }
                    None => format!("[{elem_str}]"),
                }
            }

            Type::Dict { key, value, .. } => {
                let key_str = self.print_type(key);
                let value_str = self.print_type(value);
                format!("[{key_str}: {value_str}]")
            }

            Type::Optional { inner, .. } => {
                let inner_str = self.print_type(inner);
                format!("{inner_str}?")
            }

            Type::Reference {
                inner, mutable, ..
            } => {
                let inner_str = self.print_type(inner);
                if *mutable {
                    format!("mut {inner_str}")
                } else {
                    format!("&{inner_str}")
                }
            }

            Type::Inferred { .. } => "_".to_string(),
        }
    }

    /// Pretty-prints a pattern.
    fn print_pattern(&self, pattern: &crate::ast::Pattern) -> String {
        match pattern {
            crate::ast::Pattern::Wildcard { .. } => "_".to_string(),

            crate::ast::Pattern::Literal { value, .. } => {
                // For literals, we just format the token kind directly
                format!("{value}")
            }

            crate::ast::Pattern::Variable { name, mutable, .. } => {
                let name_text =
                    self.interner.resolve(*name).unwrap_or("<unknown>");
                if *mutable {
                    format!("mut {name_text}")
                } else {
                    name_text.to_string()
                }
            }
            crate::ast::Pattern::Struct {
                type_path,
                fields,
                ..
            } => {
                let type_str = type_path
                    .iter()
                    .map(|s| self.interner.resolve(*s).unwrap_or("<unknown>"))
                    .collect::<Vec<_>>()
                    .join("::");
                let fields_str = fields
                    .iter()
                    .map(|f| {
                        let name =
                            self.interner.resolve(f.name).unwrap_or("<unknown>");
                        if let Some(pattern) = &f.pattern {
                            format!("{name}: {}", self.print_pattern(pattern))
                        } else {
                            name.to_string()
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{type_str} {{ {fields_str} }}")
            }

            crate::ast::Pattern::Enum {
                type_path,
                variant,
                payload,
                ..
            } => {
                let type_str = type_path
                    .iter()
                    .map(|s| self.interner.resolve(*s).unwrap_or("<unknown>"))
                    .collect::<Vec<_>>()
                    .join("::");
                let variant_text =
                    self.interner.resolve(*variant).unwrap_or("<unknown>");
                match payload {
                    Some(inner) => {
                        format!("{type_str}::{variant_text}({})", self.print_pattern(inner))
                    }
                    None => format!("{type_str}::{variant_text}"),
                }
            }

            crate::ast::Pattern::Tuple {
                elements, ..
            } => {
                let elems_str = elements
                    .iter()
                    .map(|p| self.print_pattern(p))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("({elems_str})")
            }
            crate::ast::Pattern::Array {
                elements, rest, ..
            } => {
                let elems_str = elements
                    .iter()
                    .map(|p| self.print_pattern(p))
                    .collect::<Vec<_>>()
                    .join(", ");
                if rest.is_some() {
                    format!("[{elems_str}, ..]")
                } else {
                    format!("[{elems_str}]")
                }
            }

            crate::ast::Pattern::Or { left, right, .. } => {
                format!("{} | {}", self.print_pattern(left), self.print_pattern(right))
            }
        }
    }
}

impl fmt::Display for Expr<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // This is a placeholder - requires interner
        write!(f, "<expression>")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Expr, Type};
    use crate::keywords;
    use crate::Span;

    // Helper function to create a printer with pre-interned keywords
    fn printer() -> PrettyPrinter {
        let interner = StringInterner::with_pre_interned(keywords::KEYWORDS);
        PrettyPrinter::new(interner)
    }

    // Helper to create symbols for testing
    fn sym(interner: &mut StringInterner, name: &str) -> oxidex_mem::Symbol {
        interner.intern(name)
    }

    #[test]
    fn test_pretty_config_default() {
        let config = PrettyConfig::default();
        assert_eq!(config.indent, "  ");
        assert_eq!(config.width, 80);
        assert!(config.trailing_commas);
    }

    #[test]
    fn test_pretty_printer_new() {
        let interner = StringInterner::new();
        let printer = PrettyPrinter::new(interner);
        assert_eq!(printer.indent_level, 0);
    }

    // ===== Expression Tests =====

    #[test]
    fn test_print_expr_integer() {
        let mut interner = StringInterner::new();
        let val = sym(&mut interner, "42");
        let expr = Expr::IntegerLiteral {
            value: val,
            type_suffix: None,
            span: Span::new(0, 2, 1, 1, 1, 3),
        };
        let mut printer = PrettyPrinter::new(interner);
        assert_eq!(printer.print_expr(&expr), "42");
    }

    #[test]
    fn test_print_expr_float() {
        let mut interner = StringInterner::new();
        let val = sym(&mut interner, "3.14");
        let expr = Expr::FloatLiteral {
            value: val,
            type_suffix: None,
            span: Span::new(0, 4, 1, 1, 1, 5),
        };
        let mut printer = PrettyPrinter::new(interner);
        assert_eq!(printer.print_expr(&expr), "3.14");
    }

    #[test]
    fn test_print_expr_string() {
        let mut interner = StringInterner::new();
        let val = sym(&mut interner, "hello");
        let expr = Expr::StringLiteral {
            value: val,
            span: Span::new(0, 6, 1, 1, 1, 7),
        };
        let mut printer = PrettyPrinter::new(interner);
        assert_eq!(printer.print_expr(&expr), "\"hello\"");
    }

    #[test]
    fn test_print_expr_bool() {
        let expr = Expr::BoolLiteral {
            value: true,
            span: Span::new(0, 4, 1, 1, 1, 5),
        };
        let mut printer = printer();
        assert_eq!(printer.print_expr(&expr), "true");

        let expr = Expr::BoolLiteral {
            value: false,
            span: Span::new(0, 5, 1, 1, 1, 6),
        };
        assert_eq!(printer.print_expr(&expr), "false");
    }

    #[test]
    fn test_print_expr_nil() {
        let expr = Expr::Nil {
            span: Span::new(0, 3, 1, 1, 1, 4),
        };
        let mut printer = printer();
        assert_eq!(printer.print_expr(&expr), "nil");
    }

    #[test]
    fn test_print_expr_identifier() {
        let mut interner = StringInterner::new();
        let name = sym(&mut interner, "x");
        let expr = Expr::Identifier(name);
        let mut printer = PrettyPrinter::new(interner);
        assert_eq!(printer.print_expr(&expr), "x");
    }

    #[test]
    fn test_print_expr_path() {
        let mut interner = StringInterner::new();
        let s1 = sym(&mut interner, "std");
        let s2 = sym(&mut interner, "collections");
        let s3 = sym(&mut interner, "Map");
        let expr = Expr::Path {
            segments: vec![s1, s2, s3],
            span: Span::new(0, 18, 1, 1, 1, 19),
        };
        let mut printer = PrettyPrinter::new(interner);
        assert_eq!(printer.print_expr(&expr), "std::collections::Map");
    }

    // Note: Tests requiring arena allocation (unary, binary, if, block, call, array, index, let, mut, return, expr)
    // are omitted here as they require complex setup. These will be covered by integration tests.
    // The basic functionality tests above verify the core pretty-printer logic.

    // ===== Type Tests =====

    #[test]
    fn test_print_type_simple() {
        let mut interner = StringInterner::new();
        let name = sym(&mut interner, "Int");
        let ty = Type::Simple {
            name,
            span: Span::new(0, 3, 1, 1, 1, 4),
        };

        let printer = PrettyPrinter::new(interner);
        assert_eq!(printer.print_type(&ty), "Int");
    }

    #[test]
    fn test_print_type_tuple() {
        let mut interner = StringInterner::new();
        let t1 = sym(&mut interner, "Int");
        let t2 = sym(&mut interner, "String");
        let ty = Type::Tuple {
            elements: vec![
                Type::Simple {
                    name: t1,
                    span: Span::new(0, 3, 1, 1, 1, 4),
                },
                Type::Simple {
                    name: t2,
                    span: Span::new(5, 11, 1, 6, 1, 12),
                },
            ],
            span: Span::new(0, 12, 1, 1, 1, 13),
        };

        let printer = PrettyPrinter::new(interner);
        assert_eq!(printer.print_type(&ty), "(Int, String)");
    }

    #[test]
    fn test_print_type_generic() {
        let mut interner = StringInterner::new();
        let name = sym(&mut interner, "Vec");
        let inner = sym(&mut interner, "T");
        let ty = Type::Generic {
            name,
            params: vec![Type::Simple {
                name: inner,
                span: Span::new(4, 5, 1, 5, 1, 6),
            }],
            span: Span::new(0, 6, 1, 1, 1, 7),
        };

        let printer = PrettyPrinter::new(interner);
        assert_eq!(printer.print_type(&ty), "Vec<T>");
    }

    #[test]
    fn test_print_type_function() {
        let mut interner = StringInterner::new();
        let p1 = sym(&mut interner, "Int");
        let p2 = sym(&mut interner, "Int");
        let ret = sym(&mut interner, "Int");
        let ty = Type::Function {
            params: vec![
                Type::Simple {
                    name: p1,
                    span: Span::new(3, 6, 1, 4, 1, 7),
                },
                Type::Simple {
                    name: p2,
                    span: Span::new(8, 11, 1, 9, 1, 12),
                },
            ],
            return_type: Box::new(Type::Simple {
                name: ret,
                span: Span::new(16, 19, 1, 17, 1, 20),
            }),
            span: Span::new(0, 19, 1, 1, 1, 20),
        };

        let printer = PrettyPrinter::new(interner);
        assert_eq!(printer.print_type(&ty), "fn(Int, Int) -> Int");
    }

    #[test]
    fn test_print_type_array() {
        let mut interner = StringInterner::new();
        let elem = sym(&mut interner, "Int");
        let size = sym(&mut interner, "10");
        let ty = Type::Array {
            element: Box::new(Type::Simple {
                name: elem,
                span: Span::new(1, 4, 1, 2, 1, 5),
            }),
            size: Some(size),
            span: Span::new(0, 8, 1, 1, 1, 9),
        };

        let printer = PrettyPrinter::new(interner);
        assert_eq!(printer.print_type(&ty), "[Int; 10]");
    }

    #[test]
    fn test_print_type_dict() {
        let mut interner = StringInterner::new();
        let key = sym(&mut interner, "String");
        let val = sym(&mut interner, "Int");
        let ty = Type::Dict {
            key: Box::new(Type::Simple {
                name: key,
                span: Span::new(1, 7, 1, 2, 1, 8),
            }),
            value: Box::new(Type::Simple {
                name: val,
                span: Span::new(9, 12, 1, 10, 1, 13),
            }),
            span: Span::new(0, 13, 1, 1, 1, 14),
        };

        let printer = PrettyPrinter::new(interner);
        assert_eq!(printer.print_type(&ty), "[String: Int]");
    }

    #[test]
    fn test_print_type_optional() {
        let mut interner = StringInterner::new();
        let inner = sym(&mut interner, "Int");
        let ty = Type::Optional {
            inner: Box::new(Type::Simple {
                name: inner,
                span: Span::new(0, 3, 1, 1, 1, 4),
            }),
            span: Span::new(0, 4, 1, 1, 1, 5),
        };

        let printer = PrettyPrinter::new(interner);
        assert_eq!(printer.print_type(&ty), "Int?");
    }

    #[test]
    fn test_print_type_inferred() {
        let ty = Type::Inferred {
            span: Span::new(0, 1, 1, 1, 1, 2),
        };

        let printer = printer();
        assert_eq!(printer.print_type(&ty), "_");
    }
}
