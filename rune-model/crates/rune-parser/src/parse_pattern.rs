// rune-parser/parse_pattern.rs
//  PATTERN PARSER

use crate::{Parser};
use crate::error::*;
use rune_ast::*;
use rune_lexer::TokenKind;

impl Parser {

    pub fn parse_pattern(&mut self) -> PResult<Pattern> {
        if !self.in_pattern {
            return Err(ParserError::Message {
                msg: "pattern parser invoked outside pattern context".into(),
                span: self.stream.peek().span,
            });
        }

        //  MUT PATTERN: mut <pat>
        if self.stream.peek().lexeme == "mut" {
            let mut_span = self.stream.next().span;
            let inner = self.parse_pattern()?;
            let end = inner.span();

            return Ok(Pattern::Mut {
                inner: Box::new(inner),
                span: Span::merge(mut_span, end),
            });
        }

        let tok = self.stream.peek();

        match tok.kind {
            TokenKind::Underscore => {
                let span = self.stream.next().span;
                Ok(Pattern::Wildcard(span))
            }

            TokenKind::Ident => self.parse_ident_based_pattern(),

            TokenKind::LParen => self.parse_tuple_pattern(),

            TokenKind::Number => {
                let t = self.stream.next();
                Ok(Pattern::Number(t.lexeme, t.span))
            }

            TokenKind::String => {
                let t = self.stream.next();
                Ok(Pattern::String(t.lexeme, t.span))
            }

            TokenKind::MapLit => self.parse_map_pattern(),

            TokenKind::LBrace => self.parse_pattern_group(),

            _ => Err(ParserError::Message {
                msg: "Unexpected token in pattern".into(),
                span: tok.span,
            })
        }
    }


    //  (a, b, c) COMMA REQUIRED
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

    //  map { x{p}, y{p} } 
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


    //  IDENT-BASED PATTERNS
    fn parse_ident_based_pattern(&mut self) -> PResult<Pattern> {
        let tok = self.stream.next_owned();
        let name = tok.lexeme;
        let span = tok.span;

        // typed pattern: x: i32
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

    //  Variant(x, y, z)
    fn parse_variant_pattern(&mut self, name: String, start: Span) -> PResult<Pattern> {
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

    //  User { a, b, c }
    fn parse_struct_pattern(&mut self, name: String, start: Span) -> PResult<Pattern> {
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


    //  { a, b, c } GROUP PATTERN
    fn parse_pattern_group(&mut self) -> PResult<Pattern> {
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


    fn parse_struct_field_pattern(&mut self, field: String, start: Span) -> PResult<PatternField> {
        self.stream.expect(TokenKind::LBrace)?;

        let mut fields = Vec::new();

        // empty: field{}
        if self.stream.consume_if(TokenKind::RBrace) {
            return Ok(PatternField::Assign {
                field,
                pat: Pattern::Group { fields, span: start },
                span: start,
            });
        }

        loop {
            let name = self.parse_ident()?;     // <<── always ident
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

    //  Struct-field pattern item
    fn parse_pattern_field(&mut self) -> PResult<PatternField> {
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
                // always struct-style pattern after identifier!
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
