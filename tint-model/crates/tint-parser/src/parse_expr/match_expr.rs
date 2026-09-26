use crate::{error::*, Parser};
use tint_ast::{Expr, MatchArm, Span};
use tint_lexer::TokenKind;

impl Parser {
    pub(crate) fn parse_match_arm(&mut self) -> PResult<MatchArm> {
        let start = self.stream.peek().span;

        // PATTERN
        let old = self.in_pattern;
        self.in_pattern = true;
        let pattern = self.parse_pattern()?;
        self.in_pattern = old;

        // OPTIONAL GUARD: `if expr`
        let guard = if self.stream.consume_if(TokenKind::If) {
            Some(self.parse_expr()?)
        } else {
            None
        };

        // FAT ARROW =>
        self.stream.expect(TokenKind::FatArrow)?;

        // BODY
        let expr = self.parse_expr()?;
        let end = expr.span();

        Ok(MatchArm {
            pattern,
            guard,
            expr,
            span: Span::merge(start, end),
        })
    }
    pub(crate) fn parse_match_expression(&mut self) -> PResult<Expr> {
        let start = self.stream.next().span; // 'match'

        // scrutinee
        let scrutinee = self.parse_expr()?; // Parse the complete scrutinee expression.
        self.stream.expect(TokenKind::LBrace)?; // match-body begins

        let mut arms = Vec::new();

        while !self.stream.consume_if(TokenKind::RBrace) {
            let arm = self.parse_match_arm()?;
            arms.push(arm);
            self.stream.consume_if(TokenKind::Comma);
        }

        let end = self.stream.last_span();

        Ok(Expr::Match {
            scrutinee: Box::new(scrutinee),
            arms,
            span: Span::merge(start, end),
        })
    }
}
