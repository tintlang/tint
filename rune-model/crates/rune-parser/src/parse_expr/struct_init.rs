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
            // -------------------------------
            // CASE 1: Struct Update prefix → ..ident
            // -------------------------------
            if self.stream.consume_if(TokenKind::DotDot) {
                let ident = self.parse_ident()?;
                let span = self.stream.last_span();

                base_expr = Some(Expr::Ident(ident, span));
            } else {
                // -------------------------------
                // CASE 2: Normal struct field
                // -------------------------------
                let field = self.parse_struct_init_field()?;
                fields.push(field);
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

        // -------------------------------
        // Decide which Expr to return
        // -------------------------------
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
