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

    // ASSIGNMENT
    // x = y
    // x += y
    fn parse_assignment(&mut self) -> PResult<Stmt> {
        let name = self.parse_ident()?;

        let op_tok = self.stream.next_owned(); // =, +=, -= ...

        let expr = self.parse_expr()?;
        let end = expr.span();

        Ok(Stmt::Assign {
            name,
            expr,
            span: Span::merge(op_tok.span, end),
        })
    }

    // MAIN SWITCH — parse_stmt()
    pub fn parse_stmt(&mut self) -> PResult<Stmt> {
        let tok = self.stream.peek().clone();

        match tok.kind {
            // let
            TokenKind::Let => {
                let start = self.stream.next().span; // consume 'let'
                self.parse_let_stmt(start)
            }
            
            // return
            TokenKind::Return => self.parse_return_stmt(),

            // if / else
            TokenKind::If => self.parse_if_stmt(),

            // loop
            TokenKind::Loop => self.parse_loop_stmt(),

            // while
            TokenKind::While => self.parse_while_stmt(),

            // break / continue
            TokenKind::Break => self.parse_break_stmt(),
            TokenKind::Continue => self.parse_continue_stmt(),

            TokenKind::Match => {
                let expr = self.parse_expr()?;  // match — это expression
                self.stream.consume_if(TokenKind::Semicolon);
                Ok(Stmt::Expr(expr))
            },

            // for
            TokenKind::For => self.parse_for_stmt(),

            // assignment: Ident '=' ...
            TokenKind::Ident => {
                // Look ahead
                if self.stream.peek_n(1).kind == TokenKind::Eq
                    || self.stream.peek_n(1).kind == TokenKind::PlusEq
                    || self.stream.peek_n(1).kind == TokenKind::MinusEq
                    || self.stream.peek_n(1).kind == TokenKind::StarEq
                    || self.stream.peek_n(1).kind == TokenKind::SlashEq
                {
                    return self.parse_assignment();
                }

                // Otherwise -> expression
                let expr = self.parse_expr()?;
                self.stream.consume_if(TokenKind::Semicolon);
                Ok(Stmt::Expr(expr))
            }

            // Fallback → expression
            TokenKind::Number
            | TokenKind::String
            | TokenKind::True
            | TokenKind::False
            | TokenKind::Ident
            | TokenKind::LParen
            | TokenKind::LBracket
            | TokenKind::Bang
            | TokenKind::Minus
            | TokenKind::PipeLambda
            => {
                let expr = self.parse_expr()?;
                self.stream.consume_if(TokenKind::Semicolon);
                Ok(Stmt::Expr(expr))
            }

            _ => Err(ParserError::Message {
                msg: format!("Unexpected token in statement: {:?}", tok.kind),
                span: tok.span,
            }),
        }
    }
}
