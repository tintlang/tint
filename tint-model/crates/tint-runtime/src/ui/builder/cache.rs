//! Subtree reuse across renders.
//!
//! Building a UI node evaluates its expressions and resolves its styles.
//! Most of a typical UI does not change from one frame to the next, so a
//! finished subtree is remembered together with the outer variables it read
//! (and the values it saw). On the next render the subtree is reused, without
//! evaluating anything below it, if those variables still hold those values.
//!
//! A subtree is only remembered when it is *pure*: its expressions contain no
//! calls, blocks, lambdas or other constructs that could have side effects or
//! depend on anything but variables. Every remembered subtree is shared
//! (`Rc`), so the renderer can also skip it by pointer comparison.

use crate::scope::HashMap;
use std::rc::Rc;

use tint_ast::{
    Expr, StringPart, StructInitField, UiAttrValue, UiModifier, UiModifierValue, UiNode,
    UiNodeOrExpr,
};

use super::super::render::UiRenderNode;

#[derive(Clone)]
pub(super) struct CacheEntry {
    /// Outer variables the node's body read (its `if{}` condition excluded).
    pub reads: Rc<Vec<tint_evaluator::ReadEntry>>,
    /// What the node's `if{}` condition evaluated to, `None` without one. The
    /// condition is re-evaluated on every render and only its *result* has to
    /// match, so flipping `selected` leaves every row but two untouched.
    pub cond: Option<bool>,
    /// Hash of the theme tokens the subtree was built under.
    pub tokens: u64,
    /// `None` records that the node built to nothing (a false `if{}`).
    pub render: Option<Rc<UiRenderNode>>,
}

/// Remembered subtrees. Entries are keyed by the address of the AST node and
/// matched by visit order, so the same node inside a `for{}` loop gets one
/// entry per iteration.
/// A node's modifiers after `use::` expansion and their resolved styles, for
/// nodes whose modifiers contain no expressions: the same every render (and for
/// every iteration of a loop), so resolved once.
pub(super) struct StyleMemo {
    pub expanded: Rc<Vec<UiModifier>>,
    pub bundle: Rc<crate::ui::tree::StyleBundle>,
}

#[derive(Default)]
pub struct UiBuildCache {
    pub(super) entries: HashMap<usize, Vec<Option<CacheEntry>>>,
    /// Keyed by (address of the node's modifiers, theme tokens hash, `if{}`
    /// already handled). `None` marks modifiers that do depend on expressions.
    pub(super) style_memo: HashMap<(usize, u64, bool), Option<Rc<StyleMemo>>>,
    purity: HashMap<usize, bool>,
}

impl UiBuildCache {
    /// Stops remembering this node (its entries never add anything: see the
    /// comparison in `build_reusing`).
    pub(super) fn mark_not_worth_caching(&mut self, key: usize) {
        self.purity.insert(key, false);
    }

    pub(super) fn is_pure(&mut self, key: usize, node: &UiNode) -> bool {
        *self.purity.entry(key).or_insert_with(|| node_is_pure(node))
    }

    /// Drops entries the last render did not visit again, so the cache never
    /// outgrows the tree it describes.
    pub(super) fn sweep(&mut self, visits: &HashMap<usize, usize>) {
        self.entries.retain(|key, entries| match visits.get(key) {
            Some(count) => {
                entries.truncate(*count);
                true
            }
            None => false,
        });
    }
}

fn node_is_pure(node: &UiNode) -> bool {
    match node {
        UiNode::BlockElement {
            attributes,
            modifiers,
            children,
            ..
        } => {
            attributes.iter().all(|a| match &a.value {
                UiAttrValue::Literal(_) | UiAttrValue::Ident(_) => true,
                UiAttrValue::Expr(e) => expr_is_pure(e),
                UiAttrValue::Modifier(_) => false,
            }) && modifiers_are_pure(modifiers)
                && children_are_pure(children)
        }
        UiNode::BlockSelfClosing { modifiers, .. } => modifiers_are_pure(modifiers),
        UiNode::For { iterable, body, .. } => expr_is_pure(iterable) && children_are_pure(body),
        UiNode::Theme { children, .. }
        | UiNode::Slot { children, .. }
        | UiNode::Component { children, .. }
        | UiNode::Variant { children, .. } => children_are_pure(children),
        UiNode::Style { modifiers, .. } => modifiers_are_pure(modifiers),
    }
}

