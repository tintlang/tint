// rune-parser/ui.rs
// UI parser for Rune DSL — STRICT MODE (NO MIXING)

use rune_lexer::TokenKind;
use rune_ast::*;
use crate::{Parser, error::*};

impl Parser {

    // ─────────────────────────────────────────────
    //  ENTRY: parse a full UI block inside ui { }
    // ─────────────────────────────────────────────
    pub fn parse_ui_root(&mut self) -> PResult<Vec<UiNode>> {
        let mut nodes = Vec::new();

        // Determine mode (XML or BLOCK) by first token
        let mode = self.detect_ui_mode()?;

        while !self.stream.check(TokenKind::RBrace) {
            match mode {
                UiMode::Xml => nodes.push(self.parse_xml_node()?),
                UiMode::Block => nodes.push(self.parse_block_node()?),
            }
        }

        Ok(nodes)
    }

    // Determine mode of UI syntax
    fn detect_ui_mode(&mut self) -> PResult<UiMode> {
        match self.stream.peek().kind {
            TokenKind::LAngle => Ok(UiMode::Xml),
            TokenKind::Ident => {
                // Tag { ... }
                if self.stream.peek2_kind() == TokenKind::LBrace {
                    Ok(UiMode::Block)
                } else {
                    Err(ParserError::Message {
                        msg: "UI must start either with <Tag> (XML mode) or Tag { } (Block mode)".into(),
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

    fn parse_attribute_rhs(&mut self) -> PResult<UiAttrValue> {
    match self.stream.peek().kind {

        // ident → theme||dark, click||increment
        TokenKind::Ident => {
            let tok = self.stream.next();
            Ok(UiAttrValue::Ident(tok.lexeme.clone()))
        }

        // string → label||"Hello"
        TokenKind::String => {
            let tok = self.stream.next();
            Ok(UiAttrValue::Literal(tok.lexeme.clone()))
        }

        // expression → opacity||{ progress * 0.5 }
        TokenKind::LBrace => {
            self.stream.next(); // {
            let expr = self.parse_expr()?;
            self.stream.expect(TokenKind::RBrace)?;
            Ok(UiAttrValue::Expr(expr))
        }

        _ => self.stream.error_here("Invalid value after ||"),
    }
}


    // ─────────────────────────────────────────────
    //  XML MODE
    // ─────────────────────────────────────────────
pub fn parse_xml_node(&mut self) -> PResult<UiNode> {
    let start = self.stream.expect(TokenKind::LAngle)?.span;
    let name = self.parse_ident()?;

    // FIX: здесь мы разбираем tuple
    let (attributes, modifiers) = self.parse_modifier_list_xml()?;

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

    let children = self.parse_xml_children()?;

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


    fn parse_xml_children(&mut self) -> PResult<Vec<UiNodeOrExpr>> {
        let mut out = Vec::new();

        loop {
            match self.stream.peek().kind {
                TokenKind::AngleSlash => break,
                TokenKind::LAngle => out.push(UiNodeOrExpr::Node(self.parse_xml_node()?)),
                TokenKind::String => {
                    let t = self.stream.next().clone();
                    let parsed = self.parse_interpolated_text(t.lexeme.clone(), t.span)?;
                    out.push(UiNodeOrExpr::Text(parsed));
                }
                _ => break,
            }
        }

        Ok(out)
    }

    // Parse modifiers like padding::20, border.opacity::{0.2 -> 1.0}
fn parse_modifier_list_xml(
    &mut self
) -> PResult<(Vec<UiAttribute>, Vec<UiModifier>)>
{
    let mut attrs = Vec::new();
    let mut mods  = Vec::new();

    while self.stream.peek().kind == TokenKind::Ident {
        // read identifier
        let tok = self.stream.next();
        let name = tok.lexeme.clone();
        let span = tok.span;

        // ATTRIBUTE: name||value
        if self.stream.peek().kind == TokenKind::OrOr {
            self.stream.next(); // ||
            let value = self.parse_attribute_rhs()?;

            attrs.push(UiAttribute {
                name,       // <-- no dots allowed
                value,
                span,
            });

            continue;
        }

        // MODIFIER: name(.segment)*::value
        let mut path = vec![name];

        // allow dot-nested segments ONLY for modifiers
        while self.stream.peek().kind == TokenKind::Dot {
            self.stream.next(); // consume '.'
            let seg = self.parse_ident()?;
            path.push(seg);
        }

        // now expect ::
        if self.stream.peek().kind == TokenKind::PathSep {
            self.stream.next(); // ::

            let value = self.parse_modifier_value_rhs()?;

            mods.push(UiModifier {
                path,
                value,
                span,
            });

            continue;
        }

        // unknown sequence → stop
        break;
    }
    // Handle control flow keywords: if{...}, for{item in list}, match{...}
    loop {
        match self.stream.peek().kind {
            TokenKind::If => {
                let tok = self.stream.next();
                let span = tok.span;

                self.stream.expect(TokenKind::LBrace)?;
                let expr = self.parse_expr()?;
                self.stream.expect(TokenKind::RBrace)?;

                mods.push(UiModifier {
                    path: vec!["if".to_string()],
                    value: UiModifierValue::Expr(expr),
                    span,
                });
            }
            TokenKind::For => {
                let tok = self.stream.next();
                let span = tok.span;

                self.stream.expect(TokenKind::LBrace)?;
                
                // Parse loop variable (just an identifier for now)
                let _var = self.parse_ident()?;
                
                // Expect "in"
                self.stream.expect(TokenKind::In)?;
                
                // Parse iterable expression
                let expr = self.parse_expr()?;
                
                self.stream.expect(TokenKind::RBrace)?;

                // Store the for modifier
                mods.push(UiModifier {
                    path: vec!["for".to_string()],
                    value: UiModifierValue::Expr(expr),
                    span,
                });
            }
            TokenKind::Match => {
                let tok = self.stream.next();
                let span = tok.span;

                self.stream.expect(TokenKind::LBrace)?;
                let expr = self.parse_expr()?;
                self.stream.expect(TokenKind::RBrace)?;

                mods.push(UiModifier {
                    path: vec!["match".to_string()],
                    value: UiModifierValue::Expr(expr),
                    span,
                });
            }
            _ => break,
        }
    }



    Ok((attrs, mods))
}


    // ─────────────────────────────────────────────
    //  BLOCK MODE
    // ─────────────────────────────────────────────

pub fn parse_block_node(&mut self) -> PResult<UiNode> {
    let start = self.stream.peek().span;
    let name = self.parse_ident()?;

    self.stream.expect(TokenKind::LBrace)?;

    let (attributes, modifiers) = self.parse_modifier_list_block()?;
    let children = self.parse_block_children()?;

    let end = self.stream.expect(TokenKind::RBrace)?.span;

    Ok(UiNode::BlockElement {
        name,
        attributes,
        modifiers,
        children,
        span: Span::merge(start, end),
    })
}

    fn parse_block_children(&mut self) -> PResult<Vec<UiNodeOrExpr>> {
        let mut out = Vec::new();

        loop {
            match self.stream.peek().kind {
                TokenKind::RBrace => break,
                TokenKind::Ident
                    if self.stream.peek2_kind() == TokenKind::LBrace =>
                {
                    out.push(UiNodeOrExpr::Node(self.parse_block_node()?));
                }

                TokenKind::String => {
                    let tok = self.stream.next().clone();
                    let parsed = self.parse_interpolated_text(tok.lexeme.clone(), tok.span)?;
                    out.push(UiNodeOrExpr::Text(parsed));
                }

                _ => break,
            }
        }

        Ok(out)
    }

fn parse_modifier_list_block(
    &mut self
) -> PResult<(Vec<UiAttribute>, Vec<UiModifier>)>
{
    let mut attrs = Vec::new();
    let mut mods  = Vec::new();

    while self.stream.peek().kind == TokenKind::Ident {

        // IMPORTANT:
        // If next tokens form a child node: Ident + '{'
        // then STOP parsing attributes/modifiers.
        if self.stream.peek2_kind() == TokenKind::LBrace {
            break;
        }

        let tok = self.stream.next();
        let name = tok.lexeme.clone();
        let span = tok.span;

        // ATTRIBUTE:  name||rhs
        if self.stream.peek().kind == TokenKind::OrOr {
            self.stream.next(); // ||
            let value = self.parse_attribute_rhs()?;

            attrs.push(UiAttribute {
                name,
                value,
                span,
            });

            continue;
        }

        // MODIFIER: dotted.path::rhs
        let mut path = vec![name];

        while self.stream.peek().kind == TokenKind::Dot {
            self.stream.next();  // consume '.'
            let seg = self.parse_ident()?;
            path.push(seg);
        }

        // expect ::
        if self.stream.peek().kind == TokenKind::PathSep {
            self.stream.next(); // ::

            let value = self.parse_modifier_value_rhs()?;

            mods.push(UiModifier {
                path,
                value,
                span,
            });

            continue;
        }

        break;
    }
    // Handle control flow keywords: if{...}, for{item in list}, match{...}
    loop {
        match self.stream.peek().kind {
            TokenKind::If => {
                let tok = self.stream.next();
                let span = tok.span;

                self.stream.expect(TokenKind::LBrace)?;
                let expr = self.parse_expr()?;
                self.stream.expect(TokenKind::RBrace)?;

                mods.push(UiModifier {
                    path: vec!["if".to_string()],
                    value: UiModifierValue::Expr(expr),
                    span,
                });
            }
            TokenKind::For => {
                let tok = self.stream.next();
                let span = tok.span;

                self.stream.expect(TokenKind::LBrace)?;
                
                // Parse loop variable (just an identifier for now)
                let _var = self.parse_ident()?;
                
                // Expect "in"
                self.stream.expect(TokenKind::In)?;
                
                // Parse iterable expression
                let expr = self.parse_expr()?;
                
                self.stream.expect(TokenKind::RBrace)?;

                // Store the for modifier
                mods.push(UiModifier {
                    path: vec!["for".to_string()],
                    value: UiModifierValue::Expr(expr),
                    span,
                });
            }
            TokenKind::Match => {
                let tok = self.stream.next();
                let span = tok.span;

                self.stream.expect(TokenKind::LBrace)?;
                let expr = self.parse_expr()?;
                self.stream.expect(TokenKind::RBrace)?;

                mods.push(UiModifier {
                    path: vec!["match".to_string()],
                    value: UiModifierValue::Expr(expr),
                    span,
                });
            }
            _ => break,
        }
    }



    Ok((attrs, mods))
}

fn parse_modifier_value_rhs(&mut self) -> PResult<UiModifierValue> {
    match self.stream.peek().kind {

        // NUMBER
        TokenKind::Number => {
            let tok = self.stream.next();
            let v: f64 = tok.lexeme.parse().unwrap();
            Ok(UiModifierValue::Number(v))
        }

        // STRING
        TokenKind::String => {
            let tok = self.stream.next();
            Ok(UiModifierValue::String(tok.lexeme.clone()))
        }

        // IDENT (NEW!)
        TokenKind::Ident => {
            let tok = self.stream.next();
            Ok(UiModifierValue::Ident(tok.lexeme.clone()))
        }

        // TUPLE LIST: { ... }
        TokenKind::LBrace => {
            self.stream.expect(TokenKind::LBrace)?;

            // empty {}
            if self.stream.check(TokenKind::RBrace) {
                self.stream.next();
                return Ok(UiModifierValue::Tuple(vec![]));
            }

            let mut items = Vec::new();

            loop {
                let val = self.parse_single_mod_value()?;
                items.push(val);

                if self.stream.consume_if(TokenKind::Comma) {
                    continue;
                }

                self.stream.expect(TokenKind::RBrace)?;
                break;
            }

            Ok(UiModifierValue::Tuple(items))
        }

        _ => Err(ParserError::Message {
            msg: "Invalid modifier value".into(),
            span: self.stream.peek().span,
        }),
    }
}

fn parse_single_mod_value(&mut self) -> PResult<UiModifierValue> {
    match self.stream.peek().kind {

        TokenKind::Number => {
            let tok = self.stream.next();
            return Ok(UiModifierValue::Number(tok.lexeme.parse().unwrap()));
        }

        TokenKind::String => {
            let tok = self.stream.next();
            return Ok(UiModifierValue::String(tok.lexeme.clone()));
        }

        TokenKind::Ident => {
            let mut key = vec![self.parse_ident()?];

            // allow ONE dotted segment: left.top is allowed
            if self.stream.consume_if(TokenKind::Dot) {
                key.push(self.parse_ident()?);

                // forbid deeper nesting
                if self.stream.peek().kind == TokenKind::Dot {
                    return self.stream.error_here("Too many nested modifier segments");
                }
            }

            // mini-modifier: "left::12"
            if self.stream.consume_if(TokenKind::PathSep) {
                let value = self.parse_modifier_value_rhs()?;
                return Ok(UiModifierValue::MiniMod {
                    key,
                    value: Box::new(value),
                });
            }

            // simple ident
            return Ok(UiModifierValue::Ident(key.remove(0)));
        }

        TokenKind::LBrace => {
            self.stream.next();
            let expr = self.parse_expr()?;
            self.stream.expect(TokenKind::RBrace)?;
            return Ok(UiModifierValue::Expr(expr));
        }

        _ => self.stream.error_here("Expected value inside modifier tuple"),
    }
}

    // ─────────────────────────────────────────────
    //  Interpolated Text:  "Hello {user.name}"
    // ─────────────────────────────────────────────

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


// ─────────────────────────────────────────────
//  UI MODE ENUM
// ─────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
enum UiMode {
    Xml,
    Block,
}
