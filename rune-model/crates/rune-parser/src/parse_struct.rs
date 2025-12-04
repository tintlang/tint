// rune-parser/parse_struct.rs

use crate::Parser;
use crate::error::*;
use rune_ast::*;
use rune_lexer::TokenKind;

impl Parser {

    // Parse field:
    //   id{10}
    //   id: 10
    fn parse_struct_init_field(&mut self) -> PResult<StructInitField> {
        let name = self.parse_ident()?;
        let start = self.stream.last_span();

        // RUST-STYLE: id: expr
        if self.stream.consume_if(TokenKind::Colon) {
            let expr = self.parse_expr()?;
            let end = expr.span();
            return Ok(StructInitField::Assign {
                name,
                expr,
                span: Span::merge(start, end),
            });
        }

        // RUNE-STYLE: id{expr}
        if self.stream.consume_if(TokenKind::LBrace) {
            let expr = self.parse_expr()?;
            self.stream.expect(TokenKind::RBrace)?;
            let end = expr.span();
            return Ok(StructInitField::Rune {
                name,
                expr,
                span: Span::merge(start, end),
            });
        }

        Err(ParserError::Message {
            msg: "Expected ':' or '{' in struct initializer".into(),
            span: start,
        })
    }


pub(crate) fn parse_struct(&mut self) -> PResult<StructDecl> {
    // consume `struct`
    let start = self.stream.expect(TokenKind::Struct)?.span;

    // name
    let name = self.parse_ident()?;

    // <T>
    let _generics = self.parse_optional_generics()?;

    // {
    self.stream.expect(TokenKind::LBrace)?;

    let mut fields = Vec::new();
    let mut style: Option<&'static str> = None;  // "typed" | "rune"

    while !self.stream.consume_if(TokenKind::RBrace) {
        let field = self.parse_struct_field()?; 

        // Determine this field's style
        let this_style = match &field {
            StructField::Typed { .. } => "typed",
            StructField::RuneTyped { .. } => "rune",
        };

        // First field → set style
        if style.is_none() {
            style = Some(this_style);
        } else {
            // Check consistency
            if style.unwrap() != this_style {
                return Err(ParserError::Message {
                    msg: format!(
                        "Mixed struct field styles are not allowed. \
                         Expected all `{}` fields, but found `{}`.",
                        style.unwrap(),
                        this_style
                    ),
                    span: field.span(),
                });
            }
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


    // Parse struct initialization:
    //   User { id{10}, name{"Mark"} }
    //   User { id: 10, name: "X" }
    pub(crate) fn parse_struct_init(&mut self, name: String, start: Span) -> PResult<Expr> {
        self.stream.expect(TokenKind::LBrace)?;

        // Empty: User {}
        if self.stream.consume_if(TokenKind::RBrace) {
            let end = self.stream.last_span();
            return Ok(Expr::StructInit {
                name,
                fields: vec![],
                span: Span::merge(start, end),
            });
        }

        // -----------------------------
        // CHECK for STRUCT UPDATE
        // -----------------------------
        if self.stream.consume_if(TokenKind::DotDot) {
            let base_ident = self.parse_ident()?;
            let base_expr = Expr::Ident(base_ident, start);

            self.stream.consume_if(TokenKind::Comma);

            let mut updates = Vec::new();
            let mut style: Option<&'static str> = None;

            while !self.stream.consume_if(TokenKind::RBrace) {
                let field = self.parse_struct_init_field()?;

                // determine style
                let this_style = match &field {
                    StructInitField::Assign { .. } => "assign",
                    StructInitField::Rune { .. } => "rune",
                };

                if style.is_none() {
                    style = Some(this_style);
                } else if style.unwrap() != this_style {
                    return Err(ParserError::Message {
                        msg: format!(
                            "Mixed struct-update field styles are not allowed. \
                            Expected all `{}` fields, but found `{}`.",
                            style.unwrap(),
                            this_style
                        ),
                        span: field.span(),
                    });
                }

                updates.push(field);

                if !self.stream.consume_if(TokenKind::Comma) {
                    break;
                }
            }

            let end = self.stream.last_span();
            return Ok(Expr::StructUpdate {
                base: Box::new(base_expr),
                updates,
                span: Span::merge(start, end),
            });
        }

        // -----------------------------
        // NORMAL STRUCT INITIALIZATION
        // -----------------------------
        let mut fields = Vec::new();
        let mut style: Option<&'static str> = None;

        loop {
            // BEFORE parsing fields, handle legacy "=" syntax error
            if self.stream.consume_if(TokenKind::Eq) {
                return Err(ParserError::Message {
                    msg: "Struct init does not support `=`. Use either Rust-style `:` or Rune-style `{}`".into(),
                    span: self.stream.last_span(),
                });
            }

            let field = self.parse_struct_init_field()?;

            let this_style = match &field {
                StructInitField::Assign { .. } => "rust",
                StructInitField::Rune { .. } => "rune",
            };

            if style.is_none() {
                style = Some(this_style);
            } else if style.unwrap() != this_style {
                return Err(ParserError::Message {
                    msg: format!(
                        "Mixed struct-init field styles are not allowed. \
                        Expected all `{}` fields, but found `{}`.",
                        style.unwrap(),
                        this_style
                    ),
                    span: field.span(),
                });
            }

            fields.push(field);

            if !self.stream.consume_if(TokenKind::Comma) {
                break;
            }

            if self.stream.peek_kind() == TokenKind::RBrace {
                break;
            }
        }

        self.stream.expect(TokenKind::RBrace)?;
        let end = self.stream.last_span();

        Ok(Expr::StructInit {
            name,
            fields,
            span: Span::merge(start, end),
        })
    }

    // ---------------------------------------------------------
    // PARSE ONE STRUCT FIELD
    // Supports:
    //   id: i32
    //   id{i32}
    // ---------------------------------------------------------
    fn parse_struct_field(&mut self) -> PResult<StructField> {
        let name = self.parse_ident()?;
        let start = self.stream.last_span();

        // RUST STYLE: name: Type
        if self.stream.consume_if(TokenKind::Colon) {
            let ty = self.parse_type()?;
            let end = self.stream.last_span();
            return Ok(StructField::Typed {
                name,
                ty,
                span: Span::merge(start, end),
            });
        }

        // RUNE STYLE: name{Type}
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

        // ERROR
        Err(ParserError::Message {
            msg: "Expected ':' or '{' after struct field name".into(),
            span: start,
        })
    }

    // ---------------------------------------------------------
    // GENERICS LIST: <T, U>
    // ---------------------------------------------------------
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
