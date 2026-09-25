// ui/builder.rs

use super::tree::{UiTree, UiNodeId};
use tint_ast::{Expr, UiAttrValue, UiAttribute, UiModifier, UiModifierValue, UiNode, UiNodeOrExpr, UiText, UiTextPart};
use tint_evaluator::{EvalHost, Value as EvalValue};

pub struct UiBuilder {
    pub tree: UiTree,
}

impl UiBuilder {
    pub fn new() -> Self {
        Self { tree: UiTree::empty() }
    }

    /// Create synthetic ROOT (id=0..n)
    ///
    /// `host` is the running VM: building the tree now genuinely evaluates
    /// `if{}`/`for{}` conditions and iterables, and text interpolations,
    /// through it (see `check_if`/`for_loop`/`render_ui_text` below) --
    /// this used to be a pure AST-level pass with no evaluator available,
    /// which is why those constructs were previously inert.
    pub fn build_root<H: EvalHost>(&mut self, nodes: &Vec<UiNode>, host: &mut H) -> UiNodeId {
        let root = self.tree.create_node("Root".into());

        for n in nodes {
            if let Some(child) = self.build(n, host) {
                self.tree.add_child(root, child);
            }
        }

        root
    }

    /// Convert UiNode AST → UiTree element. Returns `None` when the node
    /// carries an `if{}` modifier that evaluated falsy -- such a node (and
    /// everything under it) simply isn't added to its parent.
    pub fn build<H: EvalHost>(&mut self, node: &UiNode, host: &mut H) -> Option<UiNodeId> {
        match node {
            UiNode::Element { name, attributes, modifiers, children, .. } => {
                self.build_container(name, attributes, modifiers, children, host)
            }
            UiNode::BlockElement { name, attributes, modifiers, children, .. } => {
                self.build_container(name, attributes, modifiers, children, host)
            }
            UiNode::SelfClosing { name, attributes, modifiers, .. } => {
                if !check_if(modifiers, host) {
                    return None;
                }
                let id = self.tree.create_styled_node(name.clone(), modifiers);
                apply_events(&mut self.tree, id, attributes);
                apply_svg(&mut self.tree, id, attributes);
                Some(id)
            }
            UiNode::BlockSelfClosing { name, modifiers, .. } => {
                // Block-mode self-closing nodes (`Tag {}`) carry no
                // `attributes` in the AST at all (see tint_ast::UiNode) --
                // only XML self-closing (`<Tag />`) does. Nothing to wire
                // events from here.
                if !check_if(modifiers, host) {
                    return None;
                }
                Some(self.tree.create_styled_node(name.clone(), modifiers))
            }
        }
    }

    fn build_container<H: EvalHost>(
        &mut self,
        name: &str,
        attributes: &[UiAttribute],
        modifiers: &[UiModifier],
        children: &[UiNodeOrExpr],
        host: &mut H,
    ) -> Option<UiNodeId> {
        if !check_if(modifiers, host) {
            return None;
        }

        let id = self.tree.create_styled_node(name.to_string(), modifiers);
        apply_events(&mut self.tree, id, attributes);
        apply_svg(&mut self.tree, id, attributes);

        match find_for(modifiers) {
            // `for{var in iterable}` on this node: build `children` once
            // per item, with `var` bound in a fresh scope each time,
            // instead of once. A non-list value (e.g. a bare number)
            // still renders once rather than silently vanishing -- there's
            // nothing to iterate, but the modifier wasn't a no-op either.
            Some((var, iterable_expr)) => {
                let iterable = host.eval_expr(iterable_expr);
                let items = match iterable {
                    EvalValue::List(items) => items,
                    other => vec![other],
                };

                for item in items {
                    host.push_scope();
                    host.define_var(&var, item);
                    self.build_children(id, children, host);
                    host.pop_scope();
                }
            }
            None => {
                self.build_children(id, children, host);
            }
        }

        Some(id)
    }

    fn build_children<H: EvalHost>(
        &mut self,
        parent: UiNodeId,
        children: &[UiNodeOrExpr],
        host: &mut H,
    ) {
        for child in children {
            match child {
                UiNodeOrExpr::Node(n) => {
                    if let Some(cid) = self.build(n, host) {
                        self.tree.add_child(parent, cid);
                    }
                }
                UiNodeOrExpr::Text(text) => {
                    let tid = self.tree.create_text_node(render_ui_text(text, host));
                    self.tree.add_child(parent, tid);
                }
            }
        }
    }

