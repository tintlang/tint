// rune-parser/parse_ui.rs

use crate::Parser;
use crate::error::*;
use rune_ast::*;
use rune_lexer::TokenKind;

impl Parser {
    /// Parse:  ui fn Name(params...) { <UI> }
    pub(crate) fn parse_ui_fn(&mut self) -> PResult<UiFnDecl> {
        // ui
        let start_tok = self.stream.expect(TokenKind::Ui)?;
        // fn
        self.stream.expect(TokenKind::Fn)?;

        // OPTIONAL ATTRIBUTES: [@(strict, speed)]
        let attributes = self.parse_attributes()?;  

        // fn name
        let name = self.parse_ident()?;

        // parameters (...)
        self.stream.expect(TokenKind::LParen)?;
        let params = self.parse_params()?;        // ✔ now supported
        self.stream.expect(TokenKind::RParen)?;

        // "{"
        self.stream.expect(TokenKind::LBrace)?;

        // Parse exactly ONE top-level UI node
        let body = self.parse_ui_node()?;         // ✔ handled in ui.rs

        // "}"
        let end_tok = self.stream.expect(TokenKind::RBrace)?;

        Ok(UiFnDecl {
            attributes,
            name,
            params,
            body,
            span: Span::merge(start_tok.span, end_tok.span),
        })
    }
}
