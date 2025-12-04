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

        // empty: MyType {}
        if self.stream.consume_if(TokenKind::RBrace) {
            let end = self.stream.last_span();
            return Ok(Expr::StructInit {
                name,
                fields,
                span: Span::merge(start_span, end),
            });
        }

        // STYLE GUARD: "colon" | "rune"
        let mut style: Option<&'static str> = None;

        loop {
            // parse one field
            let field = self.parse_struct_init_field()?;

            // detect field style
            let this_style = match &field {
                StructInitField::Assign { .. } => "colon",
                StructInitField::Rune { .. }   => "rune",
            };

            // first field → set style
            if style.is_none() {
                style = Some(this_style);
            } else if style.unwrap() != this_style {
                return Err(ParserError::Message {
                    msg: format!(
                        "Mixed struct-init field styles are not allowed. \
                         Expected all `{}`, but found `{}`.",
                         style.unwrap(),
                         this_style
                    ),
                    span: field.span(),
                });
            }

            fields.push(field);

            // comma or end
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
            span: Span::merge(start_span, end),
        })
    }


    /// Parse one struct init field:
    ///   id: expr
    ///   id { expr }   ← named block argument
   pub(crate) fn parse_struct_init_field(&mut self) -> PResult<StructInitField> {
        let name = self.parse_ident()?;
        let start = self.stream.last_span();

        // colon-style: id: expr
        if self.stream.consume_if(TokenKind::Colon) {
            let expr = self.parse_expr()?;
            let end = expr.span();
            return Ok(StructInitField::Assign {
                name,
                expr,
                span: Span::merge(start, end),
            });
        }

        // rune-style: id { expr }
        if self.stream.consume_if(TokenKind::LBrace) {
            let expr = self.parse_expr()?;
            self.stream.expect(TokenKind::RBrace)?;
            let end = self.stream.last_span();

            return Ok(StructInitField::Rune {
                name,
                expr,
                span: Span::merge(start, end),
            });
        }

        Err(ParserError::Message {
            msg: "Invalid field in struct init".into(),
            span: start,
        })
    }
}
