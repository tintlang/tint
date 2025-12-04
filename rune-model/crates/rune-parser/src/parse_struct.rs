// rune-parser/parse_struct.rs

use crate::{Parser, error::*};
use rune_ast::{StructDecl, StructField, Span};
use rune_lexer::TokenKind;

impl Parser {
    /// Parse a struct declaration:
    ///   struct User {
    ///       id: i32,
    ///       name{String},
    ///   }
    pub(crate) fn parse_struct(&mut self) -> PResult<StructDecl> {
        // `struct`
        let start = self.stream.expect(TokenKind::Struct)?.span;

        // name
        let name = self.parse_ident()?;

        // optional <T, U>
        let _generics = self.parse_optional_generics()?;

        // `{`
        self.stream.expect(TokenKind::LBrace)?;

        let mut fields = Vec::new();
        let mut style: Option<&'static str> = None; // "typed" or "rune"

        // fields loop
        while !self.stream.consume_if(TokenKind::RBrace) {
            let field = self.parse_struct_field()?;

            // Determine this field's style
            let this_style = match &field {
                StructField::Typed { .. }     => "typed",
                StructField::RuneTyped { .. } => "runetyped",
                StructField::RuneField { .. } => "runefield",
            };

            // First field → set struct style
            if style.is_none() {
                style = Some(this_style);
            } else if style.unwrap() != this_style {
                return Err(ParserError::Message {
                    msg: format!(
                        "Mixed struct field styles are not allowed. \
                         Expected all `{}`, but found `{}`.",
                        style.unwrap(),
                        this_style
                    ),
                    span: field.span(),
                });
            }

            fields.push(field);

            // optional comma
            self.stream.consume_if(TokenKind::Comma);
        }

        let end = self.stream.last_span();

        Ok(StructDecl {
            name,
            fields,
            span: Span::merge(start, end),
        })
    }

    /// Parse a single struct field inside `struct { … }`
    /// Supports:
    ///   name: Type
    ///   name{Type}
    fn parse_struct_field(&mut self) -> PResult<StructField> {
        let name = self.parse_ident()?;
        let start = self.stream.last_span();

        // RUST-style field: name: Type
        if self.stream.consume_if(TokenKind::Colon) {
            let ty = self.parse_type()?;
            let end = self.stream.last_span();
            return Ok(StructField::Typed {
                name,
                ty,
                span: Span::merge(start, end),
            });
        }

        // RUNE-style field: name{Type}
        if self.stream.consume_if(TokenKind::LBrace) {
            let ty = self.parse_type()?;
            self.stream.expect(TokenKind::RBrace)?;
            let end = self.stream.last_span();
            return Ok(StructField::RuneTyped {
                name,
                ty,
                span: Span::merge(start, end),
            });
        }

        Err(ParserError::Message {
            msg: "Expected ':' or '{' after struct field name".into(),
            span: start,
        })
    }

    /// Parse optional generics: <T, U>
    fn parse_optional_generics(&mut self) -> PResult<Vec<String>> {
        let mut out = Vec::new();

        // no `<`
        if !self.stream.consume_if(TokenKind::LAngle) {
            return Ok(out);
        }

        // parse identifiers separated by commas until `>`
        loop {
            out.push(self.parse_ident()?);

            if self.stream.consume_if(TokenKind::RAngle) {
                break;
            }

            self.stream.expect(TokenKind::Comma)?;
        }

        Ok(out)
    }
}
