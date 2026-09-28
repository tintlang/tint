use super::*;

impl Parser {
    pub(crate) fn parse_modifier_value_rhs(&mut self) -> PResult<UiModifierValue> {
        match self.stream.peek().kind {
            TokenKind::Number => {
                let tok = self.stream.next();
                let v: f64 = tok.lexeme.parse().unwrap();
                Ok(UiModifierValue::Number(v))
            }

            TokenKind::String => {
                let tok = self.stream.next();
                Ok(UiModifierValue::String(tok.lexeme.clone()))
            }

            TokenKind::Ident => {
                let tok = self.stream.next();
                Ok(UiModifierValue::Ident(tok.lexeme.clone()))
            }

            // Theme token reference, e.g. `@text-main`. Keep it as an
            // identifier at the AST boundary so token references remain
            // usable anywhere an ordinary UI value is accepted.
            TokenKind::At => {
                self.stream.next();
                let name = self.parse_ident()?;
                Ok(UiModifierValue::Ident(format!("@{name}")))
            }

            TokenKind::LBrace => {
                let checkpoint = self.stream.checkpoint();
                self.stream.expect(TokenKind::LBrace)?;

                // Empty braces represent an empty modifier tuple.
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

                    if self.stream.check(TokenKind::RBrace) {
                        self.stream.next();
                        break;
                    }

                    // Neither a comma (more tuple items follow, e.g.
                    // `{1, #2b3555}`) nor the closing brace -- this isn't
                    // the tuple grammar at all, it's a single computed
                    // expression (`{player_y * 20}`, `{a + b}`, ...).
                    // Rewind and re-parse the whole `{...}` as one
                    // expression instead, the same way `if{}`/`for{}`/
                    // `match{}` already do just below in
                    // `parse_modifier_list_block`. `evaluate_modifier_value`
                    // (tint-runtime/src/ui/style/mod.rs) already knows how
                    // to evaluate a `UiModifierValue::Expr` against live
                    // state for ANY modifier, not just those three -- this
                    // was the only piece actually missing.
                    self.stream.restore(checkpoint);
                    self.stream.expect(TokenKind::LBrace)?;
                    let expr = self.parse_expr()?;
                    self.stream.expect(TokenKind::RBrace)?;
                    return Ok(UiModifierValue::Expr(expr));
                }

                Ok(UiModifierValue::Tuple(items))
            }

            _ => Err(ParserError::Message {
                msg: "Invalid modifier value".into(),
                span: self.stream.peek().span,
            }),
        }
    }

    pub(crate) fn parse_single_mod_value(&mut self) -> PResult<UiModifierValue> {
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

                // Mini-modifier keys allow at most one dotted segment.
                if self.stream.consume_if(TokenKind::Dot) {
                    key.push(self.parse_ident()?);

                    if self.stream.peek().kind == TokenKind::Dot {
                        return self.stream.error_here("Too many nested modifier segments");
                    }
                }

                if self.stream.consume_if(TokenKind::PathSep) {
                    let value = self.parse_modifier_value_rhs()?;
                    return Ok(UiModifierValue::MiniMod {
                        key,
                        value: Box::new(value),
                    });
                }

                return Ok(UiModifierValue::Ident(key.remove(0)));
            }

            TokenKind::At => {
                self.stream.next();
                let name = self.parse_ident()?;
                return Ok(UiModifierValue::Ident(format!("@{name}")));
            }

            TokenKind::LBrace => {
                self.stream.next();
                let expr = self.parse_expr()?;
                self.stream.expect(TokenKind::RBrace)?;
                return Ok(UiModifierValue::Expr(expr));
            }

            _ => self
                .stream
                .error_here("Expected value inside modifier tuple"),
        }
    }
}
