// rune-parser/parse_expr/primary/ident_or_struct.rs

use crate::{Parser, error::*};
use rune_ast::Expr;
use rune_lexer::TokenKind;

impl Parser {
    
    pub(crate) fn parse_ident_or_struct(&mut self) -> PResult<Expr> {
        let t = self.stream.next();
        let name = t.lexeme.clone();
        let span = t.span;

        // Struct init only if this IDENT is known struct type
        if self.stream.peek_kind() == TokenKind::LBrace
            && self.symbols.is_type(&name)
        {
            return self.parse_struct_init(name, span);
        }

        // Otherwise just IDENT
        Ok(Expr::Ident(name, span))
    }

}
