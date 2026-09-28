// ui/builder.rs

use super::tree::{UiNodeId, UiTree};
use tint_ast::{
    Expr, Span, UiAttrValue, UiAttribute, UiModifier, UiModifierValue, UiNode, UiNodeOrExpr,
    UiText, UiTextPart,
};
use tint_evaluator::{EvalHost, Value as EvalValue};
use std::collections::HashMap;

#[derive(Clone)]
pub struct UiComponent {
    pub modifiers: Vec<UiModifier>,
    pub children: Vec<UiNodeOrExpr>,
    pub variants: HashMap<String, (Vec<UiModifier>, Vec<UiNodeOrExpr>)>,
}

pub struct UiBuilder {
    pub tree: UiTree,
    /// Values collected from the active `theme::{ tokens { ... } }` block.
    /// Token resolution happens while UI styles are materialized.
    pub tokens: HashMap<String, UiModifierValue>,
    pub styles: HashMap<String, Vec<UiModifier>>,
    pub components: HashMap<String, UiComponent>,
}

mod core;
mod helpers;
mod nodes;
