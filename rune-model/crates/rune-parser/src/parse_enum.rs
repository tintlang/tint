// rune-parser/parse_enum.rs

use crate::{Parser};
use crate::error::*;
use rune_ast::*;
use rune_lexer::TokenKind;

impl Parser {
    pub(crate) fn parse_enum(&mut self) -> PResult<EnumDecl> {
        // `enum`
        let start = self.stream.expect(TokenKind::Enum)?.span;

        // enum name
        let name = self.parse_ident()?;

        // `{`
        self.stream.expect(TokenKind::LBrace)?;

        let mut variants = Vec::new();

        while !self.stream.consume_if(TokenKind::RBrace) {
            let var_start = self.stream.peek().span;
            let variant_name = self.parse_ident()?;

            let variant = match self.stream.peek_kind() {
                // TUPLE VARIANT: Variant(T1, T2)
                TokenKind::LParen => {
                    self.stream.next(); // consume '('
                    let mut types = Vec::new();

                    if !self.stream.consume_if(TokenKind::RParen) {
                        loop {
                            types.push(self.parse_type()?);

                            if self.stream.consume_if(TokenKind::RParen) {
                                break;
                            }

                            self.stream.expect(TokenKind::Comma)?;
                        }
                    }

                    EnumVariant::Tuple(variant_name, types)
                }

                // STRUCT VARIANT: Variant { a: T, b: U }
                TokenKind::LBrace => {
                    self.stream.next(); // consume '{'
                    let mut fields = Vec::new();

                    if !self.stream.consume_if(TokenKind::RBrace) {
                        loop {
                            let field_start = self.stream.peek().span;

                            let field_name = self.parse_ident()?;
                            self.stream.expect(TokenKind::Colon)?;
                            let field_ty = self.parse_type()?;

                            let field_end = self.stream.last_span();

                            fields.push(StructField::Typed {
                                name: field_name,
                                ty: field_ty,
                                span: Span::merge(field_start, field_end),
                            });

                            if self.stream.consume_if(TokenKind::RBrace) {
                                break;
                            }

                            self.stream.expect(TokenKind::Comma)?;
                        }
                    }

                    EnumVariant::Struct(variant_name, fields)
                }

                // UNIT VARIANT: Variant
                _ => EnumVariant::Unit(variant_name),
            };

            variants.push(variant);

            // optional trailing comma
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
