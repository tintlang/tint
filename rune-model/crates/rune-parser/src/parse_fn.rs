// rune-parser/parse_fn.rs

use crate::{Parser};
use crate::error::*;
use rune_ast::*;
use rune_lexer::TokenKind;

impl Parser {

    // fn name(params...) -> Type { block }
    // async fn name(params...) { ... }
    // fn name(params...) = expr
    pub(crate) fn parse_fn_decl(&mut self) -> PResult<FnDecl> {

        // OPTIONAL ATTRIBUTES: [@(strict, speed)]
        let attributes = self.parse_attributes()?;  

        // async
        let async_span = if self.stream.consume_if(TokenKind::Async) {
            Some(self.stream.last_span())
        } else {
            None
        };

        // "fn"
        let start = self.stream.expect(TokenKind::Fn)?.span;

        // -------- NAME (НЕ ПАТТЕРН!) --------
        let name = self.parse_ident()?; // имя функции должно быть идентификатором

        // -------- PARAMETERS (ПАТТЕРНЫ) --------
        let params = self.parse_fn_params()?;

        // -------- RETURN TYPE --------
        let ret_ty = if self.stream.consume_if(TokenKind::Arrow) {
            Some(self.parse_type()?)
        } else {
            None
        };

        // -------- SHORT FORM: fn x() = expr --------
        if self.stream.consume_if(TokenKind::Eq) {
            let expr = self.parse_expr()?;
            let span = Span::merge(start, expr.span());

            return Ok(FnDecl {
                attributes,
                name,
                params,
                ret_ty,
                async_: async_span.is_some(),
                body: FnBody::Expr(expr),
                exported: false,
                span,
            });
        }

        // -------- { BLOCK } --------
        let block = self.parse_block()?;
        let span = Span::merge(start, block.span);

        Ok(FnDecl {
            attributes,
            name,
            params,
            ret_ty,
            async_: async_span.is_some(),
            body: FnBody::Block(block),
            exported: false,
            span,
        })
    }

    // PARAMS: (pattern: Type {default}, ...)
    //
    // supports:
    // fn f((x,y)) {}
    // fn f(User { id, name }) {}
    // fn f(_, x, (a,(b,c))) {}
    // fn f(x: i32) {}
    // fn f(x {10}) {}
    fn parse_fn_params(&mut self) -> PResult<Vec<Param>> {
    self.stream.expect(TokenKind::LParen)?;

        if self.stream.consume_if(TokenKind::RParen) {
            return Ok(vec![]);
        }

        #[derive(PartialEq)]
        enum Style { Rune, Typed }
        let mut style: Option<Style> = None;

        let mut params = Vec::new();

        loop {
            // 1) PATTERN
            self.in_pattern = true;
            let pat = self.parse_pattern()?;
            self.in_pattern = false;

            let mut ty = None;
            let mut default: Option<DefaultValue> = None;

            // 2) optional type: x: T
            if self.stream.consume_if(TokenKind::Colon) {
                match style {
                    None => style = Some(Style::Typed),
                    Some(Style::Typed) => {}
                    Some(Style::Rune) => {
                        return Err(ParserError::Message {
                            msg: "Cannot mix typed parameters (`x: T`) with Rune parameters (`x {expr}`)".into(),
                            span: self.stream.last_span(),
                        });
                    }
                }

                ty = Some(self.parse_type()?);

                // optional default: x: T = expr
                if self.stream.consume_if(TokenKind::Eq) {
                    let expr = self.parse_expr()?;
                    default = Some(DefaultValue::Single(expr));
                }
            }

            // 3) Rune-style default: pattern {expr}
            else if self.stream.consume_if(TokenKind::LBrace) {
                match style {
                    None => style = Some(Style::Rune),
                    Some(Style::Rune) => {}
                    Some(Style::Typed) => {
                        return Err(ParserError::Message {
                            msg: "Cannot mix Rune-style defaults (`x {expr}`) with typed parameters (`x: T = expr`)".into(),
                            span: self.stream.last_span(),
                        });
                    }
                }

                let expr = self.parse_expr()?;
                self.stream.expect(TokenKind::RBrace)?;

                // SINGLE vs BROADCAST
                match &pat {
                    Pattern::Ident(_, _) => {
                        default = Some(DefaultValue::Single(expr));
                    }
                    _ => {
                        default = Some(DefaultValue::Broadcast(expr));
                    }
                }
            }

            // push parameter
            params.push(Param { pattern: pat, ty, default });

            // , or end
            if !self.stream.consume_if(TokenKind::Comma) {
                break;
            }
        }

        self.stream.expect(TokenKind::RParen)?;
        Ok(params)
    }
}
