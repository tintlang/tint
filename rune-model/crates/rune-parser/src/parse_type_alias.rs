
use crate::{Parser};
use crate::error::*;
use rune_ast::{Span, Item, TypeAliasDecl};
use rune_lexer::TokenKind;

impl Parser {
    pub(crate) fn parse_type_alias(&mut self) -> PResult<Item> {
        // type
        let start = self.stream.expect(TokenKind::Type)?.span;

        // name
        let name = self.parse_ident()?;

        // '='
        self.stream.expect(TokenKind::Eq)?;

        // parse type (this handles union, generics, primitives, tuple, etc.)
        let ty = self.parse_type()?;
        let end = ty.span();

        // A trailing semicolon is optional.
        self.stream.consume_if(TokenKind::Semicolon);

        Ok(Item::TypeAlias(TypeAliasDecl {
            name,
            ty,
            span: Span::merge(start, end),
        }))
    }
}
