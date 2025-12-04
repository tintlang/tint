// rune-parser/parse_expr/primary/ident_or_struct.rs

use crate::{Parser, error::*};
use rune_ast::Expr;
use rune_lexer::TokenKind;

impl Parser {
    pub(crate) fn parse_ident_or_struct(&mut self) -> PResult<Expr> {
        let t = self.stream.next();

        if self.stream.peek_kind() == TokenKind::LBrace {
            return self.parse_struct_init(t.lexeme, t.span);
        }

        Ok(Expr::Ident(t.lexeme, t.span))
    }
}
