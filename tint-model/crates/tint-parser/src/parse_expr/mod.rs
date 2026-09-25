mod binary;
mod match_expr;
mod parse_borrow;
mod postfix;
mod primary;
mod struct_init;

use crate::{error::*, Parser};
use tint_ast::Expr;

impl Parser {
    pub fn parse_expr(&mut self) -> PResult<Expr> {
        self.parse_binary_expr(0)
    }
}
