use crate::{Parser, error::*};
use rune_ast::{Expr, Span};
use rune_lexer::TokenKind;

impl Parser {
    pub(crate) fn parse_postfix_continuation(&mut self, expr: Expr) -> PResult<Expr> {
 match self.stream.peek_kind() {
        TokenKind::Dot => {
            self.stream.next();
            let field = self.parse_ident()?;
            let span = Span::merge(expr.span(), self.stream.last_span());
            Ok(Expr::Field { target: Box::new(expr), field, span })
        }

        TokenKind::PathSep => {
            self.stream.next();
            let item = self.parse_ident()?;
            let span = Span::merge(expr.span(), self.stream.last_span());
            Ok(Expr::Namespace { base: Box::new(expr), item, span })
        }

        TokenKind::LBracket => {
            self.stream.next();
            let index = self.parse_expr()?;
            self.stream.expect(TokenKind::RBracket)?;
            let span = Span::merge(expr.span(), index.span());
            Ok(Expr::Index { target: Box::new(expr), index: Box::new(index), span })
        }

        _ => Ok(expr),
    }
    }
}
