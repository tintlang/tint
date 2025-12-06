// rune-parser/parse_expr/struct_init.rs

use crate::{Parser, error::*};
use rune_ast::{Expr, StructInitField, Span};
use rune_lexer::TokenKind;

impl Parser {
    pub(crate) fn parse_struct_init(
        &mut self,
        name: String,
        start_span: Span,
    ) -> PResult<Expr> {

        self.stream.expect(TokenKind::LBrace)?;

        let mut fields = Vec::new();
        let mut base_expr: Option<Expr> = None;

        // -------------------------------
        // StyleDetector: None / Rune / Colon
        // -------------------------------
        #[derive(PartialEq)]
        enum Style { Rune, Colon }
        let mut style: Option<Style> = None;

        // empty struct {}
        if self.stream.consume_if(TokenKind::RBrace) {
            let end = self.stream.last_span();
            return Ok(Expr::StructInit {
                name,
                fields,
                span: Span::merge(start_span, end),
            });
        }

        loop {
            // --------------------------------
            // struct update: ..base
            // --------------------------------
            if self.stream.consume_if(TokenKind::DotDot) {
                let ident = self.parse_ident()?;
                let span = self.stream.last_span();

                base_expr = Some(Expr::Ident(ident, span));
            } else {
                // --------------------------------
                // detect style BEFORE parsing field
                // --------------------------------
                let field_name = self.parse_ident()?;
                let field_span = self.stream.last_span();

                let next = self.stream.peek_kind();

                match next {
                    TokenKind::LBrace => {
                        // rune-style
                        match style {
                            None => style = Some(Style::Rune),
                            Some(Style::Rune) => {}
                            Some(Style::Colon) => {
                                return Err(ParserError::Message {
                                    msg: "Struct initializer mixes Rune `{}` and Colon `:` styles".into(),
                                    span: field_span,
                                });
                            }
                        }

                        self.stream.next(); // consume `{`
                        let expr = self.parse_expr()?;
                        self.stream.expect(TokenKind::RBrace)?;
                        fields.push(StructInitField::Rune {
                            name: field_name,
                            expr,
                            span: field_span,
                        });
                    }

                    TokenKind::Colon => {
                        // colon-style
                        match style {
                            None => style = Some(Style::Colon),
                            Some(Style::Colon) => {}
                            Some(Style::Rune) => {
                                return Err(ParserError::Message {
                                    msg: "Struct initializer mixes Rune `{}` and Colon `:` styles".into(),
                                    span: field_span,
                                });
                            }
                        }

                        self.stream.next(); // consume `:`
                        let expr = self.parse_expr()?;
                        fields.push(StructInitField::Assign {
                            name: field_name,
                            expr,
                            span: field_span,
                        });
                    }

                    _ => {
                        return Err(ParserError::Message {
                            msg: "Expected `{` or `:` in struct field".into(),
                            span: field_span,
                        });
                    }
                }
            }

            // break if no comma
            if !self.stream.consume_if(TokenKind::Comma) {
                break;
            }
            if self.stream.peek_kind() == TokenKind::RBrace {
                break;
            }
        }

        let end = self.stream.expect(TokenKind::RBrace)?.span;

        if let Some(base) = base_expr {
            return Ok(Expr::StructUpdate {
                base: Box::new(base),
                updates: fields,
                span: Span::merge(start_span, end),
            });
        }

        Ok(Expr::StructInit {
            name,
            fields,
            span: Span::merge(start_span, end),
        })
    }
}
