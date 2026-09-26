use crate::{error::*, Parser};
use tint_ast::Expr;
use tint_lexer::TokenKind;

impl Parser {
    pub(crate) fn parse_ident_or_struct(&mut self) -> PResult<Expr> {
        let t = self.stream.next();
        let name = t.lexeme.clone();
        let span = t.span;

        // Struct init only if this IDENT is known struct type
        if self.stream.peek_kind() == TokenKind::LBrace && self.symbols.is_type(&name) {
            return self.parse_struct_init(name, span);
        }

        // Otherwise just IDENT
        Ok(Expr::Ident(name, span))
    }
}
