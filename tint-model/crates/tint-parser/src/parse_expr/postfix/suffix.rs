use super::*;

impl Parser {
    pub(crate) fn parse_call(&mut self, func: Expr) -> PResult<Expr> {
        let start = func.span();
        self.stream.expect(TokenKind::LParen)?;

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

        let end = self.stream.last_span();

        Ok(Expr::Call {
            target: Box::new(func),
            args,
            span: Span::merge(start, end),
        })
    }

    pub(crate) fn parse_field_or_tuple(&mut self, base: Expr) -> PResult<Expr> {
        let dot = self.stream.next();

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

            TokenKind::Ident | TokenKind::MapLit => {
                let field = if self.stream.peek_kind() == TokenKind::MapLit {
                    self.stream.next().lexeme
                } else {
                    self.parse_ident()?
                };
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

    pub(crate) fn parse_namespace(&mut self, base: Expr) -> PResult<Expr> {
        let sep = self.stream.next();
        let item = self.parse_ident()?;
        let span = Span::merge(base.span(), sep.span);
        Ok(Expr::Namespace {
            base: Box::new(base),
            item,
            span,
        })
    }

    pub(crate) fn parse_index(&mut self, base: Expr) -> PResult<Expr> {
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

    pub(crate) fn parse_named_call(&mut self, func: Expr) -> PResult<Expr> {
        let start = func.span();
        self.stream.expect(TokenKind::LBrace)?;
        let mut args = Vec::new();

        if !self.stream.consume_if(TokenKind::RBrace) {
            loop {
                let name = self.parse_ident()?;

                match self.stream.peek_kind() {
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
