use crate::{error::*, Parser};
use tint_ast::{Expr, Span};
use tint_lexer::TokenKind;

mod gpu;
mod suffix;

impl Parser {
    pub(crate) fn parse_postfix(&mut self) -> PResult<Expr> {
        let primary = self.parse_primary()?;
        self.parse_postfix_with(primary)
    }

    pub(crate) fn parse_postfix_with(&mut self, mut expr: Expr) -> PResult<Expr> {
        if self.in_pattern {
            return Ok(expr); // Postfix operators are not valid while parsing patterns.
        }

        loop {
            match self.stream.peek_kind() {
                TokenKind::Comma
                | TokenKind::RParen
                | TokenKind::RBracket
                | TokenKind::RBrace
                | TokenKind::Semicolon => {
                    return Ok(expr);
                }
                _ => {}
            }

            if let Some(new_expr) = self.try_parse_gpu(&expr)? {
                expr = new_expr;
                continue;
            }

            match self.stream.peek_kind() {
                TokenKind::LParen => {
                    expr = self.parse_call(expr)?;
                    continue;
                }

                TokenKind::Dot => {
                    expr = self.parse_field_or_tuple(expr)?;
                    continue;
                }

                TokenKind::PathSep => {
                    expr = self.parse_namespace(expr)?;
                    continue;
                }

                TokenKind::LBracket => {
                    expr = self.parse_index(expr)?;
                    continue;
                }

                TokenKind::LBrace => {
                    // A brace is a named call only for known function identifiers.
                    if let Expr::Ident(ref fname, _) = expr {
                        if self.symbols.is_function(fname) {
                            expr = self.parse_named_call(expr)?;
                            continue;
                        }
                    }

                    // Otherwise the brace belongs to struct initialization, not a postfix call.
                    break;
                }
                _ => break,
            }
        }
        Ok(expr)
    }
}