    pub fn finish(self) -> UiTree {
        self.tree
    }
}

/// Finds this node's `if{cond}` modifier, if any, evaluates it through
/// `host`, and reports whether the node should be built at all. Only an
/// explicit `false` (or `()`/unit, e.g. an unresolved variable) hides the
/// node; a node with no `if{}` modifier always renders.
fn check_if<H: EvalHost>(modifiers: &[UiModifier], host: &mut H) -> bool {
    for m in modifiers {
        if m.path.len() == 1 && m.path[0] == "if" {
            if let UiModifierValue::Expr(cond) = &m.value {
                let v = host.eval_expr(cond);
                return !matches!(v, EvalValue::Bool(false) | EvalValue::Unit);
            }
        }
    }
    true
}

/// Finds this node's `for{var in iterable}` modifier, if any. The parser
/// stores it as `MiniMod{key: [var], value: Expr(iterable)}` (see
/// tint-parser/src/ui/{block,xml}.rs) -- same shape as any other nested
/// modifier, just repurposed to carry the loop variable's name alongside
/// its expression.
fn find_for(modifiers: &[UiModifier]) -> Option<(String, &Expr)> {
    modifiers.iter().find_map(|m| {
        if m.path.len() != 1 || m.path[0] != "for" {
            return None;
        }
        if let UiModifierValue::MiniMod { key, value } = &m.value {
            if let UiModifierValue::Expr(e) = value.as_ref() {
                return Some((key.first()?.clone(), e));
            }
        }
        None
    })
}

/// Reads this node's `click||`/`hover_in||`/`hover_out||` attributes (a
/// bare `handler_name`, i.e. `UiAttrValue::Ident`) and records them on
/// the tree node, so the render tree carries real handler names instead
/// of silently dropping them -- previously the only place an attribute
/// like this got read at all was `UiPreviewNode.svelte`'s now-dead
/// `node.attrs`/`node.onclick`, which nothing on the Rust side ever
/// populated.
fn apply_events(tree: &mut UiTree, id: UiNodeId, attributes: &[UiAttribute]) {
    tree.set_events(
        id,
        find_handler(attributes, "click"),
        find_handler(attributes, "hover_in"),
        find_handler(attributes, "hover_out"),
    );
}

fn find_handler(attributes: &[UiAttribute], name: &str) -> Option<String> {
    attributes.iter().find_map(|a| {
        if a.name != name {
            return None;
        }
        match &a.value {
            UiAttrValue::Ident(handler) => Some(handler.clone()),
            _ => None,
        }
    })
}

/// Reads this node's `svg||"<svg ...>...</svg>"` attribute (a
/// `UiAttrValue::Literal` -- a real Tint string, not an identifier like
/// `click||handler`) and records the raw markup on the tree node, so a
/// renderer can show exactly the SVG the .tint source wrote, verbatim.
fn apply_svg(tree: &mut UiTree, id: UiNodeId, attributes: &[UiAttribute]) {
    tree.set_svg(id, find_literal(attributes, "svg"));
}

fn find_literal(attributes: &[UiAttribute], name: &str) -> Option<String> {
    attributes.iter().find_map(|a| {
        if a.name != name {
            return None;
        }
        match &a.value {
            UiAttrValue::Literal(s) => Some(s.clone()),
            _ => None,
        }
    })
}

/// Text for a `Text` AST node: literal parts are used verbatim, and an
/// interpolation is actually evaluated through `host` and stringified
/// (`Value`'s `Display` impl) -- e.g. inside a `for{tx in ...}`, `"{tx}"`
/// now renders the real per-iteration value instead of the literal
/// characters `{tx}`. An expression that evaluates to nothing in scope
/// (undefined variable) comes back as `Value::Unit`, which displays as
/// `()` rather than panicking -- a visibly-incomplete render, not a crash.
fn render_ui_text<H: EvalHost>(text: &UiText, host: &mut H) -> String {
    let mut out = String::new();
    for part in &text.parts {
        match part {
            UiTextPart::Literal(s, _) => out.push_str(s),
            UiTextPart::Interpolation(expr, _) => {
                let v = host.eval_expr(expr);
                out.push_str(&v.to_string());
            }
        }
    }
    out
}
