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
        // async?
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
                name,
                params,
                ret_ty,
                async_: async_span.is_some(),
                body: FnBody::Expr(expr),
                span,
            });
        }

        // -------- { BLOCK } --------
        let block = self.parse_block()?;
        let span = Span::merge(start, block.span);

        Ok(FnDecl {
            name,
            params,
            ret_ty,
            async_: async_span.is_some(),
            body: FnBody::Block(block),
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

        // empty ()
        if self.stream.consume_if(TokenKind::RParen) {
            return Ok(vec![]);
        }

        let mut params = Vec::new();
        let mut seen_default = false;

        loop {
            // -------- 1) PATTERN --------
            self.in_pattern = true;
            let pat = self.parse_pattern()?; // теперь можно tuple/struct/wildcard
            self.in_pattern = false;

            // -------- 2) OPTIONAL TYPE --------
            let ty = if self.stream.consume_if(TokenKind::Colon) {
                Some(self.parse_type()?)
            } else {
                None
            };

            // -------- 3) OPTIONAL DEFAULT --------
            let default = if self.stream.consume_if(TokenKind::LBrace) {
                let expr = self.parse_expr()?;
                self.stream.expect(TokenKind::RBrace)?;
                seen_default = true;
                Some(expr)
            } else {
                if seen_default && ty.is_none() {
                    return Err(ParserError::Message {
                        msg: "Cannot mix parameters with and without default values".into(),
                        span: self.stream.peek().span,
                    });
                }
                None
            };

            params.push(Param { pattern: pat, ty, default });

            // -------- 4) COMMA? --------
            if !self.stream.consume_if(TokenKind::Comma) {
                break;
            }
        }

        self.stream.expect(TokenKind::RParen)?;
        Ok(params)
    }
}
