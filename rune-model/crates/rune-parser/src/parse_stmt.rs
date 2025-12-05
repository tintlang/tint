// rune-parser/parse_stmt.rs

use crate::{Parser};
use crate::{ error::*};
use rune_ast::*;
use rune_lexer::TokenKind;

impl Parser {
 /// Parse a let-statement:
    ///   let name = expr;
    ///   let name: Type = expr;
    ///   let name { expr };
    ///   let name: Type { expr };
   pub(crate) fn parse_let_stmt(&mut self, start: Span) -> PResult<Stmt> {
        // Pattern parsing (tuple, struct, ident)
        let old = self.in_pattern;
        self.in_pattern = true;
        let pattern = self.parse_pattern()?;
        self.in_pattern = old;

        // Optional type
        let ty = if self.stream.consume_if(TokenKind::Colon) {
            Some(self.parse_type()?)
        } else {
            None
        };

        // Init (=expr  OR  {expr})
        let init = if self.stream.consume_if(TokenKind::Eq) {
            LetInit::Assign(self.parse_expr()?)
        } else {
            self.stream.expect(TokenKind::LBrace)?;
            let expr = self.parse_expr()?;
            self.stream.expect(TokenKind::RBrace)?;
            LetInit::Rune(expr)
        };

        self.stream.consume_if(TokenKind::Semicolon);

        // ★ FIX: take span BEFORE moving init
        let end_span = init.span();

        Ok(Stmt::Let {
            pattern,
            ty,
            init,              // here init is moved
            span: Span::merge(start, end_span),
        })
    }

    // RETURN statement
    fn parse_return_stmt(&mut self) -> PResult<Stmt> {
        let start = self.stream.next().span; // consume 'return'
        let expr = self.parse_expr()?;
        let end = expr.span();
        self.stream.consume_if(TokenKind::Semicolon);

        Ok(Stmt::Return(expr, Span::merge(start, end)))
    }

    // IF / ELSE
    fn parse_if_stmt(&mut self) -> PResult<Stmt> {
        let start = self.stream.next().span; // consume `if`

        let cond = self.parse_expr()?;
        let then_block = self.parse_block()?;

        let else_block = if self.stream.consume_if(TokenKind::Else) {
            Some(self.parse_block()?)
        } else {
            None
        };

        let end = else_block.as_ref().map(|b| b.span).unwrap_or(then_block.span);

        Ok(Stmt::If {
            cond,
            then: then_block,
            else_: else_block,
            span: Span::merge(start, end),
        })
    }

    // WHILE
    fn parse_while_stmt(&mut self) -> PResult<Stmt> {
        let start = self.stream.next().span;
        let cond = self.parse_expr()?;
        let body = self.parse_block()?;
        let end = body.span;

        Ok(Stmt::Match {
            expr: cond,
            arms: vec![], // временно пусто, чтобы типы прошли
            span: Span::merge(start, end),
        })
    }

    // LOOP (infinite)
    fn parse_loop_stmt(&mut self) -> PResult<Stmt> {
        let start = self.stream.next().span;
        let body = self.parse_block()?;
        let end = body.span;

        Ok(Stmt::For {
            var: "_loop".into(),
            start: Expr::Number("0".into(), start),
            end: Expr::Number("0".into(), end),
            body,
            span: Span::merge(start, end),
        })
    }

    // BREAK / CONTINUE
    fn parse_break_stmt(&mut self) -> PResult<Stmt> {
        let tok = self.stream.next();
        Ok(Stmt::Expr(Expr::Ident("break".into(), tok.span)))
    }

    fn parse_continue_stmt(&mut self) -> PResult<Stmt> {
        let tok = self.stream.next();
        Ok(Stmt::Expr(Expr::Ident("continue".into(), tok.span)))
    }

    // FOR loop
    // for i in 0..10 { ... }
    fn parse_for_stmt(&mut self) -> PResult<Stmt> {
        let start = self.stream.next().span; // "for"

        let var = self.parse_ident()?;

        self.stream.expect(TokenKind::In)?;
        let start_expr = self.parse_expr()?;

        self.stream.expect(TokenKind::DotDot)?; // range
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

    // MAIN SWITCH — parse_stmt()
 pub fn parse_stmt(&mut self) -> PResult<Stmt> {
    let tok = self.stream.peek().clone();

    // ----------------------------------------
    // Universal assignment
    // ----------------------------------------
    {
        let checkpoint = self.stream.checkpoint();

        // parse LHS as postfix-only expression (no binary ops)
        if let Ok(lhs) = self.parse_postfix() {
            if self.stream.consume_if(TokenKind::Eq) {
                let rhs = self.parse_expr()?;
                self.stream.consume_if(TokenKind::Semicolon);

                let lhs_span = lhs.span();
                let rhs_span = rhs.span();
                let span = Span::merge(lhs_span, rhs_span);

                return Ok(Stmt::Assign { lhs, rhs, span });
            }
        }

        self.stream.restore(checkpoint);
    }

    match tok.kind {
        TokenKind::Let => {
            let start = self.stream.next().span;
            return self.parse_let_stmt(start);
        }

        TokenKind::Return => return self.parse_return_stmt(),
        TokenKind::If => return self.parse_if_stmt(),
        TokenKind::Loop => return self.parse_loop_stmt(),
        TokenKind::While => return self.parse_while_stmt(),
        TokenKind::Break => return self.parse_break_stmt(),
        TokenKind::Continue => return self.parse_continue_stmt(),

        TokenKind::Match => {
            let expr = self.parse_expr()?;
            self.stream.consume_if(TokenKind::Semicolon);
            return Ok(Stmt::Expr(expr));
        }

        TokenKind::For => return self.parse_for_stmt(),

        // Default: expression statement
        _ => {
            let expr = self.parse_expr()?;
            self.stream.consume_if(TokenKind::Semicolon);
            return Ok(Stmt::Expr(expr));
        }
    }
}

}
