use crate::{error::*, Parser};
use tint_ast::*;
use tint_lexer::TokenKind;

mod block;
mod text;
mod value;
mod xml;

impl Parser {
    pub fn parse_ui_root(&mut self) -> PResult<Vec<UiNode>> {
        let mut nodes = Vec::new();

        let mode = self.detect_ui_mode()?;

        while !self.stream.check(TokenKind::RBrace) {
            match mode {
                UiMode::Xml => nodes.push(self.parse_xml_node()?),
                UiMode::Block => {
                    if self.stream.peek().lexeme == "theme"
                        && self.stream.peek2_kind() == TokenKind::PathSep
                        && self.stream.peek_n_kind(2) == TokenKind::Ident
                        && self.stream.peek_n_kind(3) == TokenKind::LBrace
                    {
                        nodes.push(self.parse_theme_node()?);
                    } else {
                        nodes.push(self.parse_block_node()?);
                    }
                }
            }
        }

        Ok(nodes)
    }

    fn detect_ui_mode(&mut self) -> PResult<UiMode> {
        match self.stream.peek().kind {
            TokenKind::LAngle => Ok(UiMode::Xml),
            TokenKind::Ident => {
                if self.stream.peek2_kind() == TokenKind::LBrace
                    || (self.stream.peek().lexeme == "theme"
                        && self.stream.peek2_kind() == TokenKind::PathSep
                        && self.stream.peek_n_kind(2) == TokenKind::Ident
                        && self.stream.peek_n_kind(3) == TokenKind::LBrace)
                {
                    Ok(UiMode::Block)
                } else {
                    Err(ParserError::Message {
                        msg: "UI must start either with <Tag> (XML mode) or Tag { } (Block mode)"
                            .into(),
                        span: self.stream.peek().span,
                    })
                }
            }
            _ => Err(ParserError::Message {
                msg: "Invalid beginning of UI. Expected <Tag> or Tag { }".into(),
                span: self.stream.peek().span,
            }),
        }
    }

    pub(crate) fn parse_attribute_rhs(&mut self) -> PResult<UiAttrValue> {
        match self.stream.peek().kind {
            TokenKind::Ident => {
                let tok = self.stream.next();
                Ok(UiAttrValue::Ident(tok.lexeme.clone()))
            }

            TokenKind::String => {
                let tok = self.stream.next();
                Ok(UiAttrValue::Literal(tok.lexeme.clone()))
            }

            TokenKind::LBrace => {
                self.stream.next();
                let expr = self.parse_expr()?;
                self.stream.expect(TokenKind::RBrace)?;
                Ok(UiAttrValue::Expr(expr))
            }

            _ => self.stream.error_here("Invalid value after ||"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum UiMode {
    Xml,
    Block,
}
