// rune-parser/parse_ui.rs

use crate::Parser;
use crate::error::*;
use rune_ast::*;
use rune_lexer::TokenKind;

impl Parser {
    /// Parse:  ui fn Name(params...) { <UI> }
   pub(crate) fn parse_ui_fn(&mut self) -> PResult<UiFnDecl> {
        // "ui"
        let start = self.stream.expect(TokenKind::Ui)?.span;

        // "fn"
        self.stream.expect(TokenKind::Fn)?;

        // LOCAL ATTRIBUTES: ui fn [@(strict, speed)]
        let mut local_attrs = AttributeList::empty();

        if self.stream.peek_kind() == TokenKind::LBracket {
            let a = self.parse_attributes()?; 
            local_attrs.extend(a);

            // forbid SECOND block (как в fn)
            if self.stream.peek_kind() == TokenKind::LBracket {
                return Err(ParserError::Message {
                    msg: "Only one attribute block allowed after `ui fn`".into(),
                    span: self.stream.peek().span,
                });
            }
        }

        // NAME
        let name = self.parse_ident()?;

        // PARAMS (...)
        self.stream.expect(TokenKind::LParen)?;
        let params = self.parse_params()?;
        self.stream.expect(TokenKind::RParen)?;

        // UI BODY
        self.stream.expect(TokenKind::LBrace)?;
        let body = self.parse_ui_node()?;
        let end = self.stream.expect(TokenKind::RBrace)?.span;

        Ok(UiFnDecl {
            attributes: local_attrs,
            name,
            params,
            body,
            span: Span::merge(start, end),
        })
    }
}
