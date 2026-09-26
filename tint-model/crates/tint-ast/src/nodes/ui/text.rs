use crate::logic::expr::Expr;
use crate::Span;

#[derive(Debug, Clone)]
pub enum UiTextPart {
    Literal(String, Span),
    Interpolation(Expr, Span),
}

#[derive(Debug, Clone)]
pub struct UiText {
    pub parts: Vec<UiTextPart>,
    pub span: Span,
}
