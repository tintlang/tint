use crate::{error::*, Parser};
use tint_ast::{Expr, Span};
use tint_lexer::TokenKind;

impl Parser {
    pub(crate) fn parse_lambda(&mut self) -> PResult<Expr> {
        let opener = self.stream.next(); // '|' or '||'
        let start = opener.span;
        if opener.kind == TokenKind::OrOr {
            let body = self.parse_expr()?;
            let span = Span::merge(start, body.span());
            return Ok(Expr::Lambda { params: Vec::new(), param_types: Vec::new(), body: Box::new(body), span });
        }

        let mut params = Vec::new();
        let mut param_types = Vec::new();
        loop {
            let name = self.parse_ident()?;
            params.push(name);
            param_types.push(if self.stream.consume_if(TokenKind::Colon) { Some(self.parse_type()?) } else { None });

            if !self.stream.consume_if(TokenKind::Comma) {
                break;
            }
        }

        self.stream.expect(TokenKind::Pipe)?; // closing |

        let body = self.parse_expr()?;
        let span = Span::merge(start, body.span());

        if param_types.iter().all(Option::is_none) {
            param_types.clear();
        }
        Ok(Expr::Lambda {
            params,
            param_types,
            body: Box::new(body),
            span,
        })
    }
}
