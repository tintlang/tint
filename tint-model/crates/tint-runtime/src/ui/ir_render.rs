//! Turns what a typed-IR `ui fn` emitted into shared render nodes, reusing
//! every subtree that is identical to one from the previous render.
//!
//! An element is identified by its template, its run-time values and its
//! children (already identified the same way), so the identity of a subtree
//! is a hash of its events and equal subtrees are the *same* `Rc`. No
//! dependency tracking is involved: the IR re-runs the whole `ui fn`, which is
//! cheap, and unchanged parts come back as pointer-equal nodes, which is what
//! the DOM patch skips on.

use super::builder::helpers::*;
use super::render::{to_render_tree, UiRenderNode};
use super::tree::UiTree;
use crate::scope::HashMap;
use std::collections::HashMap as StdMap;
use std::rc::Rc;
use tint_ast::{UiModifier, UiModifierValue};
use tint_evaluator::Value as EvalValue;
use tint_ir::typed::{modifier_exprs, UiElementTemplate, UiEvent, UiSlot, UiTemplate, UiValue};

type Hash = (u64, u64);

fn mix(h: &mut Hash, x: u64) {
    h.0 = (h.0 ^ x).wrapping_mul(0x9E37_79B9_7F4A_7C15).rotate_left(29);
    h.1 = h.1.wrapping_add(x ^ 0xD6E8_FEB8_6659_FD93).wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    h.1 ^= h.1 >> 32;
}

fn mix_str(h: &mut Hash, s: &str) {
    let mut chunks = s.as_bytes().chunks_exact(8);
    for c in &mut chunks {
        mix(h, u64::from_le_bytes(c.try_into().unwrap()));
    }
    let mut last = [0u8; 8];
    let rest = chunks.remainder();
    last[..rest.len()].copy_from_slice(rest);
    mix(h, u64::from_le_bytes(last) ^ ((s.len() as u64) << 56));
}

enum Child {
    Node(Rc<UiRenderNode>),
    Text(String),
}

struct Frame<'e> {
    template: Option<&'e UiElementTemplate>,
    index: u32,
    values: &'e [UiValue],
    hash: Hash,
    children: Vec<Child>,
}

#[derive(Default)]
pub struct IrRenderer {
    prev: HashMap<Hash, Rc<UiRenderNode>>,
    cur: HashMap<Hash, Rc<UiRenderNode>>,
    scratch: Option<UiTree>,
    /// Elements whose look does not depend on run-time values, resolved once
    /// per (template, tokens): only their children differ between uses.
    protos: HashMap<(u32, u64), UiRenderNode>,
    tokens: StdMap<String, UiModifierValue>,
    tokens_hash: u64,
}

impl IrRenderer {
    pub fn new() -> Self {
        Self::default()
    }

    /// The top-level nodes of the tree `events` describe.
    pub fn render(
        &mut self,
        templates: &[UiTemplate],
        events: &[UiEvent],
    ) -> Vec<Rc<UiRenderNode>> {
        self.prev = std::mem::take(&mut self.cur);
        self.cur.reserve(self.prev.len());
        self.tokens.clear();
        self.tokens_hash = 0;
        let mut stack: Vec<Frame> = vec![Frame {
            template: None,
            index: 0,
            values: &[],
            hash: (0, 0),
            children: Vec::new(),
        }];
        for event in events {
            match event {
                UiEvent::Open { template, values } => {
                    let Some(UiTemplate::Element(t)) = templates.get(*template as usize) else {
                        continue;
                    };
                    let mut hash = (self.tokens_hash, *template as u64);
                    for v in values {
                        match v {
                            UiValue::Number(n) => mix(&mut hash, n.to_bits()),
                            UiValue::Str(s) => mix_str(&mut hash, s),
                            UiValue::Bool(b) => mix(&mut hash, *b as u64 + 2),
                        }
                    }
                    stack.push(Frame {
                        template: Some(t),
                        index: *template,
                        values,
                        hash,
                        children: Vec::new(),
                    });
                }
                UiEvent::Close => {
                    if stack.len() > 1 {
                        let frame = stack.pop().unwrap();
                        let node = self.finish(frame);
                        stack.last_mut().unwrap().children.push(Child::Node(node));
                    }
                }
                UiEvent::Text(s) => {
                    let top = stack.last_mut().unwrap();
                    mix_str(&mut top.hash, s);
                    mix(&mut top.hash, 0x7E47);
                    top.children.push(Child::Text(s.clone()));
                }
                UiEvent::Tokens { template } => {
                    if let Some(UiTemplate::Tokens { modifiers }) = templates.get(*template as usize) {
                        for modifier in modifiers {
                            if modifier.path.len() == 1 {
                                self.tokens.insert(modifier.path[0].clone(), modifier.value.clone());
                            }
                        }
                        self.tokens_hash = hash_tokens(&self.tokens);
                    }
                }
            }
        }
        while stack.len() > 1 {
            let frame = stack.pop().unwrap();
            let node = self.finish(frame);
            stack.last_mut().unwrap().children.push(Child::Node(node));
        }
        let root = stack.pop().unwrap();
        // Top-level text is not a node of its own here; the DOM roots are elements.
        root.children
            .into_iter()
            .filter_map(|c| match c {
                Child::Node(n) => Some(n),
                Child::Text(_) => None,
            })
            .collect()
    }

