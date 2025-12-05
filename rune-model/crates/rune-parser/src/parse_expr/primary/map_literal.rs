use crate::{Parser, error::*};
use rune_ast::{Expr, Span, StructInitField};
use rune_lexer::TokenKind;

impl Parser {
    pub(crate) fn parse_map_literal(&mut self) -> PResult<Expr> {
        // consume `map`
        let start = self.stream.next().span; 

        // parse `{ ... }` using existing struct field parser
        let entries = self.parse_map_entries()?;

        let end = self.stream.last_span();

        Ok(Expr::MapInit {
            entries,
            span: Span::merge(start, end),
        })
    }

    fn parse_map_entries(&mut self) -> PResult<Vec<(String, Expr)>> {
        self.stream.expect(TokenKind::LBrace)?;

        let mut entries = Vec::new();

        if self.stream.consume_if(TokenKind::RBrace) {
            return Ok(entries);
        }

        loop {
            // parse key
            let key = self.parse_ident()?; 
            let start = self.stream.last_span();

            // Rune-style: key{expr}
            if self.stream.consume_if(TokenKind::LBrace) {
                let expr = self.parse_expr()?;
                self.stream.expect(TokenKind::RBrace)?;
                entries.push((key, expr));
            }
            // Rust-style: key: expr
            else if self.stream.consume_if(TokenKind::Colon) {
                let expr = self.parse_expr()?;
                entries.push((key, expr));
            }
            else {
                return Err(ParserError::Message {
                    msg: "Expected `{` or `:` in map entry".into(),
                    span: start,
                });
            }

            if !self.stream.consume_if(TokenKind::Comma) {
                break;
            }
        }

        self.stream.expect(TokenKind::RBrace)?;
        Ok(entries)
    }
}
