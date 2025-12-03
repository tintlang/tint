// rune-parser/ui.rs
// rune-parser/ui.rs
use rune_lexer::TokenKind;
use rune_ast::*;
use crate::{Parser, error::*};

// Helper: Convert UiModifier → Vec<UiModifierItem>
// =============================================
fn modifier_to_items(modifier: UiModifier) -> Vec<UiModifierItem> {
    match modifier.value {

        UiModifierValue::Block(children) => {
            children.into_iter().flat_map(modifier_to_items).collect()
        }

        UiModifierValue::Number(n) => vec![UiModifierItem {
            key: modifier.path.last().unwrap().clone(),
            value: Some(UiModifierValue::Number(n)),
            children: vec![],
            span: modifier.span,
        }],

        UiModifierValue::String(s) => vec![UiModifierItem {
            key: modifier.path.last().unwrap().clone(),
            value: Some(UiModifierValue::String(s)),
            children: vec![],
            span: modifier.span,
        }],

        UiModifierValue::Expr(expr) => vec![UiModifierItem {
            key: modifier.path.last().unwrap().clone(),
            value: Some(UiModifierValue::Expr(expr)),
            children: vec![],
            span: modifier.span,
        }],
    }
}

impl Parser {

    // ==========================================================
    // Parse a full UI node: <Tag ...>...</Tag>
    // ==========================================================
    pub fn parse_ui_node(&mut self) -> PResult<UiNode> {
        let start = self.stream.expect(TokenKind::LAngle)?.span;

        let name = self.parse_ident()?;
        let attributes = self.parse_ui_attributes()?;

        // parse modifiers: padding{...}, radius{...}
        let mut modifiers = Vec::new();

        loop {
            if self.stream.peek().kind != TokenKind::Ident {
                break;
            }

            let mut i = 1;

            // skip dotted path: padding.x.y
            loop {
                if self.stream.peek_n(i).kind == TokenKind::Dot
                    && self.stream.peek_n(i + 1).kind == TokenKind::Ident
                {
                    i += 2;
                } else {
                    break;
                }
            }

            // check "{"
            if self.stream.peek_n(i).kind != TokenKind::LBrace {
                break;
            }

            modifiers.push(self.parse_ui_modifier()?);
        }

        // <Tag />
        if let Some(tok) = self.stream.consume_if_ret(TokenKind::SlashAngle) {
            return Ok(UiNode::SelfClosing {
                name,
                attributes,
                modifiers,
                span: Span::merge(start, tok.span),
            });
        }

        // >
        self.stream.expect(TokenKind::RAngle)?;

        // children
        let children = self.parse_ui_children()?;

        // </Tag>
        let close = self.stream.expect(TokenKind::AngleSlash)?;
        let close_name = self.parse_ident()?;
        let end = self.stream.expect(TokenKind::RAngle)?.span;

        if close_name != name {
            return Err(ParserError::Message {
                msg: format!("Expected </{}> but got </{}>", name, close_name),
                span: close.span,
            });
        }

        Ok(UiNode::Element {
            name,
            attributes,
            modifiers,
            children,
            span: Span::merge(start, end),
        })
    }

    // ==========================================================
    // Children
    // ==========================================================
    pub fn parse_ui_children(&mut self) -> PResult<Vec<UiNodeOrExpr>> {
        let mut out = Vec::new();

        loop {
            match self.stream.peek().kind.clone() {
                TokenKind::AngleSlash => break,
                TokenKind::LAngle => {
                    out.push(UiNodeOrExpr::Node(self.parse_ui_node()?));
                }
                TokenKind::String => {
                    let tok = self.stream.next().clone();
                    let txt = self.parse_interpolated_text(tok.lexeme.clone(), tok.span)?;
                    out.push(UiNodeOrExpr::Text(txt));
                }
                _ => break,
            }
        }

        Ok(out)
    }

