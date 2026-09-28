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
                    self.collect_theme_tokens(children);
                    self.build_children(parent, children, host);
                }
            }

            // `tokens { ... }` is a compile-time/runtime style declaration,
            // not a DOM element. It is consumed by the active theme above.
            UiNode::BlockElement { name, .. } if name == "tokens" => {}

            UiNode::Style { .. } => {}

            UiNode::Component { .. } => {}

            UiNode::Slot { .. } => {}

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

        if let Some(component) = self.components.get(name) {
            let component = component.clone();
            return self.build_component(name, attributes, modifiers, children, &component, host);
        }

        let expanded = self.expand_styles(modifiers);
        let id = self.tree.create_styled_node_with_host_and_tokens(
            name.to_string(),
            &expanded,
            host,
            &self.tokens,
        );
        apply_events(&mut self.tree, id, attributes);
        apply_svg(&mut self.tree, id, attributes);
        apply_route(&mut self.tree, id, attributes);
        apply_target(&mut self.tree, id, attributes);
        apply_reference(&mut self.tree, id, attributes);
        apply_js_handler(&mut self.tree, id, attributes);
        apply_asset(&mut self.tree, id, attributes);
        apply_key(&mut self.tree, id, &expanded);

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

    fn collect_theme_tokens(&mut self, children: &[UiNodeOrExpr]) {
        for child in children {
            let UiNodeOrExpr::Node(UiNode::BlockElement {
                name, modifiers, ..
            }) = child
            else {
                continue;
            };

            if name != "tokens" {
                continue;
            }

            for modifier in modifiers {
                if modifier.path.len() == 1 {
                    self.tokens
                        .insert(modifier.path[0].clone(), modifier.value.clone());
                }
            }
        }
    }

    pub(super) fn collect_styles(&mut self, nodes: &[UiNode]) {
        for node in nodes {
            match node {
                UiNode::Style {
                    name, modifiers, ..
                } => {
                    self.styles.insert(name.clone(), modifiers.clone());
                }
                UiNode::Component { children, .. } => {
                    self.collect_styles_from_children(children);
                }
                UiNode::Theme { children, .. } | UiNode::BlockElement { children, .. } => {
                    self.collect_styles_from_children(children);
                }
                UiNode::For { body, .. } => self.collect_styles_from_children(body),
                UiNode::Slot { children, .. } => self.collect_styles_from_children(children),
                UiNode::Variant { children, .. } => self.collect_styles_from_children(children),
                UiNode::BlockSelfClosing { .. } => {}
            }
        }
    }

    pub(super) fn collect_components(&mut self, nodes: &[UiNode]) {
        for node in nodes {
            match node {
                UiNode::Component {
                    name,
                    modifiers,
                    children,
                    ..
                } => {
                    let mut base_children = Vec::new();
                    let mut variants = HashMap::new();
                    for child in children {
                        match child {
                            UiNodeOrExpr::Node(UiNode::Variant {
                                name,
                                modifiers,
                                children,
                                ..
                            }) => {
                                variants
                                    .insert(name.clone(), (modifiers.clone(), children.clone()));
                            }
                            other => base_children.push(other.clone()),
                        }
                    }
                    self.components.insert(
                        name.clone(),
                        UiComponent {
                            modifiers: modifiers.clone(),
                            children: base_children,
                            variants,
                        },
                    );
                    self.collect_components_from_children(children);
                }
                UiNode::Theme { children, .. } | UiNode::BlockElement { children, .. } => {
                    self.collect_components_from_children(children);
                }
                UiNode::For { body, .. } => self.collect_components_from_children(body),
                UiNode::Style { .. } | UiNode::Variant { .. } | UiNode::BlockSelfClosing { .. } => {
                }
                UiNode::Slot { children, .. } => self.collect_components_from_children(children),
            }
        }
    }

    fn collect_components_from_children(&mut self, children: &[UiNodeOrExpr]) {
        for child in children {
            if let UiNodeOrExpr::Node(node) = child {
                self.collect_components(std::slice::from_ref(node));
            }
        }
    }

    fn build_component<H: EvalHost>(
        &mut self,
        name: &str,
        attributes: &[UiAttribute],
        invocation_modifiers: &[UiModifier],
        invocation_children: &[UiNodeOrExpr],
        component: &UiComponent,
        host: &mut H,
    ) -> Option<UiNodeId> {
        let variant_name = invocation_modifiers.iter().find_map(|modifier| {
            if modifier.path == ["variant".to_string()] {
                if let UiModifierValue::Ident(name) = &modifier.value {
                    return Some(name.clone());
                }
            }
            None
        });
        let variant = variant_name
            .as_ref()
            .and_then(|name| component.variants.get(name));

        let mut modifiers = component.modifiers.clone();
        if let Some((variant_modifiers, _)) = variant {
            modifiers.extend_from_slice(variant_modifiers);
        }
        modifiers.extend(
            invocation_modifiers
                .iter()
                .filter(|modifier| modifier.path != ["variant".to_string()])
                .cloned(),
        );
        let expanded = self.expand_styles(&modifiers);
        let id = self.tree.create_styled_node_with_host_and_tokens(
            name.to_string(),
            &expanded,
            host,
            &self.tokens,
        );
        apply_events(&mut self.tree, id, attributes);
        apply_svg(&mut self.tree, id, attributes);
        apply_route(&mut self.tree, id, attributes);
        apply_target(&mut self.tree, id, attributes);
        apply_reference(&mut self.tree, id, attributes);
        apply_js_handler(&mut self.tree, id, attributes);
        apply_asset(&mut self.tree, id, attributes);
        apply_key(&mut self.tree, id, &expanded);

        let mut named_slots: HashMap<String, Vec<UiNodeOrExpr>> = HashMap::new();
        let mut default_children = Vec::new();
        for child in invocation_children {
            match child {
                UiNodeOrExpr::Node(UiNode::Slot { name, children, .. }) => {
                    named_slots.insert(name.clone(), children.clone());
                }
                other => default_children.push(other.clone()),
            }
        }

        let mut template_children = component.children.clone();
        if let Some((_, variant_children)) = variant {
            template_children.extend(variant_children.clone());
        }

        for child in &template_children {
            match child {
                UiNodeOrExpr::Node(UiNode::Slot { name, .. }) => {
                    let content = named_slots.get(name).cloned().unwrap_or_else(|| {
                        if name == "content" {
                            default_children.clone()
                        } else {
                            Vec::new()
                        }
                    });
                    self.build_children(id, &content, host);
                }
                other => self.build_children(id, std::slice::from_ref(other), host),
            }
        }

        Some(id)
    }

    fn collect_styles_from_children(&mut self, children: &[UiNodeOrExpr]) {
        for child in children {
            if let UiNodeOrExpr::Node(node) = child {
                self.collect_styles(std::slice::from_ref(node));
            }
        }
    }

    pub(super) fn expand_styles(&self, modifiers: &[UiModifier]) -> Vec<UiModifier> {
        let mut out = Vec::new();
        self.expand_styles_into(modifiers, &mut out, &mut Vec::new());
        out
    }

    fn expand_styles_into(
        &self,
        modifiers: &[UiModifier],
        out: &mut Vec<UiModifier>,
        stack: &mut Vec<String>,
    ) {
        for modifier in modifiers {
            if modifier.path == ["use".to_string()] {
                if let UiModifierValue::Ident(name) = &modifier.value {
                    if stack.contains(name) {
                        continue;
                    }
                    if let Some(style) = self.styles.get(name) {
                        stack.push(name.clone());
                        self.expand_styles_into(style, out, stack);
                        stack.pop();
                    }
                }
            } else {
                out.push(modifier.clone());
            }
        }
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
