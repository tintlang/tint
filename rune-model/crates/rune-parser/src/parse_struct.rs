// rune-parser/parse_struct.rs

use crate::{Parser};
use crate::{ error::* };
use rune_ast::*;
use rune_lexer::TokenKind;

impl Parser {
    pub(crate) fn parse_struct(&mut self) -> PResult<StructDecl> {
        // struct keyword
        let start = self.stream.expect(TokenKind::Struct)?.span;

        // struct name
        let name = self.parse_ident()?;

        // optional generics: <T, U>
        let _generics = self.parse_optional_generics()?;  

        // expect { 
        self.stream.expect(TokenKind::LBrace)?;

        let mut fields = Vec::new();

        // parse zero or more fields:  name: Type,
        while !self.stream.consume_if(TokenKind::RBrace) {
            let field_start = self.stream.peek().span;

            let field_name = self.parse_ident()?;
            self.stream.expect(TokenKind::Colon)?;
            let ty = self.parse_type()?;

            let field_end = self.stream.last_span();

            // optional comma
            self.stream.consume_if(TokenKind::Comma);

            fields.push(StructField {
                name: field_name,
                ty,
                span: Span::merge(field_start, field_end),
            });
        }

        let end = self.stream.last_span();

        Ok(StructDecl {
            name,
            fields,
            span: Span::merge(start, end),
        })
    }

    // GENERICS: <T, U>
    fn parse_optional_generics(&mut self) -> PResult<Vec<String>> {
        let mut out = Vec::new();

        if !self.stream.consume_if(TokenKind::LAngle) {
            return Ok(out);
        }

        loop {
            let ident = self.parse_ident()?;
            out.push(ident);

            if self.stream.consume_if(TokenKind::RAngle) {
                break;
            }

            self.stream.expect(TokenKind::Comma)?;
        }

        Ok(out)
    }
}
