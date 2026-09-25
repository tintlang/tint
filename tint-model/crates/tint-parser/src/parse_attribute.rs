use crate::{Parser, error::*};
use tint_ast::attr::{Attribute, AttributeList};
use tint_lexer::TokenKind;

impl Parser {
    pub(crate) fn try_parse_attributes(&mut self) -> PResult<Option<AttributeList>> {
        if self.stream.peek_kind() != TokenKind::LBracket {
            return Ok(None);
        }
        self.parse_attributes().map(Some)
    }

    pub(crate) fn parse_attributes(&mut self) -> PResult<AttributeList> {
        self.stream.expect(TokenKind::LBracket)?; // '['
        self.stream.expect(TokenKind::At)?;       // '@'
        self.stream.expect(TokenKind::LParen)?;   // '('

        let mut list = AttributeList::empty();

        loop {
            // --- parse dotted attribute name ---
            let mut full_name = self.parse_ident()?; // first segment

            // read ".seg" multiple times
            while self.stream.consume_if(TokenKind::Dot) {
                let seg = self.parse_ident()?;
                full_name.push('.');
                full_name.push_str(&seg);
            }

            list.items.push(Attribute { name: full_name });

            // Stop when the attribute list is not followed by a comma.
            if !self.stream.consume_if(TokenKind::Comma) {
                break;
            }
        }

        self.stream.expect(TokenKind::RParen)?;  // ')'
        self.stream.expect(TokenKind::RBracket)?; // ']'

        Ok(list)
    }
}

