use crate::{Parser, error::*};
use rune_ast::Expr;
use rune_lexer::TokenKind;

impl Parser {
    pub(crate) fn parse_expr_until(&mut self, stop: TokenKind) -> PResult<Expr> {
        let mut expr = self.parse_primary_strict()?;

        loop {
            if self.stream.peek_kind() == stop { break; }

            match self.stream.peek_kind() {
                TokenKind::Dot | TokenKind::PathSep | TokenKind::LBracket =>
                    expr = self.parse_postfix_continuation(expr)?,

                TokenKind::LParen => break,

                _ => break,
            }
        }

        Ok(expr)
    }
}
