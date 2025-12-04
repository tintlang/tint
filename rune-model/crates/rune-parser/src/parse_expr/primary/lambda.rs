use crate::{Parser, error::*};
use rune_ast::{Expr, Span};
use rune_lexer::TokenKind;

impl Parser {
    pub(crate) fn parse_lambda(&mut self) -> PResult<Expr> {
        let start = self.stream.next().span; // '|'

        let mut params = Vec::new();
        loop {
            let name = self.parse_ident()?;
            params.push(name);

            if !self.stream.consume_if(TokenKind::Comma) {
                break;
            }
        }

        self.stream.expect(TokenKind::PipeLambda)?; // closing |

        let body = self.parse_expr()?;
        let span = Span::merge(start, body.span());

        Ok(Expr::Lambda {
            params,
            body: Box::new(body),
            span,
        })
    }
}
