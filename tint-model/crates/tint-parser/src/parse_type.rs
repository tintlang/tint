use crate::{error::*, Parser};
use tint_ast::Type;
use tint_lexer::TokenKind;

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

        // `union(...)` is parsed as a dedicated composite type.
        if name == "union" && self.stream.consume_if(TokenKind::LParen) {
            let mut types = Vec::new();

            // A union requires at least one member type.
            types.push(self.parse_type()?);

            // Additional members are separated by `|`.
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
