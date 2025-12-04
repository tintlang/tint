mod number;
mod bool_lit;
mod ident_or_struct;
mod lambda;
mod array_lit;
mod paren_or_tuple;
mod unary;
mod string; 

use crate::{Parser, error::*};
use rune_ast::Expr;
use rune_lexer::TokenKind;

impl Parser {
    pub(crate) fn parse_primary(&mut self) -> PResult<Expr> {
        let tok = self.stream.peek().clone();

        match tok.kind {
            TokenKind::Number     => self.parse_number(),
            TokenKind::String     => self.parse_string_or_interpolated(),
            TokenKind::True
            | TokenKind::False    => self.parse_bool(),

            TokenKind::Ident      => self.parse_ident_or_struct(),

            TokenKind::Match      => self.parse_match_expression(),
            TokenKind::PipeLambda => self.parse_lambda(),

            TokenKind::LBracket   => self.parse_array_literal(),
            TokenKind::LParen     => self.parse_paren_or_tuple(),

            TokenKind::Bang
            | TokenKind::Minus    => self.parse_unary(),

            _ => Err(ParserError::Message {
                msg: format!("Unexpected token in expression: {:?}", tok.kind),
                span: tok.span,
            }),
        }
    }
}
