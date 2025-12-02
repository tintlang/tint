// rune-parser/ui.rs

use rune_lexer::TokenKind;
use rune_ast::{UiNode, UiNodeOrExpr, UiAttribute, UiAttrValue, Span};
use crate::{Parser, error::*};

impl Parser {
    // ---------------------------------------------------------
    // <Tag ...>...</Tag>
    // ---------------------------------------------------------
    pub fn parse_ui_node(&mut self) -> PResult<UiNode> {
        let start = self.stream.expect(TokenKind::LAngle)?.span;

        let name = self.parse_ident()?;
        let attributes = self.parse_attributes()?;

        // <Tag />
        if let Some(tok) = self.stream.consume_if_ret(TokenKind::SlashAngle) {
            return Ok(UiNode::SelfClosing {
                name,
                attributes,
                span: Span::merge(start, tok.span),
            });
        }

        self.stream.expect(TokenKind::RAngle)?; // >

        let children = self.parse_ui_children()?;

        // </Tag>
        let close_start = self.stream.expect(TokenKind::AngleSlash)?.span;
        let end_name = self.parse_ident()?;
        let close_end = self.stream.expect(TokenKind::RAngle)?.span;

        if end_name != name {
            return Err(ParserError::Message {
                msg: format!("Mismatched closing tag </{}> for <{}>", end_name, name),
                span: close_start,
            });
        }

        Ok(UiNode::Element {
            name,
            attributes,
            children,
            span: Span::merge(start, close_end),
        })
    }

    // ---------------------------------------------------------
    // Children: text, nested nodes
    // ---------------------------------------------------------
    pub fn parse_ui_children(&mut self) -> PResult<Vec<UiNodeOrExpr>> {
        let mut children = Vec::new();

        loop {
            let kind = self.stream.peek().kind.clone();

            match kind {
                TokenKind::AngleSlash => break,
                TokenKind::LAngle => {
                    children.push(UiNodeOrExpr::Node(self.parse_ui_node()?));
                }
                TokenKind::String => {
                    let t = self.stream.next().clone();
                    children.push(UiNodeOrExpr::Text(t.lexeme.clone(), t.span));
                }
                _ => break,
            }
        }

        Ok(children)
    }

    // ---------------------------------------------------------
    // Attribute list
    // ---------------------------------------------------------
    pub fn parse_attributes(&mut self) -> PResult<Vec<UiAttribute>> {
        let mut attrs = Vec::new();

        loop {
            let next = self.stream.peek().clone();

            if next.kind != TokenKind::Ident {
                break;
            }

            let name = self.parse_ident()?;
            let start = next.span;

            self.stream.expect(TokenKind::Eq)?;
            let value_tok = self.stream.next().clone();

            attrs.push(UiAttribute {
                name,
                value: UiAttrValue::Literal(value_tok.lexeme.clone()),
                span: Span::merge(start, value_tok.span),
            });
        }

        Ok(attrs)
    }
}
