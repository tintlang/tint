use super::*;

impl Parser {
pub fn parse_xml_node(&mut self) -> PResult<UiNode> {
    let start = self.stream.expect(TokenKind::LAngle)?.span;
    let name = self.parse_ident()?;

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


    pub(crate) fn parse_xml_children(&mut self) -> PResult<Vec<UiNodeOrExpr>> {
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

    // Attributes use `||`; modifiers use dotted paths followed by `::`.
pub(crate) fn parse_modifier_list_xml(
    &mut self
) -> PResult<(Vec<UiAttribute>, Vec<UiModifier>)>
{
    let mut attrs = Vec::new();
    let mut mods  = Vec::new();

    while self.stream.peek().kind == TokenKind::Ident {
        let tok = self.stream.next();
        let name = tok.lexeme.clone();
        let span = tok.span;

        if self.stream.peek().kind == TokenKind::OrOr {
            self.stream.next();
            let value = self.parse_attribute_rhs()?;

            attrs.push(UiAttribute {
                name,
                value,
                span,
            });

            continue;
        }

        let mut path = vec![name];

        while self.stream.peek().kind == TokenKind::Dot {
            self.stream.next();
            let seg = self.parse_ident()?;
            path.push(seg);
        }

        if self.stream.peek().kind == TokenKind::PathSep {
            self.stream.next();

            let value = self.parse_modifier_value_rhs()?;

            mods.push(UiModifier {
                path,
                value,
                span,
            });

            continue;
        }

        // Stop when the identifier is neither an attribute nor a modifier.
        break;
    }
    // Control-flow modifiers are parsed after regular attributes and modifiers.
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

                // The loop variable is bound per-iteration by the builder
                // (see rune-runtime/src/ui/builder.rs) -- it's no longer
                // just parsed-then-discarded, it travels with the iterable
                // as a MiniMod{key: [var], value: Expr(iterable)}.
                let var = self.parse_ident()?;

                self.stream.expect(TokenKind::In)?;

                let expr = self.parse_expr()?;

                self.stream.expect(TokenKind::RBrace)?;

                mods.push(UiModifier {
                    path: vec!["for".to_string()],
                    value: UiModifierValue::MiniMod {
                        key: vec![var],
                        value: Box::new(UiModifierValue::Expr(expr)),
                    },
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
}
