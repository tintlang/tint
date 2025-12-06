use crate::{Parser, error::*};
use rune_ast::Span;
use rune_ast::Expr;
use rune_lexer::TokenKind;

impl Parser {
    pub(crate) fn parse_named_call_args(&mut self) -> PResult<Vec<Expr>> {
        self.stream.expect(TokenKind::LBrace)?;
        let mut args = Vec::new();

        // empty { }
        if self.stream.consume_if(TokenKind::RBrace) {
            return Ok(args);
        }

        loop {
            // name
            let name = self.parse_ident()?;
            let start = self.stream.last_span();

            // name { expr }
            self.stream.expect(TokenKind::LBrace)?;
            let expr = self.parse_expr()?;
            self.stream.expect(TokenKind::RBrace)?;

            let end = expr.span();

            args.push(Expr::NamedArg {
                name,
                value: Box::new(expr),
                span: Span::merge(start, end),
            });

            if !self.stream.consume_if(TokenKind::Comma) {
                break;
            }
        }

        self.stream.expect(TokenKind::RBrace)?;
        Ok(args)
    }
}
