use super::*;

impl Parser {
    pub(crate) fn parse_ident_based_pattern(&mut self) -> PResult<Pattern> {
        let tok = self.stream.next_owned();
        let name = tok.lexeme;
        let span = tok.span;

        if self.stream.peek_kind() == TokenKind::Colon {
            self.stream.next();
            let ty = self.parse_type()?;
            return Ok(Pattern::Typed {
                pat: Box::new(Pattern::Ident(name.clone(), span)),
                ty,
                span,
            });
        }

        let next = self.stream.peek_kind();
        let is_upper = name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false);

        match next {
            TokenKind::LBrace if is_upper => self.parse_struct_pattern(name, span),
            TokenKind::LParen if is_upper => self.parse_variant_pattern(name, span),
            _ => Ok(Pattern::Ident(name, span)),
        }
    }

    pub(crate) fn parse_variant_pattern(&mut self, name: String, start: Span) -> PResult<Pattern> {
        self.stream.expect(TokenKind::LParen)?;

        let mut args = Vec::new();

        if !self.stream.consume_if(TokenKind::RParen) {
            loop {
                args.push(self.parse_pattern()?);
                if !self.stream.consume_if(TokenKind::Comma) {
                    break;
                }
            }
            self.stream.expect(TokenKind::RParen)?;
        }

        Ok(Pattern::Variant {
            name,
            args,
            span: start,
        })
    }

    pub(crate) fn parse_struct_pattern(&mut self, name: String, start: Span) -> PResult<Pattern> {
        self.stream.expect(TokenKind::LBrace)?;

        let mut fields = Vec::new();

        if self.stream.consume_if(TokenKind::RBrace) {
            return Ok(Pattern::Struct { name, fields, span: start });
        }

        loop {
            if self.stream.consume_if(TokenKind::DotDot) {
                fields.push(PatternField::Rest(self.stream.last_span()));
            } else {
                fields.push(self.parse_pattern_field()?);
            }

            if !self.stream.consume_if(TokenKind::Comma) {
                break;
            }
        }

        self.stream.expect(TokenKind::RBrace)?;

        Ok(Pattern::Struct { name, fields, span: start })
    }


    pub(crate) fn parse_pattern_group(&mut self) -> PResult<Pattern> {
        let start = self.stream.last_span();
        let mut fields = Vec::new();

        if self.stream.peek_kind() == TokenKind::RBrace {
            return Ok(Pattern::Group { fields, span: start });
        }

        loop {
            let field = self.parse_ident()?;
            let field_span = self.stream.last_span();

            fields.push(PatternField::Shorthand {
                field,
                span: field_span,
            });

            if !self.stream.consume_if(TokenKind::Comma) {
                break;
            }
        }

        let end = self.stream.expect(TokenKind::RBrace)?.span;

        Ok(Pattern::Group {
            fields,
            span: Span::merge(start, end),
        })
    }
}
