
use crate::{Parser, error::*};
use rune_ast::{Block, Span};
use rune_lexer::TokenKind;

impl Parser {
    pub fn parse_block(&mut self) -> PResult<Block> {
        // Parse "{"
        let start = self.stream.expect(TokenKind::LBrace)?.span;

        let mut stmts = Vec::new();
        let mut last_span = start;

        // Empty blocks do not contain a trailing expression.
        if self.stream.consume_if(TokenKind::RBrace) {
            return Ok(Block {
                stmts,
                span: Span::merge(start, start),
            });
        }

        // Parse multiple statements until "}"
        while !self.stream.consume_if(TokenKind::RBrace) {
            let stmt = self.parse_stmt()?;
            last_span = stmt.span();
            stmts.push(stmt);

            // Optional semicolon after stmt (particularly useful after return)
            self.stream.consume_if(TokenKind::Semicolon);
        }

        Ok(Block {
            stmts,
            span: Span::merge(start, last_span),
        })
    }
}
