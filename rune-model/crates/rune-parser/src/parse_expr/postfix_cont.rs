use crate::{Parser, error::*};
use rune_ast::{Expr, Span};
use rune_lexer::TokenKind;

impl Parser {
    pub(crate) fn parse_postfix_continuation(&mut self, mut expr: Expr) -> PResult<Expr> {
        loop {
            match self.stream.peek_kind() {
                // -------------------------
                // field access:  expr.foo
                // -------------------------
                TokenKind::Dot => {
                    self.stream.next();
                    let field = self.parse_ident()?;
                    let span = Span::merge(expr.span(), self.stream.last_span());
                    expr = Expr::Field { target: Box::new(expr), field, span };
                }

                // -------------------------
                // namespace:  Foo::Bar
                // -------------------------
                TokenKind::PathSep => {
                    self.stream.next();
                    let item = self.parse_ident()?;
                    let span = Span::merge(expr.span(), self.stream.last_span());
                    expr = Expr::Namespace { base: Box::new(expr), item, span };
                }

                // -------------------------
                // index: expr[ ... ]
                // -------------------------
                TokenKind::LBracket => {
                    self.stream.next();
                    let index = self.parse_expr()?;
                    self.stream.expect(TokenKind::RBracket)?;
                    let span = Span::merge(expr.span(), index.span());
                    expr = Expr::Index { target: Box::new(expr), index: Box::new(index), span };
                }

                // -------------------------
                // NAMED CALL:
                //
                //  make_user { name{"Rune"}, age{20} }
                // -------------------------
                TokenKind::LBrace => {
                    let args = self.parse_named_call_args()?;
                    let span = Span::merge(expr.span(), self.stream.last_span());
                    expr = Expr::Call { 
                        target: Box::new(expr), 
                        args, 
                        span 
                    };
                }


                _ => return Ok(expr),
            }
        }
    }
}
