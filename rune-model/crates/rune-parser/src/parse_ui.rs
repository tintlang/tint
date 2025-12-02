// rune-parser/parse_ui.rs

use crate::{Parser};
use crate::{ error::*};
use rune_ast::*;
use rune_lexer::TokenKind;

impl Parser {
    pub(crate) fn parse_ui_fn(&mut self) -> PResult<UiFnDecl> {
        // ui fn
        let start = self.stream.expect(TokenKind::Ui)?.span;
        self.stream.expect(TokenKind::Fn)?;
        let name = self.parse_ident()?;

        // parameters (we ignore them for now, but later: UIChildren, props)
        self.stream.expect(TokenKind::LParen)?;
        self.stream.expect(TokenKind::RParen)?;

        // body starts with "{"
        self.stream.expect(TokenKind::LBrace)?;

        // Parse exactly ONE root UI node
        let body = self.parse_ui_node()?;

        // closing "}"
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
