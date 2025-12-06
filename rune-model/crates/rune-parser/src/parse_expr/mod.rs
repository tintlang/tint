// rune-parser/parse_expr/mod.rs

mod primary;
mod primary_strict;
mod postfix;
mod postfix_cont;
mod binary;
mod expr_until;
mod match_expr;
mod struct_init;
mod parse_named_call_args;

use crate::{Parser, error::*};
use rune_ast::Expr;

impl Parser {
    /// PUBLIC entry point used everywhere else.
    pub fn parse_expr(&mut self) -> PResult<Expr> {
        eprintln!(
            "[EXPR] parse_expr start: token={:?} '{}' at {:?}, in_pattern={}",
            self.stream.peek_kind(),
            self.stream.peek().lexeme,
            self.stream.peek().span,
            self.in_pattern
        );

        let out = self.parse_binary_expr(0)?;

        eprintln!(
            "[EXPR] parse_expr end: token={:?} '{}' at {:?}, in_pattern={}",
            self.stream.peek_kind(),
            self.stream.peek().lexeme,
            self.stream.peek().span,
            self.in_pattern
        );

        Ok(out)
    }
}
