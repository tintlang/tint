use super::*;

impl Parser {
    pub(crate) fn parse_lhs_for_assignment(&mut self) -> PResult<Expr> {
        let start = self.stream.peek().span;

        let mut expr = if let TokenKind::Ident = self.stream.peek().kind {
            let name = self.stream.next().lexeme.clone();
            Expr::Ident(name, start)
        } else {
            return Err(ParserError::Message {
                msg: "Expected identifier for assignment".into(),
                span: start,
            });
        };

        loop {
            if self.stream.consume_if(TokenKind::Dot) {
                let field = self.parse_ident()?;
                let span = Span::merge(start, self.stream.last_span());
                expr = Expr::Field {
                    target: Box::new(expr),
                    field,
                    span,
                };
                continue;
            }

            if self.stream.consume_if(TokenKind::LBracket) {
                let idx = self.parse_expr()?;
                self.stream.expect(TokenKind::RBracket)?;
                let span = Span::merge(start, idx.span());
                expr = Expr::Index {
                    target: Box::new(expr),
                    index: Box::new(idx),
                    span,
                };
                continue;
            }

            break;
        }

        Ok(expr)
    }
}
