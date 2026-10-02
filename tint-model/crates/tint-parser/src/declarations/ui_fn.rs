use crate::error::*;
use crate::Parser;
use tint_ast::*;
use tint_lexer::TokenKind;

impl Parser {
    /// Parse:  ui fn Name(params...) {  <UI> / BlockUI  }
    pub(crate) fn parse_ui_fn(&mut self) -> PResult<UiFnDecl> {
        // "ui"
        let start = self.stream.expect(TokenKind::Ui)?.span;

        // "fn"
        self.stream.expect(TokenKind::Fn)?;

        // optional attributes: ui fn [@(strict, speed)]
        let mut local_attrs = AttributeList::empty();

        if self.stream.peek_kind() == TokenKind::LBracket {
            let a = self.parse_attributes()?;
            local_attrs.extend(a);

            if self.stream.peek_kind() == TokenKind::LBracket {
                return Err(ParserError::Message {
                    msg: "Only one attribute block allowed after `ui fn`".into(),
                    span: self.stream.peek().span,
                });
            }
        }

        // NAME
        let name = self.parse_ident()?;

        // PARAMS (...)
        self.stream.expect(TokenKind::LParen)?;
        let params = self.parse_params()?;
        self.stream.expect(TokenKind::RParen)?;

        // BODY ENTRY
        self.stream.expect(TokenKind::LBrace)?;

        // `state <ident> = <expr>` lines, if any, come first -- before
        // any UI node. This is the only place `state` is currently
        // parsed at all (it's a reserved keyword with no other grammar
        // yet); see UiStateDecl's doc comment in tint-ast for what these
        // become at runtime.
        let mut state = Vec::new();
        while self.stream.peek().kind == TokenKind::State || self.at_derived() || self.at_persist() {
            let persist = self.at_persist();
            if persist {
                self.stream.next();
            }
            let tok = self.stream.next();
            let decl_start = tok.span;
            let derived = tok.kind != TokenKind::State && !persist;

            let name = self.parse_ident()?;
            self.stream.expect(TokenKind::Eq)?;
            let init = self.parse_expr()?;

            state.push(UiStateDecl {
                name,
                init,
                derived,
                persist,
                span: Span::merge(decl_start, self.stream.last_span()),
            });
        }

        // Parse all UI roots using one syntax mode per block.
        let body_nodes = self.parse_ui_root()?;

        // END
        let end = self.stream.expect(TokenKind::RBrace)?.span;

        Ok(UiFnDecl {
            attributes: local_attrs,
            name,
            params,
            state,
            body: body_nodes,
            span: Span::merge(start, end),
        })
    }
}
