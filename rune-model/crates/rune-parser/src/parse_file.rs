use crate::{Parser, error::*};
use rune_ast::{File, Span, attr::AttributeList};
use rune_lexer::TokenKind;

impl Parser {
    pub fn parse_file(&mut self) -> PResult<File> {
        let start_span = self.stream.peek().span;

        // 1) global attributes at top level
        let mut globals = AttributeList::empty();
        loop {
            if let Some(attr) = self.try_parse_attributes()? {
                globals.extend(attr);
            } else {
                break;
            }
        }

        // 2) items
        let mut items = Vec::new();
        while self.stream.peek_kind() != TokenKind::Eof {
            items.push(self.parse_item()?);
        }

        let end_span = self.stream.last_span();
        let span = Span::merge(start_span, end_span);

        Ok(File { globals, items, span })
    }
}
