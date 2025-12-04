use crate::{Parser, error::*};
use rune_ast::{Expr, Span};
use rune_lexer::TokenKind;

impl Parser {
    pub(crate) fn parse_paren_or_tuple(&mut self) -> PResult<Expr> {
        let open = self.stream.next().span; // '('

        if self.stream.consume_if(TokenKind::RParen) {
            return Ok(Expr::Unit(open));
        }

        let first = self.parse_expr()?;

        if self.stream.consume_if(TokenKind::Comma) {
            // tuple
            let mut items = vec![first];

            loop {
                items.push(self.parse_expr()?);

                if !self.stream.consume_if(TokenKind::Comma) {
                    break;
                }
            }

            self.stream.expect(TokenKind::RParen)?;
            let end = self.stream.last_span();

            return Ok(Expr::Tuple {
                items,
                span: Span::merge(open, end),
            });
        }

        self.stream.expect(TokenKind::RParen)?;
        Ok(Expr::Paren(Box::new(first), open))
    }
}
