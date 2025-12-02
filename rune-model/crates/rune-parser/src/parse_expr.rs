// rune-parser/parse_expr.rs

use rune_lexer::TokenKind;
use rune_ast::{Expr, Span};
use crate::{Parser, error::*};

impl Parser {
    // ENTRY POINT
    pub fn parse_expr(&mut self) -> PResult<Expr> {
        self.parse_binary_expr(0)
    }

    // PRIMARY EXPRESSIONS
    // number, string, ident, paren, lambda
    fn parse_primary(&mut self) -> PResult<Expr> {
        let tok = self.stream.peek().clone();

        match tok.kind {
            TokenKind::Number => {
                let t = self.stream.next();
                Ok(Expr::Number(t.lexeme, t.span))
            }

            TokenKind::String => {
                let t = self.stream.next();
                Ok(Expr::String(t.lexeme, t.span))
            }

            TokenKind::Ident => {
                let t = self.stream.next();
                Ok(Expr::Ident(t.lexeme, t.span))
            }

            // λ LAMBDA: |x| expr
            TokenKind::PipeLambda => {
                let start = self.stream.next().span;

                // parse params: |x,y|
                let mut params = Vec::new();
                loop {
                    let name = self.parse_ident()?;
                    params.push(name);

                    if !self.stream.consume_if(TokenKind::Comma) {
                        break;
                    }
                }

                // expect closing |
                self.stream.expect(TokenKind::PipeLambda)?;
                let body = self.parse_expr()?;
                let span = Span::merge(start, body.span());

                Ok(Expr::Lambda { params, body: Box::new(body), span })
            }

            // Parentheses
            TokenKind::LParen => {
                let open = self.stream.next().span;
                let inner = self.parse_expr()?;
                self.stream.expect(TokenKind::RParen)?;
                Ok(Expr::Paren(Box::new(inner), open))
            }

            // Unary ops: !expr, -expr
            TokenKind::Bang | TokenKind::Minus => {
                let op = self.stream.next();
                let right = self.parse_primary()?;
                let span = Span::merge(op.span, right.span());

                Ok(Expr::Unary {
                    op: op.lexeme,
                    expr: Box::new(right),
                    span,
                })
            }

            _ => Err(ParserError::Message {
                msg: format!("Unexpected token in expression: {:?}", tok.kind),
                span: tok.span,
            }),
        }
    }

    // CALLS, FIELD ACCESS, NAMESPACE ACCESS, INDEXING
    // After parsing primary: foo(), a.b, a::b, a[b]
    fn parse_postfix(&mut self) -> PResult<Expr> {
        let mut expr = self.parse_primary()?;

        loop {
            let tok = self.stream.peek();

            match tok.kind {
                // foo(expr, expr)
                TokenKind::LParen => {
                    let span_start = expr.span();
                    self.stream.next(); // consume '('
                    let mut args = Vec::new();

                    if !self.stream.consume_if(TokenKind::RParen) {
                        loop {
                            let arg = self.parse_expr()?;
                            args.push(arg);
                            if !self.stream.consume_if(TokenKind::Comma) {
                                break;
                            }
                        }
                        self.stream.expect(TokenKind::RParen)?;
                    }

                    let span_end = self.stream.last_span();
                    expr = Expr::Call {
                        target: Box::new(expr),
                        args,
                        span: Span::merge(span_start, span_end),
                    };
                }

                // FIELD:  obj.field
                TokenKind::Dot => {
                    let dot = self.stream.next();
                    let field = self.parse_ident()?;
                    let span = Span::merge(expr.span(), dot.span);

                    expr = Expr::Field {
                        target: Box::new(expr),
                        field,
                        span,
                    };
                }

                // NAMESPACE:  math::vec::dot
                TokenKind::PathSep => {
                    let sep = self.stream.next();
                    let item = self.parse_ident()?;
                    let span = Span::merge(expr.span(), sep.span);

                    expr = Expr::Namespace {
                        base: Box::new(expr),
                        item,
                        span,
                    };
                }

                // INDEX:  arr[expr]
                TokenKind::LBracket => {
                    let start = self.stream.next().span;
                    let index_expr = self.parse_expr()?;
                    self.stream.expect(TokenKind::RBracket)?;
                    let span = Span::merge(expr.span(), index_expr.span());

                    expr = Expr::Index {
                        target: Box::new(expr),
                        index: Box::new(index_expr),
                        span,
                    };
                }

                _ => break,
            }
        }

        Ok(expr)
    }

    // Pratt parser for binary operators
    fn parse_binary_expr(&mut self, min_prec: u8) -> PResult<Expr> {
        let mut left = self.parse_postfix()?;

        loop {
            let op_tok = self.stream.peek().clone();
            let prec = op_tok.kind.binary_precedence();

            if prec < min_prec || prec == 0 {
                break;
            }

            let op = self.stream.next(); // consume operator
            let right = self.parse_binary_expr(prec + 1)?;
            let span = Span::merge(left.span(), right.span());

            left = Expr::Binary {
                left: Box::new(left),
                op: op.lexeme,
                right: Box::new(right),
                span,
            };
        }

        Ok(left)
    }
}
