
use crate::{Parser, error::*};
use tint_lexer::TokenKind;

impl Parser {

    // generics <T,U>
   pub(crate) fn parse_optional_generics(&mut self) -> PResult<Vec<String>> {
        let mut out = Vec::new();

        if !self.stream.consume_if(TokenKind::LAngle) {
            return Ok(out);
        }

        loop {
            out.push(self.parse_ident()?);

            if self.stream.consume_if(TokenKind::RAngle) {
                break;
            }

            self.stream.expect(TokenKind::Comma)?;
        }

        Ok(out)
    }
}
