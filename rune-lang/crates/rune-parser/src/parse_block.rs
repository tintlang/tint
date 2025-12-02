use crate::{Parser, error::*};
use rune_ast::{Block, Span, Stmt};
use rune_lexer::TokenKind;

impl Parser {
    pub fn parse_block(&mut self) -> PResult<Block> {
        let start = self.stream.expect(TokenKind::LBrace)?.span;

        let mut stmts = Vec::new();

        while !self.stream.consume_if(TokenKind::RBrace) {
            stmts.push(self.parse_stmt()?);
        }

        let end = self.stream.last_span();
        Ok(Block {
            stmts,
            span: Span::merge(start, end),
        })
    }

    pub fn parse_stmt(&mut self) -> PResult<Stmt> {
        // минимальная заглушка
        let expr = self.parse_expr()?;
        Ok(Stmt::Expr(expr))
    }
}
