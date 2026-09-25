use super::*;

impl Parser {
    pub(crate) fn parse_return_stmt(&mut self) -> PResult<Stmt> {
        let start = self.stream.next().span; // 'return'

        let expr = self.parse_expr()?;
        let end = expr.span();

        self.stream.consume_if(TokenKind::Semicolon);

        Ok(Stmt::Return(expr, Span::merge(start, end)))
    }
    pub(crate) fn parse_if_stmt(&mut self) -> PResult<Stmt> {
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
    pub(crate) fn parse_while_stmt(&mut self) -> PResult<Stmt> {
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
    pub(crate) fn parse_loop_stmt(&mut self) -> PResult<Stmt> {
        let start = self.stream.next().span;
        let body = self.parse_block()?;

        let end = body.span;

        Ok(Stmt::Loop {
            body,
            span: Span::merge(start, end),
        })
    }
    pub(crate) fn parse_break_stmt(&mut self) -> PResult<Stmt> {
        let tok = self.stream.next();
        Ok(Stmt::Break(tok.span))
    }

    pub(crate) fn parse_continue_stmt(&mut self) -> PResult<Stmt> {
        let tok = self.stream.next();
        Ok(Stmt::Continue(tok.span))
    }
    // FOR i in a..b
    pub(crate) fn parse_for_stmt(&mut self) -> PResult<Stmt> {
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
    // LHS for assignment:  a, a.b, a[x]
}
