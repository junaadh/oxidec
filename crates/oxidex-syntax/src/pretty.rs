//! Pretty-printer for `OxideX` AST.
//!
//! This module converts AST nodes back to source code, enabling:
//! - Debugging and validation
//! - Code formatting
//! - Round-trip testing (parse → print → parse)
//! - AST inspection

use crate::ast::{Expr, Stmt, Type};
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

            // TODO: Implement other expression variants
            _ => "<unimplemented>".to_string(),
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

            // TODO: Implement other statement variants
            _ => "<unimplemented>".to_string(),
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

            // TODO: Implement other type variants
            _ => "<unimplemented>".to_string(),
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
}
