use crate::{Parser, error::*};
use rune_ast::Expr;
use rune_lexer::TokenKind;

impl Parser {
    pub(crate) fn parse_primary_strict(&mut self) -> PResult<Expr> {
        match self.stream.peek_kind() {
            TokenKind::Ident => {
                let t = self.stream.next();
                Ok(Expr::Ident(t.lexeme, t.span))
            }
            TokenKind::Number => {
                let t = self.stream.next();
                Ok(Expr::Number(t.lexeme, t.span))
            }
            TokenKind::String => {
                let t = self.stream.next();
                Ok(Expr::String(t.lexeme, t.span))
            }
            TokenKind::True => {
                let t = self.stream.next();
                Ok(Expr::Bool(true, t.span))
            }
            TokenKind::False => {
                let t = self.stream.next();
                Ok(Expr::Bool(false, t.span))
            }

            TokenKind::LParen => Err(ParserError::Message {
                msg: "Parenthesized expression not allowed in match scrutinee".into(),
                span: self.stream.peek().span,
            }),

            _ => Err(ParserError::Message {
                msg: format!("Unexpected token in match scrutinee"),
                span: self.stream.peek().span,
            }),
        }
    }
}
