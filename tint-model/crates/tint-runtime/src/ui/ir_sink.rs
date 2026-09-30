//! Builds a `UiTree` from what a typed-IR `ui fn` emitted (`tint_ir::typed::UiHost`).
//!
//! Everything static about an element (styles, attributes, component
//! expansion) is in its template; the values of its modifier expressions
//! arrive with `open`. The style and event helpers are the ones the
//! tree-walking `UiBuilder` uses, so both produce the same tree.

use super::builder::helpers::*;
use super::tree::{UiNodeId, UiTree};
use std::collections::HashMap;
use tint_ast::{UiModifier, UiModifierValue};
use tint_evaluator::Value as EvalValue;
use tint_ir::typed::{modifier_exprs, UiElementTemplate, UiHost, UiSlot, UiValue};

pub struct IrUiSink {
    tree: UiTree,
    stack: Vec<UiNodeId>,
    tokens: HashMap<String, UiModifierValue>,
}

impl Default for IrUiSink {
    fn default() -> Self {
        Self::new()
    }
}

impl IrUiSink {
    /// Starts with the synthetic `Root` every tree has.
    pub fn new() -> Self {
        let mut tree = UiTree::empty();
        let root = tree.create_node("Root".into());
        IrUiSink {
            tree,
            stack: vec![root],
            tokens: HashMap::new(),
        }
    }

    pub fn root(&self) -> UiNodeId {
        0
    }

    pub fn finish(self) -> UiTree {
        self.tree
    }

    fn parent(&self) -> UiNodeId {
        *self.stack.last().expect("root is never closed")
    }
}

impl UiHost for IrUiSink {
    fn open(&mut self, template: &UiElementTemplate, values: &[UiValue]) {
        // One value per slot that is not `Unused`, in expression order.
        let exprs = modifier_exprs(&template.modifiers);
        let mut by_expr: HashMap<usize, EvalValue> = HashMap::new();
        let mut next = values.iter();
        for (expr, slot) in exprs.iter().zip(&template.slots) {
            if *slot == UiSlot::Unused {
                continue;
            }
            if let Some(v) = next.next() {
                let v = match v {
                    UiValue::Number(n) => EvalValue::Number(*n),
                    UiValue::Str(s) => EvalValue::String(s.clone()),
                    UiValue::Bool(b) => EvalValue::Bool(*b),
                };
                by_expr.insert(*expr as *const _ as usize, v);
            }
        }
        let id = self.tree.create_styled_node_with_eval(
            template.tag.clone(),
            &template.modifiers,
            &mut |e| {
                by_expr
                    .get(&(e as *const _ as usize))
                    .cloned()
                    .unwrap_or(EvalValue::Unit)
            },
            &self.tokens,
        );
        let attributes = &template.attributes;
        apply_events(&mut self.tree, id, attributes);
        apply_svg(&mut self.tree, id, attributes);
        apply_route(&mut self.tree, id, attributes);
        apply_tick(&mut self.tree, id, attributes);
        apply_pointer(&mut self.tree, id, attributes);
        apply_input(&mut self.tree, id, attributes);
        apply_preview(&mut self.tree, id, attributes);
        apply_target(&mut self.tree, id, attributes);
        apply_reference(&mut self.tree, id, attributes);
        apply_js_handler(&mut self.tree, id, attributes);
        apply_asset(&mut self.tree, id, attributes);
        apply_key(&mut self.tree, id, &template.modifiers);
        let parent = self.parent();
        self.tree.add_child(parent, id);
        self.stack.push(id);
    }

    fn close(&mut self) {
        if self.stack.len() > 1 {
            self.stack.pop();
        }
    }

    fn text(&mut self, text: &str) {
        let id = self.tree.create_text_node(text.to_string());
        let parent = self.parent();
        self.tree.add_child(parent, id);
    }

    fn tokens(&mut self, modifiers: &[UiModifier]) {
        for modifier in modifiers {
            if modifier.path.len() == 1 {
                self.tokens
                    .insert(modifier.path[0].clone(), modifier.value.clone());
            }
        }
    }
}
