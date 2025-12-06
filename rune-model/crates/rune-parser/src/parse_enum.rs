// rune-parser/parse_enum.rs
use crate::{Parser};
use crate::error::*;
use rune_ast::*;
use rune_lexer::TokenKind;

impl Parser {
    pub(crate) fn parse_enum(&mut self) -> PResult<EnumDecl> {
        let start = self.stream.expect(TokenKind::Enum)?.span;
        let name = self.parse_ident()?;

        // GENERIC PARAMETERS: <T, E>
        let mut generics = Vec::new();

        if self.stream.consume_if(TokenKind::LAngle) {
            loop {
                let g = self.parse_ident()?;
                generics.push(g);

                if self.stream.consume_if(TokenKind::Comma) {
                    continue;
                }

                self.stream.expect(TokenKind::RAngle)?;
                break;
            }
        }

        self.stream.expect(TokenKind::LBrace)?;

        let mut variants = Vec::new();

        while !self.stream.consume_if(TokenKind::RBrace) {
            let variant_name = self.parse_ident()?;

            let variant = match self.stream.peek_kind() {
    TokenKind::LBrace => {
        self.stream.next(); // consume '{'

        let mut fields = Vec::new();

        #[derive(PartialEq)]
        enum Style { Rune, Typed }
        let mut style: Option<Style> = None;

        // empty variant {}
        if self.stream.consume_if(TokenKind::RBrace) {
            variants.push(EnumVariant::Struct(variant_name, fields));
            continue; // идём парсить следующий variant
        }

        loop {
            let field_name = self.parse_ident()?;
            let field_span = self.stream.last_span();

            if self.stream.consume_if(TokenKind::Colon) {
                // --- Typed style ---
                match style {
                    None => style = Some(Style::Typed),
                    Some(Style::Typed) => {}
                    Some(Style::Rune) => {
                        return self.stream.error_here::<_>(
                            "Cannot mix Rune `{field}` and typed `field: T` enum fields"
                        );
                    }
                }

                let ty = self.parse_type()?;

                fields.push(StructField::Typed {
                    name: field_name,
                    ty,
                    span: field_span,
                });
            }
            else {
                // --- Rune style ---
                match style {
                    None => style = Some(Style::Rune),
                    Some(Style::Rune) => {}
                    Some(Style::Typed) => {
                        return self.stream.error_here::<_>(
                            "Cannot mix typed `field: T` and Rune `field` enum fields"
                        );
                    }
                }

                fields.push(StructField::RuneField {
                    name: field_name,
                    span: field_span,
                });
            }

            if self.stream.consume_if(TokenKind::RBrace) {
                break;
            }

            self.stream.expect(TokenKind::Comma)?;
        }

        EnumVariant::Struct(variant_name, fields)
    }

                // Tuple variants -> запрещены
                TokenKind::LParen => {
                    return self.stream.error_here(
                        "Tuple variants like Foo(T) are not allowed in Rune"
                    );
                }

                // Unit variant
                _ => EnumVariant::Unit(variant_name),
            };

            variants.push(variant);
            self.stream.consume_if(TokenKind::Comma);
        }

        let end = self.stream.last_span();

        Ok(EnumDecl {
            name,
            generics,
            variants,
            exported: false,
            span: Span::merge(start, end),
        })
    }
}
