use crate::{Parser, error::*};
use rune_ast::attr::{Attribute, AttributeList};
use rune_lexer::TokenKind;

impl Parser {
     pub(crate) fn try_parse_attributes(&mut self) -> PResult<Option<AttributeList>> {
        if self.stream.peek_kind() == TokenKind::LBracket {
            let attrs = self.parse_attributes()?;
            Ok(Some(attrs))
        } else {
            Ok(None)
        }
    }

    pub(crate) fn parse_attributes(&mut self) -> PResult<AttributeList> {
        let mut items = Vec::new();

        loop {
            // ищем "["
            if !self.stream.consume_if(TokenKind::LBracket) {
                break;
            }

            // ждём "@"
            self.stream.expect(TokenKind::At)?;
            self.stream.expect(TokenKind::LParen)?;

            // читаем список идентификаторов
            loop {
                let name = self.parse_ident()?;
                items.push(Attribute { name });

                if self.stream.consume_if(TokenKind::Comma) {
                    continue; // ещё атрибут
                }

                break;
            }

            self.stream.expect(TokenKind::RParen)?;
            self.stream.expect(TokenKind::RBracket)?;
        }

        Ok(AttributeList { items })
    }
}
