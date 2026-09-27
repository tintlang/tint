use super::helpers::*;
use super::*;

impl UiBuilder {
    pub(super) fn build_into<H: EvalHost>(
        &mut self,
        parent: UiNodeId,
        node: &UiNode,
        host: &mut H,
    ) {
        if let UiNode::Theme { name, children, .. } = node {
            if theme_matches(name, host) {
                for child in children {
                    match child {
                        UiNodeOrExpr::Node(child) => self.build_into(parent, child, host),
                        UiNodeOrExpr::Text(text) => {
                            let id = self.tree.create_text_node(render_ui_text(text, host));
                            self.tree.add_child(parent, id);
                        }
                    }
                }
            }
        } else if let Some(id) = self.build(node, host) {
            self.tree.add_child(parent, id);
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

        let id = self.tree.create_styled_node(name.to_string(), modifiers);
        apply_events(&mut self.tree, id, attributes);
        apply_svg(&mut self.tree, id, attributes);
        apply_route(&mut self.tree, id, attributes);

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
                // built, spliced straight into `id` -- the `<case ...>`
                // wrapper itself never appears in the render tree, only
                // its own children do. A child that isn't `case`-tagged
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
                            UiNode::Element {
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
