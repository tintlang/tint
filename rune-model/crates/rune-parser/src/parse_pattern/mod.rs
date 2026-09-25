use crate::{error::*, Parser};
use rune_ast::*;
use rune_lexer::TokenKind;

mod collections;
mod fields;
mod named;

impl Parser {
    pub fn parse_pattern(&mut self) -> PResult<Pattern> {
        if !self.in_pattern {
            return Err(ParserError::Message {
                msg: "pattern parser invoked outside pattern context".into(),
                span: self.stream.peek().span,
            });
        }

        if self.stream.peek().lexeme == "mut" {
            let mut_span = self.stream.next().span;
            let inner = self.parse_pattern()?;
            let end = inner.span();

            return Ok(Pattern::Mut {
                inner: Box::new(inner),
                span: Span::merge(mut_span, end),
            });
        }

        if self.stream.peek_kind() == TokenKind::SelfKw {
            let tok = self.stream.next();
            return Ok(Pattern::Ident("self".into(), tok.span));
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

}
