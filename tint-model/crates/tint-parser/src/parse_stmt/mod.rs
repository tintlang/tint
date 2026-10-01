use crate::{error::*, Parser};
use tint_ast::*;
use tint_lexer::TokenKind;

mod assignment;
mod control_flow;

impl Parser {
    pub(crate) fn parse_let_stmt(&mut self, start: Span) -> PResult<Stmt> {
        let old_in_pattern = self.in_pattern;
        self.in_pattern = true;
        let pattern = self.parse_pattern()?;
        self.in_pattern = old_in_pattern;

        let ty = if self.stream.consume_if(TokenKind::Colon) {
            Some(self.parse_type()?)
        } else {
            None
        };

        let init = if self.stream.consume_if(TokenKind::Eq) {
            LetInit::Assign(self.parse_expr()?)
        } else {
            self.stream.expect(TokenKind::LBrace)?;
            let expr = self.parse_expr()?;
            self.stream.expect(TokenKind::RBrace)?;
            LetInit::Tint(expr)
        };

        self.stream.consume_if(TokenKind::Semicolon);
        let end = init.span();

        Ok(Stmt::Let {
            pattern,
            ty,
            init,
            span: Span::merge(start, end),
        })
    }

    fn can_start_stmt(kind: &TokenKind) -> bool {
        matches!(
            kind,
            TokenKind::Let
                | TokenKind::If
                | TokenKind::While
                | TokenKind::Loop
                | TokenKind::For
                | TokenKind::Return
                | TokenKind::Break
                | TokenKind::Continue
                | TokenKind::Match
                | TokenKind::Borrow
                | TokenKind::Ident
                | TokenKind::Number
                | TokenKind::String
                | TokenKind::True
                | TokenKind::False
                | TokenKind::LParen
                | TokenKind::LBracket
                | TokenKind::LBrace
        )
    }

    pub fn parse_stmt(&mut self) -> PResult<Stmt> {
        let kind = self.stream.peek_kind();

        // Assignment parsing is speculative because identifiers also start expressions.
        let checkpoint = self.stream.checkpoint();
        if let Ok(lhs) = self.parse_lhs_for_assignment() {
            if self.stream.consume_if(TokenKind::Eq) {
                let rhs = self.parse_expr()?;
                self.stream.consume_if(TokenKind::Semicolon);
                let span = Span::merge(lhs.span(), rhs.span());
                return Ok(Stmt::Assign { lhs, rhs, span });
            }
        }
        self.stream.restore(checkpoint);

        match kind {
            TokenKind::Let => {
                let start = self.stream.next().span;
                return self.parse_let_stmt(start);
            }
            TokenKind::Await => {
                return Err(ParserError::Message {
                    msg: "`await` is only allowed at the top level of an `async fn` body".into(),
                    span: self.stream.peek().span,
                });
            }
            TokenKind::Return => return self.parse_return_stmt(),
            TokenKind::If => return self.parse_if_stmt(),
            TokenKind::While => return self.parse_while_stmt(),
            TokenKind::Loop => return self.parse_loop_stmt(),
            TokenKind::Break => return self.parse_break_stmt(),
            TokenKind::Continue => return self.parse_continue_stmt(),
            TokenKind::For => return self.parse_for_stmt(),
            TokenKind::Match => return Ok(Stmt::Expr(self.parse_expr()?)),
            _ => {}
        }

        let expr = self.parse_expr()?;
        let next = self.stream.peek_kind();

        if next == TokenKind::Semicolon {
            self.stream.next();
            return Ok(Stmt::Expr(expr));
        }

        if next == TokenKind::RBrace || Self::can_start_stmt(&next) {
            return Ok(Stmt::Expr(expr));
        }

        Err(ParserError::Message {
            msg: format!("Unexpected token after expression: {:?}", next),
            span: self.stream.peek().span,
        })
    }
}
