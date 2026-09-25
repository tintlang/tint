use super::*;

impl Parser {
    pub(crate) fn parse_tuple_pattern(&mut self) -> PResult<Pattern> {
        let start_tok = self.stream.expect(TokenKind::LParen)?;
        let start = start_tok.span;

        let mut items = Vec::new();

        if self.stream.consume_if(TokenKind::RParen) {
            return Ok(Pattern::Tuple(items, start));
        }

        loop {
            items.push(self.parse_pattern()?);

            if self.stream.consume_if(TokenKind::RParen) {
                break;
            }

            self.stream.expect(TokenKind::Comma)?;
        }

        let end = self.stream.last_span();
        Ok(Pattern::Tuple(items, Span::merge(start, end)))
    }

    pub(crate) fn parse_map_pattern(&mut self) -> PResult<Pattern> {
        let start = self.stream.expect(TokenKind::MapLit)?.span;
        self.stream.expect(TokenKind::LBrace)?;

        let mut fields = Vec::new();

        if self.stream.consume_if(TokenKind::RBrace) {
            return Ok(Pattern::Map { fields, span: start });
        }

        loop {
            let field = self.parse_ident()?;
            let field_span = self.stream.last_span();

            match self.stream.peek_kind() {

                TokenKind::Comma | TokenKind::RBrace => {
                    fields.push(PatternField::Shorthand { field, span: field_span });
                }

                TokenKind::Colon => {
                    self.stream.next();
                    let pat = self.parse_pattern()?;
                    let end = pat.span();

                    fields.push(PatternField::Assign {
                        field,
                        pat,
                        span: Span::merge(field_span, end),
                    });
                }

                TokenKind::LBrace => {
                    self.stream.next();
                    let pat = self.parse_pattern()?;
                    let end = self.stream.expect(TokenKind::RBrace)?.span;

                    fields.push(PatternField::Assign {
                        field,
                        pat,
                        span: Span::merge(field_span, end),
                    });
                }

                _ => {
                    return Err(ParserError::Message {
                        msg: "Invalid field in map-pattern".into(),
                        span: self.stream.peek().span,
                    });
                }
            }

            if !self.stream.consume_if(TokenKind::Comma) {
                break;
            }
        }

        self.stream.expect(TokenKind::RBrace)?;

        Ok(Pattern::Map {
            fields,
            span: Span::merge(start, self.stream.last_span()),
        })
    }

}