    fn finish(&mut self, mut frame: Frame) -> Rc<UiRenderNode> {
        let mut hash = frame.hash;
        for child in &frame.children {
            if let Child::Node(n) = child {
                mix(&mut hash, Rc::as_ptr(n) as usize as u64);
            }
        }
        // The parent hashes this node by address, so the node has to exist
        // (and stay alive) whichever way it was found.
        if let Some(found) = self.cur.get(&hash).or_else(|| self.prev.get(&hash)) {
            let found = Rc::clone(found);
            self.cur.entry(hash).or_insert_with(|| Rc::clone(&found));
            return found;
        }
        let node = Rc::new(self.build(&mut frame));
        self.cur.insert(hash, Rc::clone(&node));
        node
    }

    fn build(&mut self, frame: &mut Frame) -> UiRenderNode {
        let template = frame.template.expect("only elements are built");
        let fixed = template.slots.iter().all(|s| *s == UiSlot::Unused);
        let only_nodes = frame.children.iter().all(|c| matches!(c, Child::Node(_)));
        let lone_text = frame.children.len() == 1
            && matches!(frame.children[0], Child::Text(_))
            && !matches!(template.tag.as_str(), "TextArea" | "Preview")
            && crate::ui::render::fold_text_enabled();
        if fixed && (only_nodes || lone_text) {
            let key = (frame.index, self.tokens_hash);
            if !self.protos.contains_key(&key) {
                let mut bare = Frame {
                    template: frame.template,
                    index: frame.index,
                    values: frame.values,
                    hash: frame.hash,
                    children: Vec::new(),
                };
                let node = self.build_slow(&mut bare);
                self.protos.insert(key, node);
            }
            let mut node = self.protos[&key].clone();
            if lone_text {
                if let Some(Child::Text(s)) = frame.children.pop() {
                    if node.text.is_none() {
                        node.text = Some(s);
                        return node;
                    }
                    frame.children.push(Child::Text(s));
                }
            } else {
                node.children = frame
                    .children
                    .drain(..)
                    .filter_map(|c| match c {
                        Child::Node(n) => Some(n),
                        Child::Text(_) => None,
                    })
                    .collect();
                return node;
            }
        }
        self.build_slow(frame)
    }

