use super::helpers::*;
use super::*;

impl UiBuilder {
    pub(super) fn build_into<H: EvalHost>(
        &mut self,
        parent: UiNodeId,
        node: &UiNode,
        host: &mut H,
    ) {
        match node {
            UiNode::Theme { name, children, .. } => {
                if theme_matches(name, host) {
                    self.build_children(parent, children, host);
                }
            }

            // `children { ... }` -- a grouping tag for separating a
            // node's own children from its modifiers when they'd
            // otherwise be hard to tell apart at a glance (several
            // modifiers followed by several structural children). Same
            // "splice into parent, no wrapper node of its own" shape as
            // `Theme`/`For` above and below -- it's an ordinary tag as
            // far as the parser is concerned (no grammar change needed to
            // support it), just one the builder recognizes by name and
            // never wraps.
            //
            // RESERVED NAME: "children" is matched by name alone, with no
            // way to opt out -- a user component actually named `children`
            // (e.g. `children { padding::8 "x" }` meant as an ordinary
            // tag) would silently splice its own children into the parent
            // instead of rendering as its own node, the same way naming a
            // component `case` right before a control-flow keyword would
            // collide with that grammar. Documented in
            // docs/ui/ui-syntax.md's "Grouping children" section; not
            // otherwise guarded against (no semantic-checker warning) --
            // matches this project's existing scope for that checker
            // (existence/duplicate-binding only, see
            // tint-semantics/src/checker/ui.rs).
            UiNode::BlockElement { name, children, .. } if name == "children" => {
                self.build_children(parent, children, host);
            }

            // Standalone `for { var in iterable } { body }`: unlike the
            // `for{}` modifier (`find_for` below, which repeats a single
            // node's own children into that same node), this has no node
            // of its own -- each iteration's `body` is built straight into
            // `parent`, so it can sit among static siblings.
            UiNode::For {
                var,
                iterable,
                body,
                ..
            } => {
                let value = host.eval_expr(iterable);
                let items = match value {
                    EvalValue::List(items) => items,
                    other => vec![other],
                };

                for item in items {
                    host.push_scope();
                    host.define_var(var, item);
                    self.build_children(parent, body, host);
                    host.pop_scope();
                }
            }

            _ => {
                if let Some(id) = self.build(node, host) {
                    self.tree.add_child(parent, id);
                }
            }
        }
    }

    pub(super) fn build_container<H: EvalHost>(
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

        let id = self
            .tree
            .create_styled_node_with_host(name.to_string(), modifiers, host);
        apply_events(&mut self.tree, id, attributes);
        apply_svg(&mut self.tree, id, attributes);
        apply_route(&mut self.tree, id, attributes);
        apply_asset(&mut self.tree, id, attributes);
        apply_key(&mut self.tree, id, modifiers);

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
            None => match find_match(modifiers) {
                // `match{scrutinee}` on this node: only the first
                // `case`-tagged child whose label matches (or `_`) gets
                // built, spliced straight into `id` -- the `case { ... }`
                // arm itself never appears in the render tree, only its
                // own children do. A child that isn't `case`-tagged
                // (rare, but not forbidden) always builds, same as if
                // there were no `match{}` at all.
                Some(scrutinee_expr) => {
                    let scrutinee = host.eval_expr(scrutinee_expr);
                    let mut matched = false;

                    for child in children {
                        let UiNodeOrExpr::Node(node) = child else {
                            self.build_children(id, std::slice::from_ref(child), host);
                            continue;
                        };
                        let (case_children, case_attrs) = match node {
                            UiNode::BlockElement {
                                children,
                                attributes,
                                ..
                            } => (Some(children), Some(attributes)),
                            _ => (None, None),
                        };
                        let Some(label) = case_attrs.and_then(|a| find_case_label(a)) else {
                            // Not a `case`-tagged child at all -- build it
                            // unconditionally, same as a plain sibling.
                            self.build_into(id, node, host);
                            continue;
                        };
                        if !matched && case_label_matches(&label, &scrutinee) {
                            matched = true;
                            if let Some(case_children) = case_children {
                                self.build_children(id, case_children, host);
                            }
                        }
                    }
                }
                None => {
                    self.build_children(id, children, host);
                }
            },
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
                    self.build_into(parent, n, host);
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
