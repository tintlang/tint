use crate::{Parser, error::*};
use tint_ast::{Expr, Span};

impl Parser {
    pub(crate) fn parse_unary(&mut self) -> PResult<Expr> {
        let op = self.stream.next();
        let right = self.parse_primary()?;
        let span = Span::merge(op.span, right.span());

        Ok(Expr::Unary {
            op: op.lexeme,
            expr: Box::new(right),
            span,
        })
    }
}
