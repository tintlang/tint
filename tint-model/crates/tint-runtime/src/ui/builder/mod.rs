// ui/builder.rs

use super::tree::{UiNodeId, UiTree};
use std::collections::HashMap;
use tint_ast::{
    Expr, Span, UiAttrValue, UiAttribute, UiModifier, UiModifierValue, UiNode, UiNodeOrExpr,
    UiText, UiTextPart,
};
use tint_evaluator::{EvalHost, Value as EvalValue};

#[derive(Clone)]
pub struct UiComponent {
    pub modifiers: Vec<UiModifier>,
    pub children: Vec<UiNodeOrExpr>,
    pub variants: HashMap<String, (Vec<UiModifier>, Vec<UiNodeOrExpr>)>,
}

pub use cache::UiBuildCache;

pub struct UiBuilder {
    pub tree: UiTree,
    /// Values collected from the active `theme::{ tokens { ... } }` block.
    /// Token resolution happens while UI styles are materialized.
    pub tokens: HashMap<String, UiModifierValue>,
    pub styles: HashMap<String, Vec<UiModifier>>,
    pub components: HashMap<String, UiComponent>,
    /// Subtree reuse across renders; `None` builds everything fresh.
    pub cache: Option<UiBuildCache>,
    visits: HashMap<usize, usize>,
    component_depth: usize,
    tokens_hash: u64,
}

mod cache;
mod core;
pub(crate) mod helpers;
mod nodes;
