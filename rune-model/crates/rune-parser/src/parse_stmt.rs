// rune-parser/parse_stmt.rs

use crate::{Parser};
use crate::{ error::*};
use rune_ast::*;
use rune_lexer::TokenKind;

impl Parser {
pub fn parse_stmt(&mut self) -> PResult<Stmt> {
    let tok = self.stream.peek();

    match tok.kind {
        TokenKind::Let => {
            // LET stmt
            let start = self.stream.next().span;
            let name = self.parse_ident()?;
            self.stream.expect(TokenKind::Eq)?;
            let expr = self.parse_expr()?;
            let end = expr.span();
            self.stream.consume_if(TokenKind::Semicolon);

            return Ok(Stmt::Let {
                name,
                expr,
                span: Span::merge(start, end),
            });
        }

        TokenKind::Ident
        | TokenKind::Number
        | TokenKind::String
        | TokenKind::LParen => {
            // expression statement
            let expr = self.parse_expr()?;
            self.stream.consume_if(TokenKind::Semicolon);
            return Ok(Stmt::Expr(expr));
        }

        _ => {
            return Err(ParserError::Message {
                msg: format!("Unexpected token in statement: {:?}", tok.kind),
                span: tok.span,
            })
        }
    }
}
}
