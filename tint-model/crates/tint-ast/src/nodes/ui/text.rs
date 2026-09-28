use crate::logic::expr::Expr;
use crate::Span;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum UiTextPart {
    Literal(String, Span),
    Interpolation(Expr, Span),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UiText {
    pub parts: Vec<UiTextPart>,
    pub span: Span,
}
