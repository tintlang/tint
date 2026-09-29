use crate::{error::*, Parser};
use tint_ast::{Expr, Span};

impl Parser {
    /// `-x.y()` and `!xs.is_empty()` apply the operator to the whole postfix
    /// chain (a method call binds tighter than a prefix operator), but `as`
    /// binds looser: `-1 as u32` is `(-1) as u32`.
    pub(crate) fn parse_unary(&mut self) -> PResult<Expr> {
        let op = self.stream.next();
        let mut operand = self.parse_postfix()?;

        let mut casts = Vec::new();
        while let Expr::Cast { expr, ty, .. } = operand {
            casts.push(ty);
            operand = *expr;
        }

        let span = Span::merge(op.span, operand.span());
        let mut result = Expr::Unary {
            op: op.lexeme,
            expr: Box::new(operand),
            span,
        };
        for ty in casts.into_iter().rev() {
            let span = Span::merge(result.span(), ty.span());
            result = Expr::Cast {
                expr: Box::new(result),
                ty,
                span,
            };
        }
        Ok(result)
    }
}
