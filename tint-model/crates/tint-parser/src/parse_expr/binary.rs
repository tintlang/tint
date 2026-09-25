use crate::{Parser, error::*};
use tint_ast::{Expr, Span};
use tint_lexer::TokenKind;

impl Parser {
    pub(crate) fn parse_binary_expr(&mut self, min_prec: u8) -> PResult<Expr> {
        let mut left = self.parse_postfix()?;

        loop {
            let tok = self.stream.peek().clone();

            // --- STOP TOKENS ---
            match tok.kind {
                TokenKind::RParen |
                TokenKind::RBracket |
                TokenKind::RBrace |
                TokenKind::Comma => {
                    break;
                }
                _ => {}
            }

            let prec = tok.kind.binary_precedence();
            if prec == 0 || prec < min_prec { break; }

            let op = self.stream.next();
            let right = self.parse_binary_expr(prec + 1)?;
            let span = Span::merge(left.span(), right.span());

            left = Expr::Binary {
                left: Box::new(left),
                op: op.lexeme,
                right: Box::new(right),
                span,
            };
        }

        Ok(left)
    }
}
