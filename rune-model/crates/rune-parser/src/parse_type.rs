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

        let name = self.parse_ident()?; 

        // Если имя == "union", то начинаем парсить union(...)
        if name == "union" && self.stream.consume_if(TokenKind::LParen) {

            let mut types = Vec::new();

            // минимум один тип
            types.push(self.parse_type()?);

            // остальные через |
            while self.stream.consume_if(TokenKind::Pipe) {
                types.push(self.parse_type()?);
            }

            self.stream.expect(TokenKind::RParen)?;

            return Ok(Type::Union(types));
        }

        // Look for generic arguments: < ... >
        if self.stream.consume_if(TokenKind::LAngle) {
            let mut params = Vec::new();

            // Case: < >
            if !self.stream.consume_if(TokenKind::RAngle) {
                loop {
                    params.push(self.parse_type()?);

                    if self.stream.consume_if(TokenKind::RAngle) {
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
