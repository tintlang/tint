// rune-parser/parse_ui.rs

use crate::{Parser};
use crate::{ error::*};
use rune_ast::*;
use rune_lexer::TokenKind;

impl Parser {
    pub(crate) fn parse_ui_fn(&mut self) -> PResult<UiFnDecl> {
        let start = self.stream.expect(TokenKind::Ui)?.span;

        self.stream.expect(TokenKind::Fn)?;
        let name = self.parse_ident()?;

        self.stream.expect(TokenKind::LParen)?;
        self.stream.expect(TokenKind::RParen)?;

        // expect { 
        self.stream.expect(TokenKind::LBrace)?;

        // a single root UI node
        let body = self.parse_ui_node()?;

        // expect }
        self.stream.expect(TokenKind::RBrace)?;

        let end = body.span();

        Ok(UiFnDecl {
            name,
            params: vec![],
            body,
            span: Span::merge(start, end),
        })
    }
}
