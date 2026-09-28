use super::block::UiModifierBlock;
use crate::logic::expr::Expr;
use crate::Span;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UiAttribute {
    pub name: String,
    pub value: UiAttrValue,
    pub span: Span,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum UiAttrValue {
    Literal(String), // attr||"text"
    Ident(String),   // attr||foo
    Expr(Expr),      // attr||a + b
    Modifier(UiModifierBlock),
}
