use rune_lexer::TokenKind;
use rune_ast::{Expr, Span};
use crate::{Parser, error::*};

impl Parser {
    /// Entry: parse full expression
    pub fn parse_expr(&mut self) -> PResult<Expr> {
        self.parse_binary_expr(0)
    }

    /// Parse literals, identifiers, parenthesis
    pub fn parse_primary(&mut self) -> PResult<Expr> {
        // ВАЖНО: забираем данные и отпускаем borrow
        let tok = self.stream.next().clone();
        let span = tok.span;
        let lex = tok.lexeme.clone();

        match tok.kind {
            TokenKind::Number => Ok(Expr::Number(lex, span)),
            TokenKind::String => Ok(Expr::String(lex, span)),
            TokenKind::Ident  => Ok(Expr::Ident(lex, span)),

            TokenKind::LParen => {
                let expr = self.parse_expr()?;
                self.stream.expect(TokenKind::RParen)?;
                Ok(Expr::Paren(Box::new(expr), span))
            }

            _ => Err(ParserError::Message {
                msg: "Expected expression".into(),
                span,
            }),
        }
    }

    /// Pratt parser
    pub fn parse_binary_expr(&mut self, min_prec: u8) -> PResult<Expr> {
        let mut left = self.parse_primary()?;

        loop {
            let op = self.stream.peek();

            let prec = op.kind.binary_precedence();
            if prec < min_prec {
                break;
            }

            // Read operator
            let op_tok = self.stream.next();

            // Right-hand side
            let right = self.parse_binary_expr(prec + 1)?;

            let span = Span::merge(left.span(), right.span());

            left = Expr::Binary {
                left: Box::new(left),
                op: op_tok.lexeme.clone(),
                right: Box::new(right),
                span,
            };
        }

        Ok(left)
    }
}
