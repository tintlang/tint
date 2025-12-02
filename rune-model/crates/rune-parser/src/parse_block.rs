// rune-parser/parse_block.rs

use crate::{Parser, error::*};
use rune_ast::{Block, Span};
use rune_lexer::TokenKind;

impl Parser {
    pub fn parse_block(&mut self) -> PResult<Block> {
        let start = self.stream.expect(TokenKind::LBrace)?.span;

        let mut stmts = Vec::new();
        let mut last_span = start;

        while !self.stream.consume_if(TokenKind::RBrace) {
            let stmt = self.parse_stmt()?;
            last_span = stmt.span();
            stmts.push(stmt);
        }

        Ok(Block {
            stmts,
            span: Span::merge(start, last_span), 
        })
    }
}
