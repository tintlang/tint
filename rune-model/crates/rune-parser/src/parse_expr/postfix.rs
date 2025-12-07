// rune-parser/parse_expr/postfix.rs
// RUNELANG POSTFIX PARSER 

use crate::{Parser, error::*};
use rune_ast::{Expr, Span};
use rune_lexer::TokenKind;

impl Parser {

    // Top-level postfix entry
    pub(crate) fn parse_postfix(&mut self) -> PResult<Expr> {
        let primary = self.parse_primary()?;
        self.parse_postfix_with(primary)
    }

    pub(crate) fn try_parse_gpu(&mut self, base: &Expr) -> PResult<Option<Expr>> {
        // save stream position BEFORE trying to parse .gpu(...)
        let checkpoint = self.stream.checkpoint();

        // must start with '.'
        if !self.stream.consume_if(TokenKind::Dot) {
            return Ok(None);
        }

        // must be ident 'gpu'
        if self.stream.peek().lexeme != "gpu" {
            self.stream.restore(checkpoint);
            return Ok(None);
        }

        let gpu_tok = self.stream.next(); // consume 'gpu'
        let gpu_span = gpu_tok.span;

        // must have '('
        if !self.stream.consume_if(TokenKind::LParen) {
            self.stream.restore(checkpoint);
            return Ok(None);
        }

        // parse args: kernel + user args
        let mut args = vec![base.clone()];

        if !self.stream.consume_if(TokenKind::RParen) {
            loop {
                args.push(self.parse_expr()?);

                if !self.stream.consume_if(TokenKind::Comma) {
                    break;
                }
            }
            self.stream.expect(TokenKind::RParen)?;
        }

        let span = Span::merge(base.span(), self.stream.last_span());

        Ok(Some(Expr::Call {
            target: Box::new(Expr::Ident("gpu$call".into(), gpu_span)),
            args,
            span,
        }))
    }

    /// Main postfix reducer: repeatedly applies suffix operators
    pub(crate) fn parse_postfix_with(&mut self, mut expr: Expr) -> PResult<Expr> {
        if self.in_pattern {
            return Ok(expr); // postfix запрещён в паттернах
        }

        loop {

             // 1) SPECIAL POSTFIX: .gpu(...)
            if let Some(new_expr) = self.try_parse_gpu(&expr)? {
                expr = new_expr;
                continue;
            }

            match self.stream.peek_kind() {

                // FUNCTION CALL
                TokenKind::LParen => {
                    expr = self.parse_call(expr)?;
                    continue;
                }

                // FIELD ACCESS & TUPLE INDEX
                TokenKind::Dot => {
                    expr = self.parse_field_or_tuple(expr)?;
                    continue;
                }

                // NAMESPACE ( :: ) 
                TokenKind::PathSep => {
                    expr = self.parse_namespace(expr)?;
                    continue;
                }

                // INDEXING: expr[ index ] 
                TokenKind::LBracket => {
                    expr = self.parse_index(expr)?;
                    continue;
                }

                //  NAMED CALL: func { ... } 
                TokenKind::LBrace => {
                    // Only allow if expr is IDENT and registered as function
                    if let Expr::Ident(ref fname, _) = expr {
                        if self.symbols.is_function(fname) {
                            expr = self.parse_named_call(expr)?;
                            continue;
                        }
                    }

                    // OTHERWISE this is NOT postfix —> this is struct-init scope
                    break;
                }

                _ => break,
            }
        }

        Ok(expr)
    }

    // FUNCTION CALL:   expr(args...)
    fn parse_call(&mut self, func: Expr) -> PResult<Expr> {
        let start = func.span();
        self.stream.expect(TokenKind::LParen)?;

        let mut args = Vec::new();

        if !self.stream.consume_if(TokenKind::RParen) {
            loop {
                let arg = self.parse_expr()?;      // FULL expression
                args.push(arg);

                if !self.stream.consume_if(TokenKind::Comma) {
                    break;
                }
            }
            self.stream.expect(TokenKind::RParen)?;
        }

        let end = self.stream.last_span();

        Ok(Expr::Call {
            target: Box::new(func),
            args,
            span: Span::merge(start, end),
        })
    }

    // FIELD & TUPLE INDEX:   obj.field   /   obj.0
    fn parse_field_or_tuple(&mut self, base: Expr) -> PResult<Expr> {
        let dot = self.stream.next(); // '.'

        match self.stream.peek_kind() {
            TokenKind::Number => {
                let t = self.stream.next_owned();
                let index: usize = t.lexeme.parse().unwrap();
                let span = Span::merge(base.span(), t.span);
                Ok(Expr::TupleIndex {
                    target: Box::new(base),
                    index,
                    span,
                })
            }

            TokenKind::Ident => {
                let field = self.parse_ident()?;
                let span = Span::merge(base.span(), dot.span);
                Ok(Expr::Field {
                    target: Box::new(base),
                    field,
                    span,
                })
            }

            _ => Err(ParserError::Message {
                msg: "Expected field or tuple index after '.'".into(),
                span: self.stream.peek().span,
            }),
        }
    }

    // NAMESPACE:  expr::Name
    fn parse_namespace(&mut self, base: Expr) -> PResult<Expr> {
        let sep = self.stream.next(); // '::'
        let item = self.parse_ident()?;
        let span = Span::merge(base.span(), sep.span);
        Ok(Expr::Namespace {
            base: Box::new(base),
            item,
            span,
        })
    }

    // INDEXING:  expr[ index ]
    fn parse_index(&mut self, base: Expr) -> PResult<Expr> {
        self.stream.expect(TokenKind::LBracket)?;
        let index_expr = self.parse_expr()?;
        self.stream.expect(TokenKind::RBracket)?;

        let span = Span::merge(base.span(), self.stream.last_span());
        Ok(Expr::Index {
            target: Box::new(base),
            index: Box::new(index_expr),
            span,
        })
    }

    // NAMED CALL:  func { x: 1, y{2} }
    fn parse_named_call(&mut self, func: Expr) -> PResult<Expr> {
        let start = func.span();
        self.stream.expect(TokenKind::LBrace)?;
        let mut args = Vec::new();

        if !self.stream.consume_if(TokenKind::RBrace) {
            loop {
                let name = self.parse_ident()?;

                match self.stream.peek_kind() {
                    // Rune-style: field { expr }
                    TokenKind::LBrace => {
                        self.stream.next();
                        let value = self.parse_expr()?;
                        self.stream.expect(TokenKind::RBrace)?;
                        let span = Span::merge(start, self.stream.last_span());
                        args.push(Expr::NamedArg {
                            name,
                            value: Box::new(value),
                            span,
                        });
                    }

                    // Rust-style: field: expr
                    TokenKind::Colon => {
                        self.stream.next();
                        let value = self.parse_expr()?;
                        let span = Span::merge(start, value.span());
                        args.push(Expr::NamedArg {
                            name,
                            value: Box::new(value),
                            span,
                        });
                    }

                    _ => {
                        return Err(ParserError::Message {
                            msg: "Invalid named-call field".into(),
                            span: self.stream.peek().span,
                        });
                    }
                }

                if !self.stream.consume_if(TokenKind::Comma) {
                    break;
                }
            }

            self.stream.expect(TokenKind::RBrace)?;
        }

        let end = self.stream.last_span();
        Ok(Expr::Call {
            target: Box::new(func),
            args,
            span: Span::merge(start, end),
        })
    }
}
