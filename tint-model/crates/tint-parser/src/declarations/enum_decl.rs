use crate::error::*;
use crate::Parser;
use tint_ast::*;
use tint_lexer::TokenKind;

impl Parser {
    pub(crate) fn parse_enum(&mut self) -> PResult<EnumDecl> {
        let start = self.stream.expect(TokenKind::Enum)?.span;
        let name = self.parse_ident()?;

        // GENERIC PARAMETERS: <T, E>
        let generics = self.parse_optional_generics()?;

        self.stream.expect(TokenKind::LBrace)?;

        let mut variants = Vec::new();

        while !self.stream.consume_if(TokenKind::RBrace) {
            let variant_name = self.parse_ident()?;

            let variant = match self.stream.peek_kind() {
                TokenKind::LBrace => {
                    self.stream.next(); // consume '{'

                    let mut fields = Vec::new();

                    #[derive(PartialEq)]
                    enum Style {
                        Tint,
                        Typed,
                    }
                    let mut style: Option<Style> = None;

                    // empty variant {}
                    if self.stream.consume_if(TokenKind::RBrace) {
                        variants.push(EnumVariant::Struct(variant_name, fields));
                        continue;
                    }
                    loop {
                        let field_name = self.parse_ident()?;
                        let field_span = self.stream.last_span();

                        if self.stream.consume_if(TokenKind::Colon) {
                            // --- Typed style ---
                            match style {
                                None => style = Some(Style::Typed),
                                Some(Style::Typed) => {}
                                Some(Style::Tint) => {
                                    return self.stream.error_here::<_>(
                            "Cannot mix Tint `{field}` and typed `field: T` enum fields"
                        );
                                }
                            }

                            let ty = self.parse_type()?;

                            fields.push(StructField::Typed {
                                name: field_name,
                                ty,
                                span: field_span,
                            });
                        } else {
                            // --- Tint style ---
                            match style {
                                None => style = Some(Style::Tint),
                                Some(Style::Tint) => {}
                                Some(Style::Typed) => {
                                    return self.stream.error_here::<_>(
                                        "Cannot mix typed `field: T` and Tint `field` enum fields",
                                    );
                                }
                            }

                            fields.push(StructField::TintField {
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

                // Tuple-style variants are useful for compact events, for
                // example `Move(i32, i32)`.
                TokenKind::LParen => {
                    self.stream.next();
                    let mut fields = Vec::new();

                    if !self.stream.consume_if(TokenKind::RParen) {
                        loop {
                            fields.push(self.parse_type()?);
                            if !self.stream.consume_if(TokenKind::Comma) {
                                break;
                            }
                        }
                        self.stream.expect(TokenKind::RParen)?;
                    }

                    EnumVariant::Tuple(variant_name, fields)
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
