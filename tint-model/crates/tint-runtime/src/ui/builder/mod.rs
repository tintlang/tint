// ui/builder.rs

use super::tree::{UiNodeId, UiTree};
use tint_ast::{
    Expr, Span, UiAttrValue, UiAttribute, UiModifier, UiModifierValue, UiNode, UiNodeOrExpr,
    UiText, UiTextPart,
};
use tint_evaluator::{EvalHost, Value as EvalValue};

pub struct UiBuilder {
    pub tree: UiTree,
}

mod core;
mod helpers;
mod nodes;
