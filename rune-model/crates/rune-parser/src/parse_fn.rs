// rune-parser/parse_fn.rs

use crate::{Parser};
use crate::error::*;
use rune_ast::*;
use rune_lexer::TokenKind;

impl Parser {

    // fn name(params...) -> Type { block }
    // fn name(params...) = expr
    // async fn name(...) { ... }
    pub(crate) fn parse_fn_decl(&mut self) -> PResult<FnDecl> {
        // async?
        let async_span = if self.stream.consume_if(TokenKind::Async) {
            Some(self.stream.last_span())
        } else {
            None
        };

        let start = self.stream.expect(TokenKind::Fn)?.span;

        let name = self.parse_ident()?;

        // PARAMETERS
        let params = self.parse_fn_params()?;

        // RETURN TYPE (optional)
        // fn x() -> T { ... }
        let ret_ty = if self.stream.consume_if(TokenKind::Arrow) {
            Some(self.parse_type()?)
        } else {
            None
        };

        // SHORT FORM
        // fn x() = expr
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

        // NORMAL BLOCK BODY
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

    // PARAMS: (a: i32, b: f32, string{""})
fn parse_fn_params(&mut self) -> PResult<Vec<Param>> {
    self.stream.expect(TokenKind::LParen)?;

    // empty params: ()
    if self.stream.consume_if(TokenKind::RParen) {
        return Ok(vec![]);
    }

    let mut params = Vec::new();
    let mut seen_default = false;

    loop {
        // ----- 1) name -----
        let name = self.parse_ident()?;

        // ----- 2) optional ": type" -----
        let ty = if self.stream.consume_if(TokenKind::Colon) {
            Some(self.parse_type()?)
        } else {
            None
        };

        // ----- 3) optional default { expr } -----
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

        params.push(Param { name, ty, default });

        // ----- 4) comma? -----
        if !self.stream.consume_if(TokenKind::Comma) {
            break;
        }
    }

    self.stream.expect(TokenKind::RParen)?;
    Ok(params)
}

}
