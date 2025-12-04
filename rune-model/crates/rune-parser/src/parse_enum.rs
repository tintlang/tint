// rune-parser/parse_enum.rs
use crate::{Parser};
use crate::error::*;
use rune_ast::*;
use rune_lexer::TokenKind;

impl Parser {
    pub(crate) fn parse_enum(&mut self) -> PResult<EnumDecl> {
        let start = self.stream.expect(TokenKind::Enum)?.span;
        let name = self.parse_ident()?;

        self.stream.expect(TokenKind::LBrace)?;

        let mut variants = Vec::new();

        while !self.stream.consume_if(TokenKind::RBrace) {
            let variant_name = self.parse_ident()?;

            let variant = match self.stream.peek_kind() {
                TokenKind::LBrace => {
                    self.stream.next(); // consume '{'

                    let mut fields = Vec::new();
                    let mut style: Option<&'static str> = None;
                    // allowed: "typed", "rune"

                    if !self.stream.consume_if(TokenKind::RBrace) {
                        loop {
                            let field_name = self.parse_ident()?;

                            // ────────────────────────────────
                            // TYPED FIELD  (name : Type)
                            // ────────────────────────────────
                            if self.stream.consume_if(TokenKind::Colon) {
                                if let Some(s) = style {
                                    if s != "typed" {
                                        return self.stream.error_here::<_>(
                                            "Cannot mix typed-fields and rune-fields in one enum variant"
                                        );
                                    }
                                } else {
                                    style = Some("typed");
                                }

                                let ty = self.parse_type()?;

                                fields.push(StructField::Typed {
                                    name: field_name,
                                    ty,
                                    span: self.stream.last_span(),
                                });
                            }

                            // ────────────────────────────────
                            // RUNE FIELD  (name)
                            // ────────────────────────────────
                            else {
                                if let Some(s) = style {
                                    if s != "rune" {
                                        return self.stream.error_here::<_>(
                                            "Cannot mix typed-fields and rune-fields in one enum variant"
                                        );
                                    }
                                } else {
                                    style = Some("rune");
                                }

                                fields.push(StructField::RuneField {
                                    name: field_name,
                                    span: self.stream.last_span(),
                                });
                            }

                            // Close variant?
                            if self.stream.consume_if(TokenKind::RBrace) {
                                break;
                            }

                            self.stream.expect(TokenKind::Comma)?;
                        }
                    }

                    EnumVariant::Struct(variant_name, fields)
                }

                // Tuple variants → запрещены
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
            variants,
            span: Span::merge(start, end),
        })
    }
}
