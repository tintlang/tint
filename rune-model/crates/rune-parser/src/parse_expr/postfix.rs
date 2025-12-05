use crate::{Parser, error::*};
use rune_ast::{Expr, Span};
use rune_lexer::TokenKind;

impl Parser {
    fn parse_fn_call_arg(&mut self) -> PResult<Expr> {
        // named arg?   name { expr }
        if self.stream.peek_kind() == TokenKind::Ident
            && self.stream.peek2_kind() == TokenKind::LBrace
        {
            // name
            let name = self.parse_ident()?;
            let start = self.stream.last_span();

            // {
            self.stream.expect(TokenKind::LBrace)?;
            let value = self.parse_expr()?;
            self.stream.expect(TokenKind::RBrace)?;
            let end = self.stream.last_span();

            return Ok(Expr::NamedArg {
                name,
                value: Box::new(value),
                span: Span::merge(start, end),
            });
        }

        // otherwise normal expression
        self.parse_expr()
    }
    
    pub(crate) fn parse_postfix(&mut self) -> PResult<Expr> {
    if self.in_pattern {
        eprintln!(
            "[POSTFIX] ERROR: postfix inside pattern at {:?}, token={:?}",
            self.stream.peek().span,
            self.stream.peek_kind()
        );
        return Err(ParserError::Message {
            msg: "postfix not allowed in pattern mode".into(),
            span: self.stream.peek().span,
        });
    }

    eprintln!(
        "[POSTFIX] primary start: token={:?} '{}' at {:?}",
        self.stream.peek_kind(),
        self.stream.peek().lexeme,
        self.stream.peek().span
    );

    let mut expr = self.parse_primary()?;

    loop {
        let tok = self.stream.peek().clone();
        eprintln!(
            "[POSTFIX] loop: token={:?} '{}' at {:?}",
            tok.kind,
            tok.lexeme,
            tok.span
        );

        match tok.kind {
            // -------- FUNCTION CALL --------
            TokenKind::LParen => {
                let start_span = expr.span();
                self.stream.next(); // '('
                eprintln!("[POSTFIX] start call");

                let mut args = Vec::new();

                if !self.stream.consume_if(TokenKind::RParen) {
                    loop {
                        let arg = self.parse_fn_call_arg()?;
                        args.push(arg);

                        if !self.stream.consume_if(TokenKind::Comma) {
                            break;
                        }
                    }
                    self.stream.expect(TokenKind::RParen)?;
                }

                let end_span = self.stream.last_span();
                expr = Expr::Call {
                    target: Box::new(expr),
                    args,
                    span: Span::merge(start_span, end_span),
                };

                eprintln!("[POSTFIX] parsed call OK");
            }

            
// -------- FIELD ACCESS OR TUPLE INDEX --------
TokenKind::Dot => {
    let dot = self.stream.next(); // '.'
    eprintln!("[POSTFIX] start field or tuple index");

    match self.stream.peek_kind() {

        // tuple index: v.0, v.1, v.2, ...
        TokenKind::Number => {
            let num_tok = self.stream.next_owned();
            let index: usize = num_tok.lexeme.parse().unwrap();

            let span = Span::merge(expr.span(), num_tok.span);
            expr = Expr::TupleIndex {
                target: Box::new(expr),
                index,
                span,
            };

            eprintln!("[POSTFIX] parsed tuple index .{}", index);
        }

        // normal field: v.x
        TokenKind::Ident => {
            let field = self.parse_ident()?;

            let span = Span::merge(expr.span(), dot.span);
            expr = Expr::Field {
                target: Box::new(expr),
                field: field.clone(),
                span,
            };

            eprintln!("[POSTFIX] parsed field '{}'", field);
        }

        other => {
            return Err(ParserError::Message {
                msg: format!("Expected field or tuple index after '.', found {:?}", other),
                span: self.stream.peek().span,
            });
        }
    }
    
}

            // -------- NAMESPACE ACCESS --------
            TokenKind::PathSep => {
                let sep = self.stream.next(); // '::'
                eprintln!("[POSTFIX] start namespace");

                let item = self.parse_ident()?;
                let span = Span::merge(expr.span(), sep.span);

                expr = Expr::Namespace {
                    base: Box::new(expr),
                    item: item.clone(),
                    span,
                };

                eprintln!("[POSTFIX] parsed namespace '{}'", item);
            }

            // -------- INDEXING --------
            TokenKind::LBracket => {
                let start = self.stream.next().span; // '['
                eprintln!("[POSTFIX] start index");

                let index_expr = self.parse_expr()?;
                self.stream.expect(TokenKind::RBracket)?;

                let span = Span::merge(expr.span(), index_expr.span());
                expr = Expr::Index {
                    target: Box::new(expr),
                    index: Box::new(index_expr),
                    span,
                };

                eprintln!("[POSTFIX] parsed index");
            }

            _ => break,
        }
    }

    eprintln!(
        "[POSTFIX] exit: next token={:?} '{}' at {:?}",
        self.stream.peek_kind(),
        self.stream.peek().lexeme,
        self.stream.peek().span
    );

    Ok(expr)
}
}
