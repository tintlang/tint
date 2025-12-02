use rune_lexer::{Token, TokenKind};
use crate::{token_stream::TokenStream, error::*};
use rune_ast::*;

pub struct Parser {
    pub(crate) stream: TokenStream,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { stream: TokenStream::new(tokens) }
    }

    pub fn parse_program(&mut self) -> PResult<Program> {
        let mut items = Vec::new();

        while self.stream.peek().kind != TokenKind::Eof {
            items.push(self.parse_item()?);
        }

        Ok(Program { items })
    }

    pub(crate) fn parse_item(&mut self) -> PResult<Item> {
        match self.stream.peek().kind {
            TokenKind::Fn => Ok(Item::Fn(self.parse_fn()?)),
            TokenKind::Ui => Ok(Item::UiFn(self.parse_ui_fn()?)),
            TokenKind::Enum => Ok(Item::Enum(self.parse_enum()?)),
            TokenKind::Struct => Ok(Item::Struct(self.parse_struct()?)),
            _ => Err(ParserError::Message {
                msg: "Expected item".into(),
                span: self.stream.peek().span,
            }),
        }
    }
}
