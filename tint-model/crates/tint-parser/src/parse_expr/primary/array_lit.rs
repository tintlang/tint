use crate::{Parser, error::*};
use tint_ast::{Expr, Span};
use tint_lexer::TokenKind;

impl Parser {
    pub(crate) fn parse_array_literal(&mut self) -> PResult<Expr> {
        let start = self.stream.next().span; // '['

        if self.stream.consume_if(TokenKind::RBracket) {
            return Ok(Expr::Array { items: vec![], span: start });
        }

        let mut items = Vec::new();

        loop {
            items.push(self.parse_expr()?);

            if !self.stream.consume_if(TokenKind::Comma) {
                break;
            }

            if self.stream.peek_kind() == TokenKind::RBracket {
                break;
            }
        }

        self.stream.expect(TokenKind::RBracket)?;
        let end = self.stream.last_span();

        Ok(Expr::Array { items, span: Span::merge(start, end) })
    }
}
