use super::*;

impl Parser {
    /// `-72` in a modifier value: a minus sign directly followed by a number.
    fn parse_negative_number(&mut self) -> Option<UiModifierValue> {
        if self.stream.peek().kind != TokenKind::Minus {
            return None;
        }
        let checkpoint = self.stream.checkpoint();
        self.stream.next();
        if self.stream.peek().kind == TokenKind::Number {
            let tok = self.stream.next();
            let v: f64 = tok.lexeme.parse().unwrap();
            return Some(UiModifierValue::Number(-v));
        }
        self.stream.restore(checkpoint);
        None
    }

    /// `16..44` after a number: a range (fluid sizes, `text::{16..44}`).
    fn number_or_range(&mut self, first: f64) -> UiModifierValue {
        if self.stream.peek().kind == TokenKind::DotDot && self.stream.peek2_kind() == TokenKind::Number {
            self.stream.next();
            let second: f64 = self.stream.next().lexeme.parse().unwrap();
            return UiModifierValue::Range(first, second);
        }
        UiModifierValue::Number(first)
    }

    pub(crate) fn parse_modifier_value_rhs(&mut self) -> PResult<UiModifierValue> {
        if let Some(value) = self.parse_negative_number() {
            return Ok(value);
        }
        match self.stream.peek().kind {
            TokenKind::Number => {
                let tok = self.stream.next();
                let v: f64 = tok.lexeme.parse().unwrap();
                Ok(self.number_or_range(v))
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
                    let val = match self.parse_single_mod_value() {
                        Ok(v) => v,
                        Err(_) => {
                            // Not tuple grammar (`{r.id}`): a single expression.
                            self.stream.restore(checkpoint);
                            self.stream.expect(TokenKind::LBrace)?;
                            let expr = self.parse_expr()?;
                            self.stream.expect(TokenKind::RBrace)?;
                            return Ok(UiModifierValue::Expr(expr));
                        }
                    };
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
        if let Some(value) = self.parse_negative_number() {
            return Ok(value);
        }
        match self.stream.peek().kind {
            TokenKind::Number => {
                let tok = self.stream.next();
                let v: f64 = tok.lexeme.parse().unwrap();
                return Ok(self.number_or_range(v));
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

                if key.len() > 1 {
                    return self.stream.error_here("A dotted name is only a mini-modifier key (`a.b::value`)");
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
