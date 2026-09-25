
use crate::{Parser, error::*};
use rune_ast::{StructDecl, StructField, StructMember, KitRef, Span};
use rune_lexer::TokenKind;

impl Parser {

    // Parse struct member: either field OR @kit.path
    fn parse_struct_member(&mut self) -> PResult<StructMember> {
        let tok = self.stream.peek();

        // Kit references may optionally include a body.
        if tok.kind == TokenKind::At {
            let start = self.stream.next().span; // eaten '@'

            // parse path after '@'
            let mut path = Vec::new();
            path.push(self.parse_ident()?);

            while self.stream.consume_if(TokenKind::Dot) {
                path.push(self.parse_ident()?);
            }

            // parse optional body { ... }
            let body = if self.stream.consume_if(TokenKind::LBrace) {
                let mut members = Vec::new();

                // parse until '}'
                while !self.stream.consume_if(TokenKind::RBrace) {
                    members.push(self.parse_struct_member()?);

                    // optional comma (but not required)
                    self.stream.consume_if(TokenKind::Comma);
                }

                Some(members)
            } else {
                None
            };

            let span = Span::merge(start, self.stream.last_span());

            return Ok(StructMember::Kit(KitRef {
                path,
                body,
                span,
            }));
        }

        // Otherwise parse a regular struct field.
        let field = self.parse_struct_field()?;
        Ok(StructMember::Field(field))
    }

    // Parse the struct body.
    pub(crate) fn parse_struct(&mut self) -> PResult<StructDecl> {
        let start = self.stream.expect(TokenKind::Struct)?.span;

        let name = self.parse_ident()?;

        let _generics = self.parse_optional_generics()?;

        self.stream.expect(TokenKind::LBrace)?;

        let mut fields = Vec::new();          // ONLY StructField here (no kit!)
        let mut style: Option<&'static str> = None;

        // members loop
        while !self.stream.consume_if(TokenKind::RBrace) {
            let member = self.parse_struct_member()?;

            match &member {
                StructMember::Field(f) => {
                    // determine style
                    let this_style = match f {
                        StructField::Typed { .. }     => "typed",
                        StructField::RuneTyped { .. } => "runetyped",
                        StructField::RuneField { .. } => "runefield",
                    };

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
                            span: f.span(),
                        });
                    }

                    fields.push(member);
                }

                StructMember::Kit(_k) => {
                    // kit does NOT participate in typed/runetyped style
                    fields.push(member);
                }
            }

            self.stream.consume_if(TokenKind::Comma);
        }

        let end = self.stream.last_span();

        Ok(StructDecl {
            name,
            members: fields,     // NOTE: YOU MUST UPDATE StructDecl
            exported: false,
            span: Span::merge(start, end),
        })
    }

    // Parse simple struct field
    fn parse_struct_field(&mut self) -> PResult<StructField> {
        let name = self.parse_ident()?;
        let start = self.stream.last_span();

        if self.stream.consume_if(TokenKind::Colon) {
            let ty = self.parse_type()?;
            let end = self.stream.last_span();
            return Ok(StructField::Typed {
                name,
                ty,
                span: Span::merge(start, end),
            });
        }

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
}
