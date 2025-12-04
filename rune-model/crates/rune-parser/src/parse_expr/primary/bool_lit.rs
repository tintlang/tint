use crate::{Parser, error::*};
use rune_ast::Expr;
use rune_lexer::TokenKind;

impl Parser {
    pub(crate) fn parse_bool(&mut self) -> PResult<Expr> {
        let t = self.stream.next();

        match t.kind {
            TokenKind::True  => Ok(Expr::Bool(true,  t.span)),
            TokenKind::False => Ok(Expr::Bool(false, t.span)),
            _ => unreachable!(),
        }
    }
}
