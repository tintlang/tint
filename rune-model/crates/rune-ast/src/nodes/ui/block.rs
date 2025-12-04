use crate::Span;
use super::modifier::UiModifier;
use super::modifier::UiModifierValue;

#[derive(Debug, Clone)]
pub struct UiModifierBlock {
    pub path: Vec<String>,
    pub items: Vec<UiModifierItem>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct UiModifierItem {
    pub key: String,
    pub value: Option<UiModifierValue>,
    pub children: Vec<UiModifierItem>,
    pub span: Span,
}
