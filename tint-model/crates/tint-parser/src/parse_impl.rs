use crate::Parser;
use crate::error::PResult;
use tint_ast::{Item, ImplBlock, Span};
use tint_lexer::TokenKind;

impl Parser {
    pub fn parse_impl(&mut self) -> PResult<Item> {
        let start = self.stream.expect(TokenKind::Impl)?.span;

        // 1. Parse optional generics: <T, U>
        let mut generics: Vec<String> = Vec::new();
        if self.stream.consume_if(TokenKind::LAngle) {
            loop {
                let ident = self.stream.expect_ident()?;
                generics.push(ident.lexeme.clone());

                if self.stream.consume_if(TokenKind::Comma) {
                    continue;
                }
                break;
            }
            self.stream.expect(TokenKind::RAngle)?;
        }

        // 2. Target type (Type)
        let target = self.parse_type()?;

        // 3. Body
        self.stream.expect(TokenKind::LBrace)?;

        let mut methods = Vec::new();
        while !self.stream.consume_if(TokenKind::RBrace) {
            methods.push(self.parse_fn_decl()?);
        }

        Ok(Item::Impl(ImplBlock {
            span: Span::merge(start, self.stream.last_span()),
            generics,
            target,
            methods,
        }))
    }
}
