use crate::{error::*, Parser};
use tint_ast::Expr;

impl Parser {
    pub(crate) fn parse_number(&mut self) -> PResult<Expr> {
        let t = self.stream.next();
        Ok(Expr::Number(t.lexeme, t.span))
    }
}
