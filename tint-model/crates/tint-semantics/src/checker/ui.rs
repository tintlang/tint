// Existence/duplicate-binding checks over a `ui fn` body -- the UI-tree
// counterpart to `statements.rs`/`expressions.rs`'s logic-code walk. Same
// scope as everywhere else in this checker: is every identifier actually
// in scope, are there no duplicate bindings -- NOT type checking, and NOT
// resolution of anything that needs a value's type (a `click||handler`
// name, a `<case label>`, a modifier path like `padding`/`hover` are all
// left alone here, exactly as they are at runtime until something actually
// evaluates them).
impl SemanticChecker {
    fn visit_ui_fn(&mut self, f: &UiFnDecl) {
        self.ctx.mode = Mode::UI;
        self.scopes.push();
        for param in &f.params {
            self.bind_pattern(&param.pattern);
        }
        // The `state` names themselves are already bound into the
        // PERSISTENT base scope by `collect_top_level_names` (see that
        // function's doc comment -- a plain top-level `fn` used as a
        // `click||`/`hover_in||` handler needs to see them too, matching
        // the real `UiSession` runtime model, so they can't live in the
        // scope frame this function pushes and pops). What nothing has
        // visited yet is each state's own INIT expression -- `state count
        // = starting_value` should still flag an unknown `starting_value`.
        for state in &f.state {
            self.visit_expr(&state.init);
        }
        for node in &f.body {
            self.visit_ui_node(node);
        }
        self.scopes.pop();
        self.ctx.mode = Mode::Logic;
    }

    fn visit_ui_node(&mut self, node: &UiNode) {
        match node {
            UiNode::Element {
                attributes,
                modifiers,
                children,
                ..
            }
            | UiNode::BlockElement {
                attributes,
                modifiers,
                children,
                ..
            } => {
                self.visit_ui_attributes(attributes);
                self.visit_ui_modifiers(modifiers);
                self.visit_ui_children_with_for(modifiers, children);
            }

            UiNode::SelfClosing {
                attributes,
                modifiers,
                ..
            } => {
                self.visit_ui_attributes(attributes);
                self.visit_ui_modifiers(modifiers);
            }

            UiNode::BlockSelfClosing { modifiers, .. } => {
                self.visit_ui_modifiers(modifiers);
            }

            UiNode::Theme { children, .. } => {
                for child in children {
                    self.visit_ui_node_or_expr(child);
                }
            }
        }
    }

    // `for{var in iterable}` (see `tint-runtime/src/ui/builder/helpers.rs`'s
    // `find_for`, which this mirrors) binds `var` for the DURATION of this
    // node's children only -- not the node's own other modifiers (the
    // iterable expression itself is evaluated in the outer scope, already
    // covered by `visit_ui_modifiers` above) and not anything outside this
    // node. Without this, `for{tx in [...]} Text { "{tx}" }` would flag
    // `tx` as unknown the moment the child's interpolation is visited.
    fn visit_ui_children_with_for(&mut self, modifiers: &[UiModifier], children: &[UiNodeOrExpr]) {
        match find_for_loop_var(modifiers) {
            Some(var) => {
                self.scopes.push();
                self.scopes.define(&var, Type::Unit);
                for child in children {
                    self.visit_ui_node_or_expr(child);
                }
                self.scopes.pop();
            }
            None => {
                for child in children {
                    self.visit_ui_node_or_expr(child);
                }
            }
        }
    }

    fn visit_ui_node_or_expr(&mut self, node: &UiNodeOrExpr) {
        match node {
            UiNodeOrExpr::Node(n) => self.visit_ui_node(n),
            UiNodeOrExpr::Text(t) => self.visit_ui_text(t),
        }
    }

    fn visit_ui_text(&mut self, text: &UiText) {
        for part in &text.parts {
            if let UiTextPart::Interpolation(expr, _) = part {
                self.visit_expr(expr);
            }
        }
    }

    fn visit_ui_attributes(&mut self, attributes: &[UiAttribute]) {
        for attr in attributes {
            match &attr.value {
                UiAttrValue::Expr(e) => self.visit_expr(e),
                UiAttrValue::Modifier(block) => self.visit_ui_modifier_block(block),
                // `click||handler_name`, `<case label>`, `svg||"..."`,
                // `route||"..."` -- a bare identifier or literal, resolved
                // dynamically at runtime (against `TintVM.logic_functions`
                // or a case comparison), never a variable read this pass
                // could meaningfully check.
                UiAttrValue::Literal(_) | UiAttrValue::Ident(_) => {}
            }
        }
    }

    fn visit_ui_modifiers(&mut self, modifiers: &[UiModifier]) {
        for m in modifiers {
            self.visit_ui_modifier_value(&m.value);
        }
    }

    fn visit_ui_modifier_value(&mut self, value: &UiModifierValue) {
        match value {
            UiModifierValue::Expr(e) => self.visit_expr(e),
            UiModifierValue::Block(mods) => self.visit_ui_modifiers(mods),
            UiModifierValue::Tuple(items) => {
                for item in items {
                    self.visit_ui_modifier_value(item);
                }
            }
            // `for{var in expr}`'s own shape (`MiniMod{key: [var], value:
            // Expr(iterable)}`) -- the iterable expression still needs
            // visiting here (in the OUTER scope, before `var` exists);
            // binding `var` for the children is `visit_ui_children_with_for`'s
            // job, not this generic modifier walk's.
            UiModifierValue::MiniMod { value, .. } => self.visit_ui_modifier_value(value),
            UiModifierValue::Number(_)
            | UiModifierValue::String(_)
            | UiModifierValue::Range(_, _)
            | UiModifierValue::Ident(_) => {}
        }
    }

    fn visit_ui_modifier_block(&mut self, block: &UiModifierBlock) {
        self.visit_ui_modifier_items(&block.items);
    }

    fn visit_ui_modifier_items(&mut self, items: &[UiModifierItem]) {
        for item in items {
            if let Some(value) = &item.value {
                self.visit_ui_modifier_value(value);
            }
            self.visit_ui_modifier_items(&item.children);
        }
    }
}

/// Finds a `for{var in iterable}` modifier among `modifiers`, if any, and
/// returns just the loop variable's name -- a checker-local mirror of
/// `tint-runtime/src/ui/builder/helpers.rs`'s `find_for` (that one also
/// returns the iterable `Expr`, which this pass already visits generically
/// via `visit_ui_modifier_value`, so only the variable name is needed
/// here). Duplicated rather than shared because `tint-semantics` has no
/// dependency on `tint-runtime` (and shouldn't gain one just for this) --
/// both read the exact same `UiModifierValue::MiniMod{key, value}` shape
/// the parser produces, so they can't drift on what `for{}` parses to
/// without both being updated together.
fn find_for_loop_var(modifiers: &[UiModifier]) -> Option<String> {
    modifiers.iter().find_map(|m| {
        if m.path.len() != 1 || m.path[0] != "for" {
            return None;
        }
        if let UiModifierValue::MiniMod { key, value } = &m.value {
            if matches!(value.as_ref(), UiModifierValue::Expr(_)) {
                return key.first().cloned();
            }
        }
        None
    })
}
