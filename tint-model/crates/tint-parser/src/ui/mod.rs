use crate::{error::*, Parser};
use tint_ast::*;
use tint_lexer::TokenKind;

mod block;
mod text;
mod value;

impl Parser {
    pub fn parse_ui_root(&mut self) -> PResult<Vec<UiNode>> {
        let mut nodes = Vec::new();

        self.check_ui_root_start()?;

        while !self.stream.check(TokenKind::RBrace) {
            if self.stream.peek().lexeme == "theme"
                && self.stream.peek2_kind() == TokenKind::PathSep
                && self.stream.peek_n_kind(2) == TokenKind::Ident
                && self.stream.peek_n_kind(3) == TokenKind::LBrace
            {
                nodes.push(self.parse_theme_node()?);
            } else if self.stream.peek().lexeme == "style"
                && self.stream.peek2_kind() == TokenKind::Ident
                && self.stream.peek_n_kind(2) == TokenKind::LBrace
            {
                nodes.push(self.parse_style_node()?);
            } else if self.stream.peek().lexeme == "component"
                && self.stream.peek2_kind() == TokenKind::Ident
                && matches!(self.stream.peek_n_kind(2), TokenKind::LBrace | TokenKind::LParen)
            {
                nodes.push(self.parse_component_node()?);
            } else {
                nodes.push(self.parse_block_node()?);
            }
        }

        Ok(nodes)
    }

    /// Parses a `.tn` source file that is intended to be expanded inside a
    /// `ui fn` body by the source-import loader.  These files deliberately do
    /// not have a `ui fn` wrapper of their own (`Nav { ... }`, `Hero { ... }`,
    /// etc.), so treating them as a normal program produces the misleading
    /// top-level "expected fn/ui/struct/enum" diagnostic.
    pub fn parse_ui_fragment(&mut self) -> PResult<Vec<UiNode>> {
        self.recover_ui_indentation = true;
        let mut nodes = Vec::new();
        self.check_ui_root_start()?;

        while !self.stream.check(TokenKind::Eof) {
            if self.stream.peek().lexeme == "theme"
                && self.stream.peek2_kind() == TokenKind::PathSep
                && self.stream.peek_n_kind(2) == TokenKind::Ident
                && self.stream.peek_n_kind(3) == TokenKind::LBrace
            {
                nodes.push(self.parse_theme_node()?);
            } else if self.stream.peek().lexeme == "style"
                && self.stream.peek2_kind() == TokenKind::Ident
                && self.stream.peek_n_kind(2) == TokenKind::LBrace
            {
                nodes.push(self.parse_style_node()?);
            } else if self.stream.peek().lexeme == "component"
                && self.stream.peek2_kind() == TokenKind::Ident
                && matches!(self.stream.peek_n_kind(2), TokenKind::LBrace | TokenKind::LParen)
            {
                nodes.push(self.parse_component_node()?);
            } else {
                nodes.push(self.parse_block_node()?);
            }
        }

        Ok(nodes)
    }

    /// UI syntax is block-mode only (`Tag { ... }`) -- the earlier XML
    /// dialect (`<Tag ...>...</Tag>`) was removed (see `tint-parser/src/ui/block.rs`'s
    /// `parse_block_case_node` for where its one syntax-unique feature,
    /// `<case label>`, landed in block mode as `case label { ... }`).
    /// A leading `<` gets its own, specific error rather than falling
    /// through to the generic message below, since it's the one mistake
    /// someone coming from the old syntax (or an example predating the
    /// removal) is actually likely to make.
    fn check_ui_root_start(&mut self) -> PResult<()> {
        match self.stream.peek().kind {
            TokenKind::LAngle => Err(ParserError::Message {
                msg: "XML-style UI syntax (<Tag>...</Tag>) has been removed -- \
                      use block syntax instead: Tag { ... }"
                    .into(),
                span: self.stream.peek().span,
            }),
            TokenKind::Ident => {
                if self.stream.peek2_kind() == TokenKind::LBrace
                    || (self.stream.peek().lexeme == "style"
                        && self.stream.peek2_kind() == TokenKind::Ident
                        && self.stream.peek_n_kind(2) == TokenKind::LBrace)
                    || (self.stream.peek().lexeme == "component"
                        && self.stream.peek2_kind() == TokenKind::Ident
                        && matches!(self.stream.peek_n_kind(2), TokenKind::LBrace | TokenKind::LParen))
                    || (self.stream.peek().lexeme == "theme"
                        && self.stream.peek2_kind() == TokenKind::PathSep
                        && self.stream.peek_n_kind(2) == TokenKind::Ident
                        && self.stream.peek_n_kind(3) == TokenKind::LBrace)
                {
                    Ok(())
                } else {
                    Err(ParserError::Message {
                        msg: "Invalid beginning of UI. Expected Tag { }".into(),
                        span: self.stream.peek().span,
                    })
                }
            }
            _ => Err(ParserError::Message {
                msg: "Invalid beginning of UI. Expected Tag { }".into(),
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
            // `trap||true`, `aria_hidden||false`
            TokenKind::True | TokenKind::False => {
                let tok = self.stream.next();
                Ok(UiAttrValue::Ident(if tok.kind == TokenKind::True { "true" } else { "false" }.to_string()))
            }

            TokenKind::String => {
                let tok = self.stream.next();
                Ok(UiAttrValue::Literal(tok.lexeme.clone()))
            }

            // `every||180`: a bare number is kept as its text, like a string.
            TokenKind::Number => {
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
