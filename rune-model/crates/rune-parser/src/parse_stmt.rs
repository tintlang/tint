// rune-parser/parse_stmt.rs

use crate::{Parser};
use crate::{ error::*};
use rune_ast::*;
use rune_lexer::TokenKind;

impl Parser {

    // ────────────────────────────────────────────────
    // LET statement
    // ────────────────────────────────────────────────
    pub(crate) fn parse_let_stmt(&mut self, start: Span) -> PResult<Stmt> {
        let old = self.in_pattern;
        self.in_pattern = true;
        let pattern = self.parse_pattern()?;
        self.in_pattern = old;

        // optional type
        let ty = if self.stream.consume_if(TokenKind::Colon) {
            Some(self.parse_type()?)
        } else {
            None
        };

        // init
        let init = if self.stream.consume_if(TokenKind::Eq) {
            LetInit::Assign(self.parse_expr()?)
        } else {
            self.stream.expect(TokenKind::LBrace)?;
            let expr = self.parse_expr()?;
            self.stream.expect(TokenKind::RBrace)?;
            LetInit::Rune(expr)
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

    // ────────────────────────────────────────────────
    // RETURN
    // ────────────────────────────────────────────────
    fn parse_return_stmt(&mut self) -> PResult<Stmt> {
        let start = self.stream.next().span; // 'return'

        let expr = self.parse_expr()?;
        let end = expr.span();            

        self.stream.consume_if(TokenKind::Semicolon);

        Ok(Stmt::Return(expr, Span::merge(start, end)))
    }

    // ────────────────────────────────────────────────
    // IF / ELSE
    // ────────────────────────────────────────────────
    fn parse_if_stmt(&mut self) -> PResult<Stmt> {
        let start = self.stream.next().span;

        let cond = self.parse_expr()?;
        let then_block = self.parse_block()?;

        let else_block = if self.stream.consume_if(TokenKind::Else) {
            Some(self.parse_block()?)
        } else {
            None
        };

        let end = else_block
            .as_ref()
            .map(|b| b.span)
            .unwrap_or(then_block.span);

        Ok(Stmt::If {
            cond,
            then: then_block,
            else_: else_block,
            span: Span::merge(start, end),
        })
    }

    // ────────────────────────────────────────────────
    // WHILE
    // ────────────────────────────────────────────────
    fn parse_while_stmt(&mut self) -> PResult<Stmt> {
        let start = self.stream.next().span;
        let cond = self.parse_expr()?;
        let body = self.parse_block()?;

        let end = body.span;     

        Ok(Stmt::While {
            cond,
            body,                
            span: Span::merge(start, end),
        })
}

    // ────────────────────────────────────────────────
    // LOOP
    // ────────────────────────────────────────────────
    fn parse_loop_stmt(&mut self) -> PResult<Stmt> {
        let start = self.stream.next().span;
        let body = self.parse_block()?;

        let end = body.span;  

        Ok(Stmt::Loop {
            body,
            span: Span::merge(start, end),
        })
    }

    // ────────────────────────────────────────────────
    // BREAK / CONTINUE
    // ────────────────────────────────────────────────
    fn parse_break_stmt(&mut self) -> PResult<Stmt> {
        let tok = self.stream.next();
        Ok(Stmt::Break(tok.span))
    }

    fn parse_continue_stmt(&mut self) -> PResult<Stmt> {
        let tok = self.stream.next();
        Ok(Stmt::Continue(tok.span))
    }

    // ────────────────────────────────────────────────
    // FOR i in a..b
    // ────────────────────────────────────────────────
    fn parse_for_stmt(&mut self) -> PResult<Stmt> {
        let start = self.stream.next().span; // for

        let var = self.parse_ident()?;
        self.stream.expect(TokenKind::In)?;

        let start_expr = self.parse_expr()?;

        self.stream.expect(TokenKind::DotDot)?;
        let end_expr = self.parse_expr()?;

        let body = self.parse_block()?;

        let end = body.span;

        Ok(Stmt::For {
            var,
            start: start_expr,
            end: end_expr,
            body,
            span: Span::merge(start, end),
        })
    }

    // ────────────────────────────────────────────────
    // LHS for assignment:  a, a.b, a[x]
    // ────────────────────────────────────────────────
    fn parse_lhs_for_assignment(&mut self) -> PResult<Expr> {
        let start = self.stream.peek().span;

        let mut expr = if let TokenKind::Ident = self.stream.peek().kind {
            let name = self.stream.next().lexeme.clone();
            Expr::Ident(name, start)
        } else {
            return Err(ParserError::Message {
                msg: "Expected identifier for assignment".into(),
                span: start,
            });
        };

        loop {
            if self.stream.consume_if(TokenKind::Dot) {
                let field = self.parse_ident()?;
                let span = Span::merge(start, self.stream.last_span());
                expr = Expr::Field { target: Box::new(expr), field, span };
                continue;
            }

            if self.stream.consume_if(TokenKind::LBracket) {
                let idx = self.parse_expr()?;
                self.stream.expect(TokenKind::RBracket)?;
                let span = Span::merge(start, idx.span());
                expr = Expr::Index { target: Box::new(expr), index: Box::new(idx), span };
                continue;
            }

            break;
        }

        Ok(expr)
    }

    // ────────────────────────────────────────────────
    // MAIN: parse_stmt()
    // ────────────────────────────────────────────────
    pub fn parse_stmt(&mut self) -> PResult<Stmt> {
        let tok = self.stream.peek().clone();

        // ─────────────────────────────────────
        // Try assignment
        // ─────────────────────────────────────
        {
            let checkpoint = self.stream.checkpoint();

            if let Ok(lhs) = self.parse_lhs_for_assignment() {
                if self.stream.consume_if(TokenKind::Eq) {
                    let rhs = self.parse_expr()?;
                    self.stream.consume_if(TokenKind::Semicolon);

                    // FIX: Get spans BEFORE moving lhs/rhs
                    let lhs_span = lhs.span();
                    let rhs_span = rhs.span();
                    let span = Span::merge(lhs_span, rhs_span);

                    return Ok(Stmt::Assign {
                        lhs,
                        rhs,
                        span,
                    });
                }
            }

            self.stream.restore(checkpoint);
        }

        // ─────────────────────────────────────
        // Let / return / if / while / for ...
        // ─────────────────────────────────────
        match tok.kind {
            TokenKind::Let => {
                let start = self.stream.next().span;
                return self.parse_let_stmt(start);
            }

            TokenKind::Return => return self.parse_return_stmt(),
            TokenKind::If => return self.parse_if_stmt(),
            TokenKind::While => return self.parse_while_stmt(),
            TokenKind::Loop => return self.parse_loop_stmt(),
            TokenKind::Break => return self.parse_break_stmt(),
            TokenKind::Continue => return self.parse_continue_stmt(),
            TokenKind::For => return self.parse_for_stmt(),
            TokenKind::Match => {
                let expr = self.parse_expr()?;
                return Ok(Stmt::Expr(expr));
            }

            _ => {}
        }

        // DEFAULT = Expression statement
        let expr = self.parse_expr()?;
        self.stream.consume_if(TokenKind::Semicolon);
        Ok(Stmt::Expr(expr))
    }
}
