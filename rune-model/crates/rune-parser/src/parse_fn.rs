// rune-parser/parse_fn.rs

use crate::{Parser};
use crate::{ error::*};
use rune_ast::*;
use rune_lexer::TokenKind;

impl Parser {
    pub(crate) fn parse_fn(&mut self) -> PResult<FnDecl> {
        let start = self.stream.expect(TokenKind::Fn)?.span;

        let name = self.parse_ident()?;

        self.stream.expect(TokenKind::LParen)?;
        self.stream.expect(TokenKind::RParen)?;

        let body = self.parse_block()?;
        let body_span = body.span; 

        Ok(FnDecl {
            name,
            params: vec![],
            body,
            span: Span::merge(start, body_span),
        })
    }
}
