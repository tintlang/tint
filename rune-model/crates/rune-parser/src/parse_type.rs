// rune-parser/parse_type.rs

use crate::{Parser, error::*};
use rune_ast::Type;
use rune_lexer::TokenKind;

impl Parser {
    /// Parse a type:
    ///   - Simple: MyType
    ///   - Generic: Vec<T>, Map<K,V>, Option<Result<T,E>>
    pub(crate) fn parse_type(&mut self) -> PResult<Type> {
        
        // parse ()
        if self.stream.consume_if(TokenKind::LParen) {
            self.stream.expect(TokenKind::RParen)?;
            return Ok(Type::Unit);
        }

        // First parse type ident, e.g. "Vec"
        let name = self.parse_ident()?;

        // Look for generic arguments: < ... >
        if self.stream.consume_if(TokenKind::Less) {
            let mut params = Vec::new();

            // Case: < >
            if !self.stream.consume_if(TokenKind::Greater) {
                loop {
                    params.push(self.parse_type()?);

                    if self.stream.consume_if(TokenKind::Greater) {
                        break;
                    }

                    self.stream.expect(TokenKind::Comma)?;
                }
            }

            return Ok(Type::Generic(name, params));
        }

        // Otherwise: simple type
        Ok(Type::Simple(name))
    }
}
