use crate::{Parser, error::*};
use rune_ast::Expr;
use rune_lexer::TokenKind;
use rune_ast::BorrowKind;
use rune_ast::Span;

impl Parser {
    /// Parse:
    ///   borrow(x)
    ///   borrow@strict(x)
    ///   borrow@group(x)
    ///   borrow(x) { ... }
    ///   borrow@strict(x) { ... }
    pub(crate) fn parse_borrow_expr(&mut self) -> PResult<Expr> {
        // expect 'borrow'
        let start_span = self.stream.expect(TokenKind::Borrow)?.span;

        // Default
        let mut kind = BorrowKind::HighLevel;

        // Optional @kind
        if self.stream.consume_if(TokenKind::At) {
            // Check if there was whitespace between @ and tag?
            // NO — because lexer loses whitespace info.

            // So simply reject cases where next is NOT "strict" or "group".
            let ident = self.stream.expect_ident()?;

            match ident.lexeme.as_str() {
                "strict" => kind = BorrowKind::Strict,
                "group"  => kind = BorrowKind::Group,
                other => {
                    return Err(ParserError::Message {
                        msg: format!("Unknown borrow tag `@{}` (only @strict / @group allowed)", other),
                        span: ident.span,
                    });
                }
            }
        }

        // Parse "(" expr ")"
        self.stream.expect(TokenKind::LParen)?;
        let target = self.parse_expr()?;
        self.stream.expect(TokenKind::RParen)?;

        // Optional block
        let block = if self.stream.peek_kind() == TokenKind::LBrace {
            Some(self.parse_block()?)
        } else {
            None
        };

        let span = Span::merge(start_span, self.stream.last_span());


        Ok(Expr::Borrow {
            kind,
            target: Box::new(target),
            block,
            span,
        })
    }
}
