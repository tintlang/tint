use crate::logic::expr::Expr;
use crate::Span;

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
    Expr(Expr),
    Range(f64, f64),
    Block(Vec<UiModifier>),      // padding::{ left::..., right::... }
    Tuple(Vec<UiModifierValue>), // text::{24, bold}
    Ident(String),
    MiniMod {
        key: Vec<String>,
        value: Box<UiModifierValue>,
    },
}
