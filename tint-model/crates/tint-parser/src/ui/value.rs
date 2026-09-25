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

        TokenKind::LBrace => {
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

        TokenKind::LBrace => {
            self.stream.next();
            let expr = self.parse_expr()?;
            self.stream.expect(TokenKind::RBrace)?;
            return Ok(UiModifierValue::Expr(expr));
        }

        _ => self.stream.error_here("Expected value inside modifier tuple"),
    }
}
}