    // ==========================================================
    // UI attributes
    // ==========================================================
  pub fn parse_ui_attributes(&mut self) -> PResult<Vec<UiAttribute>> {
    let mut attrs = Vec::new();

    while self.stream.peek().kind == TokenKind::Ident
        && self.stream.peek2_kind() == TokenKind::Eq
        {
        let name = self.parse_ident()?;
        let start_span = self.stream.last_span();

        self.stream.expect(TokenKind::Eq)?;

        let value = match self.stream.peek().kind.clone() {

            // text="hello"
            TokenKind::String => {
                let t = self.stream.next().clone();
                UiAttrValue::Literal(t.lexeme.clone())
            }

            // text={a + b}
            TokenKind::LBrace => {
                self.stream.next();
                let expr = self.parse_expr()?;
                self.stream.expect(TokenKind::RBrace)?;
                UiAttrValue::Expr(expr)
            }

            // padding{10}, radius{8}, animate.opacity{...}
            TokenKind::Ident if self.stream.peek2_kind() == TokenKind::LBrace => {
                let modifier = self.parse_ui_modifier()?;

                UiAttrValue::Modifier(UiModifierBlock {
                    path: modifier.path.clone(),
                    items: modifier_to_items(modifier.clone()),
                    span: modifier.span,
                })
            }


            // disabled=true  / color=blue
            TokenKind::Ident => {
                UiAttrValue::Ident(self.parse_ident()?)
            }

            _ => return self.stream.error_here("Invalid attribute value"),
        };

        attrs.push(UiAttribute {
            name,
            value,
            span: Span::merge(start_span, self.stream.last_span()),
        });
    }

    Ok(attrs)
}


    // ==========================================================
    // Unified UiModifier parser
    // ==========================================================
    pub fn parse_ui_modifier(&mut self) -> PResult<UiModifier> {
        let start = self.stream.peek().span;

        let mut path = vec![self.parse_ident()?];
        while self.stream.consume_if(TokenKind::Dot) {
            path.push(self.parse_ident()?);
        }

        self.stream.expect(TokenKind::LBrace)?;

        // simple value
        if let Some(v) = self.try_parse_mod_value()? {
            let end = self.stream.expect(TokenKind::RBrace)?.span;
            return Ok(UiModifier {
                path,
                value: v,
                span: Span::merge(start, end),
            });
        }

        // block
        let mut nested = Vec::new();

        while !self.stream.check(TokenKind::RBrace) {
            nested.push(self.parse_modifier_item()?);
        }

        let end = self.stream.expect(TokenKind::RBrace)?.span;

        Ok(UiModifier {
            path,
            value: UiModifierValue::Block(nested),
            span: Span::merge(start, end),
        })
    }

    // ==========================================================
    // Parse modifier value (number/string/expr)
    // ==========================================================
    fn try_parse_mod_value(&mut self) -> PResult<Option<UiModifierValue>> {
        match self.stream.peek().kind.clone() {
            TokenKind::Number => {
                let tok = self.stream.next();
                Ok(Some(UiModifierValue::Number(tok.lexeme.parse().unwrap())))
            }
            TokenKind::Ident => {
                let tok = self.stream.next();
                Ok(Some(UiModifierValue::String(tok.lexeme.clone())))
            }
            TokenKind::LBrace => {
                self.stream.next();
                let expr = self.parse_expr()?;
                self.stream.expect(TokenKind::RBrace)?;
                Ok(Some(UiModifierValue::Expr(expr)))
            }
            _ => Ok(None),
        }
    }

    // ==========================================================
    // A single item inside block: left:10, from:0, etc.
    // ==========================================================
    fn parse_modifier_item(&mut self) -> PResult<UiModifier> {
        let start = self.stream.peek().span;

        let mut path = vec![self.parse_ident()?];
        while self.stream.consume_if(TokenKind::Dot) {
            path.push(self.parse_ident()?);
        }

        if self.stream.consume_if(TokenKind::Colon) {
            let v = match self.try_parse_mod_value()? {
                Some(v) => v,
                None => return self.stream.error_here("Expected value after ':'"),
            };
            return Ok(UiModifier {
                path,
                value: v,
                span: Span::merge(start, self.stream.last_span()),
            });
        }

        if self.stream.check(TokenKind::LBrace) {
            return self.parse_ui_modifier();
        }

        self.stream.error_here("Invalid modifier syntax")
    }

    // ==========================================================
    // Interpolated UI Text
    // ==========================================================
    fn parse_interpolated_text(&mut self, raw: String, span: Span) -> PResult<UiText> {
        let mut parts = Vec::new();
        let mut buf = String::new();
        let chars: Vec<char> = raw.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            if chars[i] == '{' {
                if !buf.is_empty() {
                    parts.push(UiTextPart::Literal(buf.clone(), span));
                    buf.clear();
                }
                i += 1;

                let mut expr_buf = String::new();
                while i < chars.len() && chars[i] != '}' {
                    expr_buf.push(chars[i]);
                    i += 1;
                }
                i += 1;

                let mut p = Parser::new_expr_only(expr_buf.clone(), span);
                let expr = p.parse_expr()?;
                parts.push(UiTextPart::Interpolation(expr, span));
            } else {
                buf.push(chars[i]);
                i += 1;
            }
        }

        if !buf.is_empty() {
            parts.push(UiTextPart::Literal(buf, span));
        }

        Ok(UiText { parts, span })
    }
}