    fn build_slow(&mut self, frame: &mut Frame) -> UiRenderNode {
        let template = frame.template.expect("only elements are built");
        let mut tree = self.scratch.take().unwrap_or_else(UiTree::empty);
        tree.nodes.clear();
        let mut by_expr: Vec<(usize, EvalValue)> = Vec::new();
        let mut exprs = modifier_exprs(&template.modifiers);
        exprs.extend(tint_ir::typed::ui::attr_exprs(&template.attributes));
        let mut next = frame.values.iter();
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
                by_expr.push((*expr as *const _ as usize, v));
            }
        }
        let lookup = |e: &tint_ast::Expr| {
            let p = e as *const _ as usize;
            by_expr.iter().find(|(k, _)| *k == p).map(|(_, v)| v.clone()).unwrap_or(EvalValue::Unit)
        };
        let id = tree.create_styled_node_with_eval(
            template.tag.clone(),
            &template.modifiers,
            &mut |e| lookup(e),
            &self.tokens,
        );
        let attributes = &template.attributes;
        apply_slot_attrs(&mut tree, id, attributes, &lookup);
        apply_slot_component(&mut tree, id, attributes, &lookup);
        apply_events(&mut tree, id, attributes);
        apply_svg(&mut tree, id, attributes);
        apply_route(&mut tree, id, attributes);
        apply_tick(&mut tree, id, attributes);
        apply_pointer(&mut tree, id, attributes);
        apply_input(&mut tree, id, attributes);
        apply_preview(&mut tree, id, attributes);
        apply_target(&mut tree, id, attributes);
        apply_reference(&mut tree, id, attributes);
        apply_js_handler(&mut tree, id, attributes);
        apply_asset(&mut tree, id, attributes);
        apply_key(&mut tree, id, &template.modifiers);
        for modifier in &template.modifiers {
            if modifier.path.len() == 1 && modifier.path[0] == "key" {
                if let UiModifierValue::Expr(expr) = &modifier.value {
                    let key = match lookup(expr) {
                        EvalValue::String(s) => s,
                        other => other.to_string(),
                    };
                    tree.set_key(id, Some(key));
                }
            }
        }
        for child in frame.children.drain(..) {
            let cid = match child {
                Child::Node(rc) => {
                    let cid = tree.create_node(String::new());
                    tree.nodes[cid].prebuilt = Some(rc);
                    cid
                }
                Child::Text(s) => tree.create_text_node(s),
            };
            tree.add_child(id, cid);
        }
        let node = to_render_tree(&tree, id);
        self.scratch = Some(tree);
        node
    }
}

fn hash_tokens(tokens: &StdMap<String, UiModifierValue>) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut keys: Vec<&String> = tokens.keys().collect();
    keys.sort();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for key in keys {
        key.hash(&mut hasher);
        format!("{:?}", tokens[key]).hash(&mut hasher);
    }
    hasher.finish()
}

#[allow(dead_code)]
fn _unused(_: &[UiModifier]) {}

/// `placeholder||"text"`, `disabled||{busy}` ...: plain HTML attributes (see `apply_html_attrs`); an
/// expression's value is the slot's. A bool is present (empty) when true and left out when false.
fn apply_slot_attrs(
    tree: &mut UiTree,
    id: crate::ui::tree::UiNodeId,
    attributes: &[tint_ast::UiAttribute],
    lookup: &dyn Fn(&tint_ast::Expr) -> EvalValue,
) {
    use tint_ast::UiAttrValue;
    let mut out: Vec<(String, String)> = Vec::new();
    for attr in attributes {
        if attr.name == "props" || !tint_ir::typed::ui::VALUE_ATTRS.contains(&attr.name.as_str()) {
            continue;
        }
        let value = match &attr.value {
            UiAttrValue::Literal(s) => Some(EvalValue::String(s.clone())),
            UiAttrValue::Expr(expr) => Some(lookup(expr)),
            UiAttrValue::Ident(name) if name == "true" => Some(EvalValue::Bool(true)),
            UiAttrValue::Ident(name) if name == "false" => Some(EvalValue::Bool(false)),
            _ => None,
        };
        match value {
            Some(EvalValue::Bool(true)) => out.push((attr.name.clone(), String::new())),
            Some(EvalValue::Bool(false)) | None => {}
            Some(EvalValue::String(s)) => out.push((attr.name.clone(), s)),
            Some(other) => out.push((attr.name.clone(), other.to_string())),
        }
    }
    out.sort();
    tree.set_attrs(id, out);
}

/// `component||"Name"` with `props||{..}`: the props arrive as JSON text already.
fn apply_slot_component(
    tree: &mut UiTree,
    id: crate::ui::tree::UiNodeId,
    attributes: &[tint_ast::UiAttribute],
    lookup: &dyn Fn(&tint_ast::Expr) -> EvalValue,
) {
    use tint_ast::UiAttrValue;
    let name = attributes.iter().find_map(|a| match (&*a.name, &a.value) {
        ("component", UiAttrValue::Literal(s)) => Some(s.clone()),
        _ => None,
    });
    let Some(name) = name else { return };
    let props = attributes.iter().find_map(|a| match (&*a.name, &a.value) {
        ("props", UiAttrValue::Expr(e)) => match lookup(e) {
            EvalValue::String(json) => Some(json),
            _ => None,
        },
        _ => None,
    });
    tree.set_component(id, Some(name), props);
}
