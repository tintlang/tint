use crate::Span;
use crate::logic::expr::Expr;

#[derive(Debug, Clone)]
pub struct UiModifier {
    pub path: Vec<String>,
    pub value: UiModifierValue,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum UiModifierValue {
    Number(f64),
    String(String),
    Expr(Expr),                 // animate{ opacity: 0 -> 1 }
    Block(Vec<UiModifier>),    // nested modifiers
    Range(f64, f64),
}
