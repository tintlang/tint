use crate::{error::*, Parser};
use tint_ast::{ConstDecl, Item, Span};
use tint_lexer::TokenKind;

impl Parser {
    pub(crate) fn parse_const_decl(&mut self) -> PResult<Item> {
        let start = self.stream.expect(TokenKind::Const)?.span;
        let name_token = self.stream.expect(TokenKind::Ident)?;
        let name = name_token.lexeme.clone();
        self.stream.expect(TokenKind::Colon)?;
        let ty = Some(self.parse_type()?);
        self.stream.expect(TokenKind::Eq)?;
        let init = self.parse_expr()?;
        self.stream.consume_if(TokenKind::Semicolon);
        let span = Span::merge(start, init.span());
        Ok(Item::Const(ConstDecl {
            name,
            ty,
            init,
            span,
        }))
    }
}
