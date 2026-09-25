use super::*;

impl Parser {
    pub(crate) fn try_parse_gpu(&mut self, base: &Expr) -> PResult<Option<Expr>> {
        // `.gpu(...)` is speculative because a regular field access also starts with `.`.
        let checkpoint = self.stream.checkpoint();

        if !self.stream.consume_if(TokenKind::Dot) {
            return Ok(None);
        }

        if self.stream.peek().lexeme != "gpu" {
            self.stream.restore(checkpoint);
            return Ok(None);
        }

        let gpu_tok = self.stream.next();
        let gpu_span = gpu_tok.span;

        if !self.stream.consume_if(TokenKind::LParen) {
            self.stream.restore(checkpoint);
            return Ok(None);
        }

        let mut args = vec![base.clone()];

        if !self.stream.consume_if(TokenKind::RParen) {
            loop {
                args.push(self.parse_expr()?);

                if !self.stream.consume_if(TokenKind::Comma) {
                    break;
                }
            }
            self.stream.expect(TokenKind::RParen)?;
        }

        let span = Span::merge(base.span(), self.stream.last_span());

        Ok(Some(Expr::Call {
            target: Box::new(Expr::Ident("gpu$call".into(), gpu_span)),
            args,
            span,
        }))
    }

    // Repeatedly applies postfix operators until the next token belongs to the outer grammar.
}
