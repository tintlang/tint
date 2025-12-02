use rune_lexer::TokenKind;
use crate::{Parser, error::*};

impl Parser {
    /// Parse identifier (tag names, variable names, attribute names)
    pub fn parse_ident(&mut self) -> PResult<String> {
        let tok = self.stream.expect(TokenKind::Ident)?;
        Ok(tok.lexeme.clone())
    }
}
