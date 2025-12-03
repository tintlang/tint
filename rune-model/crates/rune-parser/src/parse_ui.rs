// rune-parser/parse_ui.rs

use crate::Parser;
use crate::error::*;
use rune_ast::*;
use rune_lexer::TokenKind;

impl Parser {
    /// Parse:  ui fn Name(params...) { <UI> }
    pub(crate) fn parse_ui_fn(&mut self) -> PResult<UiFnDecl> {
        // ui
        let start_tok = self.stream.expect(TokenKind::Ui)?;
        // fn
        self.stream.expect(TokenKind::Fn)?;

        // fn name
        let name = self.parse_ident()?;

        // parameters (...)
        self.stream.expect(TokenKind::LParen)?;
        let params = self.parse_params()?;        // ✔ now supported
        self.stream.expect(TokenKind::RParen)?;

        // "{"
        self.stream.expect(TokenKind::LBrace)?;

        // Parse exactly ONE top-level UI node
        let body = self.parse_ui_node()?;         // ✔ handled in ui.rs

        // "}"
        let end_tok = self.stream.expect(TokenKind::RBrace)?;

        Ok(UiFnDecl {
            name,
            params,
            body,
            span: Span::merge(start_tok.span, end_tok.span),
        })
    }

// ============================================================
// VALUE PARSER for MODIFIERS
// Supports:
//   10
//   blue
//   a + b
//   { expr }
// ============================================================

fn try_parse_value(&mut self) -> PResult<Option<UiModifierValue>> {
    let kind = self.stream.peek().kind.clone();

    match kind {
        TokenKind::Number => {
            let tok = self.stream.next();
            let v: f64 = tok.lexeme.parse().unwrap();
            return Ok(Some(UiModifierValue::Number(v)));
        }

        TokenKind::Ident => {
            let tok = self.stream.next();
            return Ok(Some(UiModifierValue::String(tok.lexeme.clone())));
        }

        // { expr }
        TokenKind::LBrace => {
            self.stream.next(); // {
            let expr = self.parse_expr()?;
            self.stream.expect(TokenKind::RBrace)?;
            return Ok(Some(UiModifierValue::Expr(expr)));
        }

        _ => Ok(None),
    }
}

// ============================================================
// Parse ONE UI modifier: 
//    padding{12}
//    padding.x{ left: 10 right: 20 }
//    animate.opacity{ from: 0 to: 1 }
// ============================================================

}
