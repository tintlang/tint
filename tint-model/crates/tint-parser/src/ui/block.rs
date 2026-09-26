use super::*;

impl Parser {
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

    pub(crate) fn parse_block_children(&mut self) -> PResult<Vec<UiNodeOrExpr>> {
        let mut out = Vec::new();

        loop {
            match self.stream.peek().kind {
                TokenKind::RBrace => break,
                TokenKind::Ident
                    if self.stream.peek2_kind() == TokenKind::PathSep
                        && self.stream.peek_n_kind(2) == TokenKind::Ident
                        && self.stream.peek_n_kind(3) == TokenKind::LBrace
                        && self.stream.peek().lexeme == "theme" =>
                {
                    out.push(UiNodeOrExpr::Node(self.parse_theme_node()?));
                }
                TokenKind::Ident if self.stream.peek2_kind() == TokenKind::LBrace => {
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

    pub(crate) fn parse_modifier_list_block(
        &mut self,
    ) -> PResult<(Vec<UiAttribute>, Vec<UiModifier>)> {
        let mut attrs = Vec::new();
        let mut mods = Vec::new();

        while self.stream.peek().kind == TokenKind::Ident {
            // `theme::dark { ... }` is a themed child block, not the
            // `theme::dark` modifier followed by an unexpected brace.
            if self.stream.peek().lexeme == "theme"
                && self.stream.peek2_kind() == TokenKind::PathSep
                && self.stream.peek_n_kind(2) == TokenKind::Ident
                && self.stream.peek_n_kind(3) == TokenKind::LBrace
            {
                break;
            }

            // `Ident {` starts a child node, so stop modifier parsing before it.
            if self.stream.peek2_kind() == TokenKind::LBrace {
                break;
            }

            let tok = self.stream.next();
            let name = tok.lexeme.clone();
            let span = tok.span;

            if self.stream.peek().kind == TokenKind::OrOr {
                self.stream.next();
                let value = self.parse_attribute_rhs()?;

                attrs.push(UiAttribute { name, value, span });

                continue;
            }

            let mut path = vec![name];

            while self.stream.peek().kind == TokenKind::Dot {
                self.stream.next(); // consume '.'
                let seg = self.parse_ident()?;
                path.push(seg);
            }

            if self.stream.peek().kind == TokenKind::PathSep {
                self.stream.next();

                let value = self.parse_modifier_value_rhs()?;

                mods.push(UiModifier { path, value, span });

                continue;
            }

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
                    // (see tint-runtime/src/ui/builder.rs) -- it's no longer
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
    pub(crate) fn parse_theme_node(&mut self) -> PResult<UiNode> {
        let start = self.stream.expect(TokenKind::Ident)?.span;
        self.stream.expect(TokenKind::PathSep)?;
        let name = self.stream.expect(TokenKind::Ident)?.lexeme;
        self.stream.expect(TokenKind::LBrace)?;
        let children = self.parse_block_children()?;
        let end = self.stream.expect(TokenKind::RBrace)?.span;
        Ok(UiNode::Theme {
            name,
            children,
            span: Span::merge(start, end),
        })
    }
}
