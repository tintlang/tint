use super::*;

impl Parser {
    pub(crate) fn parse_struct_field_pattern(&mut self, field: String, start: Span) -> PResult<PatternField> {
        self.stream.expect(TokenKind::LBrace)?;

        let mut fields = Vec::new();

        // Empty braces represent an empty nested group pattern.
        if self.stream.consume_if(TokenKind::RBrace) {
            return Ok(PatternField::Assign {
                field,
                pat: Pattern::Group { fields, span: start },
                span: start,
            });
        }

        loop {
            let name = self.parse_ident()?; // Nested group fields are identifiers.
            let name_span = self.stream.last_span();

            fields.push(PatternField::Shorthand {
                field: name,
                span: name_span,
            });

            if !self.stream.consume_if(TokenKind::Comma) {
                break;
            }
        }

        let end = self.stream.expect(TokenKind::RBrace)?.span;

        Ok(PatternField::Assign {
            field,
            pat: Pattern::Group {
                fields,
                span: Span::merge(start, end),
            },
            span: Span::merge(start, end),
        })
    }

    pub(crate) fn parse_pattern_field(&mut self) -> PResult<PatternField> {
        if self.stream.peek_kind() == TokenKind::RBrace {
            return Err(ParserError::Message {
                msg: "Unexpected end of struct pattern".into(),
                span: self.stream.peek().span,
            });
        }

        let field = self.parse_ident()?;
        let start = self.stream.last_span();

        match self.stream.peek_kind() {
            TokenKind::LBrace => {
                return self.parse_struct_field_pattern(field, start);
            }

            TokenKind::Colon => {
                self.stream.next();
                let pat = self.parse_pattern()?;
                let end = pat.span();

                Ok(PatternField::Assign {
                    field,
                    pat,
                    span: Span::merge(start, end),
                })
            }

            _ => Ok(PatternField::Shorthand {
                field,
                span: start,
            }),
        }
}
}