fn children_are_pure(children: &[UiNodeOrExpr]) -> bool {
    children.iter().all(|child| match child {
        UiNodeOrExpr::Node(node) => node_is_pure(node),
        UiNodeOrExpr::Text(text) => text.parts.iter().all(|part| match part {
            tint_ast::UiTextPart::Literal(..) => true,
            tint_ast::UiTextPart::Interpolation(expr, _) => expr_is_pure(expr),
        }),
    })
}

fn modifiers_are_pure(modifiers: &[UiModifier]) -> bool {
    modifiers.iter().all(|m| modifier_value_is_pure(&m.value))
}

fn modifier_value_is_pure(value: &UiModifierValue) -> bool {
    match value {
        UiModifierValue::Number(_)
        | UiModifierValue::String(_)
        | UiModifierValue::Range(..)
        | UiModifierValue::Ident(_) => true,
        UiModifierValue::Expr(expr) => expr_is_pure(expr),
        UiModifierValue::Block(modifiers) => modifiers_are_pure(modifiers),
        UiModifierValue::Tuple(values) => values.iter().all(modifier_value_is_pure),
        UiModifierValue::MiniMod { value, .. } => modifier_value_is_pure(value),
    }
}

/// Only expression forms that read variables and compute from them are pure.
/// Calls (which include method calls), blocks, lambdas, `if`/`match`, borrows,
/// `?` and casts are all treated as possibly impure.
fn expr_is_pure(expr: &Expr) -> bool {
    match expr {
        Expr::Number(..)
        | Expr::String(..)
        | Expr::Bool(..)
        | Expr::Unit(..)
        | Expr::Ident(..)
        | Expr::SelfKw(..) => true,
        Expr::InterpolatedString { parts, .. } => parts.iter().all(|part| match part {
            StringPart::Text(_) => true,
            StringPart::Expr(e) => expr_is_pure(e),
        }),
        Expr::Field { target, .. }
        | Expr::Namespace { base: target, .. }
        | Expr::TupleIndex { target, .. } => expr_is_pure(target),
        Expr::Index { target, index, .. } => expr_is_pure(target) && expr_is_pure(index),
        Expr::Unary { expr, .. } => expr_is_pure(expr),
        Expr::Binary { left, right, .. } => expr_is_pure(left) && expr_is_pure(right),
        Expr::Paren(inner, _) => expr_is_pure(inner),
        Expr::Array { items, .. } | Expr::Tuple { items, .. } => items.iter().all(expr_is_pure),
        Expr::StructInit { fields, .. } | Expr::VariantInit { fields, .. } => {
            fields.iter().all(field_is_pure)
        }
        Expr::StructUpdate { base, updates, .. } => {
            expr_is_pure(base) && updates.iter().all(field_is_pure)
        }
        Expr::MapInit { entries, .. } => entries.iter().all(|(_, e)| expr_is_pure(e)),
        Expr::NamedArg { value, .. } => expr_is_pure(value),
        Expr::Call { .. }
        | Expr::Match { .. }
        | Expr::If { .. }
        | Expr::Lambda { .. }
        | Expr::Block(..)
        | Expr::Borrow { .. }
        | Expr::Try { .. }
        | Expr::Cast { .. } => false,
    }
}

fn field_is_pure(field: &StructInitField) -> bool {
    match field {
        StructInitField::Assign { expr, .. } | StructInitField::Tint { expr, .. } => {
            expr_is_pure(expr)
        }
    }
}

/// True when no modifier value (ignoring a handled `if{}`) needs evaluating.
pub(super) fn modifiers_static(modifiers: &[UiModifier], skip_if: bool) -> bool {
    fn value_static(v: &UiModifierValue) -> bool {
        match v {
            UiModifierValue::Expr(_) => false,
            UiModifierValue::Block(items) => items.iter().all(|m| value_static(&m.value)),
            UiModifierValue::Tuple(items) => items.iter().all(value_static),
            UiModifierValue::MiniMod { value, .. } => value_static(value),
            _ => true,
        }
    }
    modifiers
        .iter()
        .all(|m| (skip_if && m.path.len() == 1 && m.path[0] == "if") || value_static(&m.value))
}
