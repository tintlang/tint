// rune-parser/parse_pattern.rs

use crate::{Parser};
use crate::error::*;
use rune_ast::*;
use rune_lexer::TokenKind;

impl Parser {

pub fn parse_pattern(&mut self) -> PResult<Pattern> {
    let tok = self.stream.peek();

    match tok.kind {

        TokenKind::Underscore => {
            let span = self.stream.next().span;
            return Ok(Pattern::Wildcard(span));
        }

        TokenKind::Ident => {
            return self.parse_ident_based_pattern();
        }

        TokenKind::LParen => {
            return self.parse_tuple_pattern();
        }

        TokenKind::Number => {
            let t = self.stream.next();
            return Ok(Pattern::Number(t.lexeme, t.span));
        }

        TokenKind::String => {
            let t = self.stream.next();
            return Ok(Pattern::String(t.lexeme, t.span));
        }

        _ => Err(ParserError::Message {
            msg: "Unexpected token in pattern".into(),
            span: tok.span,
        })
    }
}

    // -----------------------------------------
    // (a, b, c)
    // -----------------------------------------
    pub(crate) fn parse_tuple_pattern(&mut self) -> PResult<Pattern> {
        let start_tok = self.stream.expect(TokenKind::LParen)?;
        let start = start_tok.span;

        let mut items = Vec::new();

        // ()
        if self.stream.consume_if(TokenKind::RParen) {
            return Ok(Pattern::Tuple(items, start));
        }

        // (x, y, ...)
        loop {
            let pat = self.parse_pattern()?;
            items.push(pat);

            if !self.stream.consume_if(TokenKind::Comma) {
                break;
            }
        }

        let end = self.stream.expect(TokenKind::RParen)?.span;

        Ok(Pattern::Tuple(items, Span::merge(start, end)))
    }


pub fn parse_let_pattern(&mut self) -> PResult<Pattern> {
    match self.stream.peek_kind() {

        TokenKind::Ident => {
            // SPAN копируем до parse_ident()
            let span = self.stream.peek().span;
            let id = self.parse_ident()?;
            Ok(Pattern::Ident(id, span))
        }

        TokenKind::LParen => {
            // tuple pattern
            self.parse_tuple_pattern()
        }

        _ => Err(ParserError::Message {
            msg: "Invalid pattern in let (only `name` or `(x,y)` allowed)".into(),
            span: self.stream.peek().span,
        })
    }
}
    // -----------------------------------------
    // x
    // Some(x)
    // Error(msg, code)
    // User { id, name }
    // -----------------------------------------
    fn parse_ident_based_pattern(&mut self) -> PResult<Pattern> {
        let tok = self.stream.next_owned(); 
        let name = tok.lexeme;
        let start = tok.span;

        match self.stream.peek_kind() {
            // Variant: Some(x)
            TokenKind::LParen => {
                self.stream.next(); // '('
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

            // Struct pattern: User { id, name, .. }
            TokenKind::LBrace => self.parse_struct_pattern(name, start),

            // Simple identifier binding: x
            _ => Ok(Pattern::Ident(name, start)),
        }
    }

    // -----------------------------------------
    // User { field, field: pat, .. }
    // -----------------------------------------
    fn parse_struct_pattern(&mut self, name: String, start: Span) -> PResult<Pattern> {
        self.stream.expect(TokenKind::LBrace)?;

        let mut fields = Vec::new();

        // User {}
        if self.stream.consume_if(TokenKind::RBrace) {
            return Ok(Pattern::Struct { name, fields, span: start });
        }

        loop {
            if self.stream.consume_if(TokenKind::DotDot) {
                let s = self.stream.last_span();
                fields.push(PatternField::Rest(s));
            } else {
                fields.push(self.parse_pattern_field()?);
            }

            if !self.stream.consume_if(TokenKind::Comma) {
                break;
            }
            if self.stream.peek_kind() == TokenKind::RBrace {
                break;
            }
        }

        self.stream.expect(TokenKind::RBrace)?;

        Ok(Pattern::Struct {
            name,
            fields,
            span: start,
        })
    }

    // -----------------------------------------
    // field
    // field: pat
    // -----------------------------------------
    fn parse_pattern_field(&mut self) -> PResult<PatternField> {
        let field = self.parse_ident()?;
        let start = self.stream.last_span();

        if self.stream.consume_if(TokenKind::Colon) {
            let pat = self.parse_pattern()?;
            let end = pat.span();
            return Ok(PatternField::Assign {
                field,
                pat,
                span: Span::merge(start, end),
            });
        }

        Ok(PatternField::Shorthand {
            field,
            span: start,
        })
    }
}
