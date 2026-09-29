// Existence/duplicate-binding checks over a `ui fn` body -- the UI-tree
// counterpart to `statements.rs`/`expressions.rs`'s logic-code walk. Same
// scope as everywhere else in this checker: is every identifier actually
// in scope, are there no duplicate bindings, and do the typed UI control-flow
// and event expressions make sense. Host-specific styling values remain
// intentionally open (a `click||handler`
// name, a `case label { ... }` arm's label, a modifier path like
// `padding`/`hover` are all left alone here, exactly as they are at
// runtime until something actually evaluates them).
impl SemanticChecker {
    fn visit_ui_fn(&mut self, f: &UiFnDecl) {
        self.ctx.mode = Mode::UI;
        self.ui_tokens = self.external_context.ui_tokens.clone();
        self.ui_styles = self.external_context.ui_styles.clone();
        self.ui_variants = self.external_context.ui_variants.clone();
        for node in &f.body {
            self.collect_ui_tokens(node);
            self.collect_ui_styles(node);
            self.collect_ui_variants(node);
        }
        self.scopes.push();
        for param in &f.params {
            let ty = self.param_type(&f.name, param);
            self.bind_pattern_typed(&param.pattern, ty);
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
            let state_ty = self.infer_expr(&state.init);
            if let Some(declared) = self.scopes.lookup(&state.name) {
                self.require_compatible(&declared, &state_ty, state.init.span());
            }
        }
        for node in &f.body {
            self.visit_ui_style_declarations(node);
        }
        for node in &f.body {
            self.visit_ui_node(node);
        }
        self.scopes.pop();
        self.ctx.mode = Mode::Logic;
    }

    fn visit_ui_node(&mut self, node: &UiNode) {
        match node {
            UiNode::BlockElement {
                attributes,
                modifiers,
                children,
                ..
            } => {
                self.visit_ui_attributes(attributes);
                self.visit_ui_modifiers(modifiers);
                self.visit_ui_children_with_for(modifiers, children);
            }

            UiNode::BlockSelfClosing { modifiers, .. } => {
                self.visit_ui_modifiers(modifiers);
            }

            UiNode::Style { .. } => {}

            UiNode::Component {
                modifiers,
                children,
                ..
            } => {
                self.visit_ui_modifiers(modifiers);
                for child in children {
                    self.visit_ui_node_or_expr(child);
                }
            }

            UiNode::Variant {
                modifiers,
                children,
                ..
            } => {
                self.visit_ui_modifiers(modifiers);
                for child in children {
                    self.visit_ui_node_or_expr(child);
                }
            }

            UiNode::Slot { children, .. } => {
                for child in children {
                    self.visit_ui_node_or_expr(child);
                }
            }

            UiNode::Theme { children, .. } => {
                for child in children {
                    self.visit_ui_node_or_expr(child);
                }
            }

            // Standalone `for { var in iterable } { body }` -- the
            // iterable is visited in the OUTER scope (mirrors the `for{}`
            // modifier's iterable, visited by `visit_ui_modifiers` before
            // `visit_ui_children_with_for` binds `var`), then `var` is
            // bound only for `body`.
            UiNode::For {
                var,
                iterable,
                body,
                ..
            } => {
                let iterable_ty = self.infer_expr(iterable);
                self.scopes.push();
                let item_ty = self.iterable_item(&iterable_ty, iterable.span());
                self.scopes.define(var, item_ty);
                for child in body {
                    self.visit_ui_node_or_expr(child);
                }
                self.scopes.pop();
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
                let item = self.for_item.take().unwrap_or(Type::Unknown);
                self.scopes.push();
                self.scopes.define(&var, item);
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

    /// Element type of something iterated by `for`; an open type is taken to
    /// be a list.
    fn iterable_item(&mut self, iterable: &Type, span: Span) -> Type {
        match self.shallow(iterable) {
            Type::Array(item) => *item,
            Type::Var(_) => {
                let item = self.fresh(span, "the element type of a loop");
                self.unify(iterable, &Type::Array(Box::new(item.clone())));
                item
            }
            Type::Unknown => Type::Unknown,
            found => {
                let found = self.resolve(&found);
                self.error(
                    span,
                    SemanticErrorKind::TypeMismatch {
                        expected: "array".into(),
                        found: type_name(&found),
                    },
                );
                Type::Unknown
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
                // `click||handler_name`, a `case`-arm's label, `svg||"..."`,
                // `route||"..."` -- a bare identifier or literal, resolved
                // dynamically at runtime (against `TintVM.logic_functions`
                // or a case comparison), never a variable read this pass
                // could meaningfully check.
                UiAttrValue::Literal(_) => {}
                UiAttrValue::Ident(name) => {
                    // Host-invoked handlers: the argument types are fixed by
                    // the runtime (`frame||` gets `dt`, key events get the
                    // key name).
                    let args = match attr.name.as_str() {
                        "frame" => Some(vec![Type::Number]),
                        "key_down" | "key_up" => Some(vec![Type::String]),
                        "click" | "pointer_down" | "pointer_up" | "hover_in" | "hover_out"
                        | "tick" => Some(Vec::new()),
                        _ => None,
                    };
                    if let (Some(args), Some((params, _))) = (args, self.fn_types.get(name).cloned()) {
                        if params.len() == args.len() {
                            for (param, arg) in params.iter().zip(&args) {
                                self.require_compatible(param, arg, attr.span);
                            }
                        }
                    }
                    if matches!(
                        attr.name.as_str(),
                        "click"
                            | "pointer_down"
                            | "pointer_up"
                            | "hover_in"
                            | "hover_out"
                            | "frame"
                    ) && !self.known_fns.contains(name)
                    {
                        self.error(attr.span, SemanticErrorKind::UnknownIdent(name.clone()));
                    }
                }
            }
        }
    }

    fn visit_ui_modifiers(&mut self, modifiers: &[UiModifier]) {
        for m in modifiers {
            if m.path == ["use".to_string()] {
                if let UiModifierValue::Ident(name) = &m.value {
                    if !self.ui_styles.contains(name) {
                        self.error(m.span, SemanticErrorKind::UnknownUiStyle(name.clone()));
                    }
                }
            }
            if m.path == ["variant".to_string()] {
                if let UiModifierValue::Ident(name) = &m.value {
                    if !self.ui_variants.contains(name) {
                        self.error(m.span, SemanticErrorKind::UnknownUiVariant(name.clone()));
                    }
                }
            }
            if m.path == ["if".to_string()] {
                if let UiModifierValue::Expr(expr) = &m.value {
                    let ty = self.infer_expr(expr);
                    self.require_compatible(&Type::Bool, &ty, expr.span());
                }
            }
            if m.path == ["for".to_string()] {
                if let UiModifierValue::MiniMod { value, .. } = &m.value {
                    if let UiModifierValue::Expr(expr) = value.as_ref() {
                        let ty = self.infer_expr(expr);
                        self.for_item = Some(self.iterable_item(&ty, expr.span()));
                    }
                }
            }
            self.check_ui_token_refs(&m.value, m.span);
            self.visit_ui_modifier_value(&m.value);
        }
    }

    fn collect_ui_tokens(&mut self, node: &UiNode) {
        match node {
            UiNode::Theme { children, .. } => {
                for child in children {
                    if let UiNodeOrExpr::Node(node) = child {
                        self.collect_ui_tokens(node);
                    }
                }
            }
            UiNode::BlockElement {
                name,
                modifiers,
                children,
                ..
            } => {
                if name == "tokens" {
                    for modifier in modifiers {
                        if !modifier.path.is_empty() {
                            self.ui_tokens.insert(modifier.path.join("."));
                        }
                    }
                }
                for child in children {
                    if let UiNodeOrExpr::Node(node) = child {
                        self.collect_ui_tokens(node);
                    }
                }
            }
            UiNode::BlockSelfClosing { .. } => {}
            UiNode::Style { .. } => {}
            UiNode::Component { children, .. }
            | UiNode::Variant { children, .. }
            | UiNode::Slot { children, .. } => {
                for child in children {
                    if let UiNodeOrExpr::Node(node) = child {
                        self.collect_ui_tokens(node);
                    }
                }
            }
            UiNode::For { body, .. } => {
                for child in body {
                    if let UiNodeOrExpr::Node(node) = child {
                        self.collect_ui_tokens(node);
                    }
                }
            }
        }
    }

    fn collect_ui_styles(&mut self, node: &UiNode) {
        match node {
            UiNode::Style { name, .. } => {
                self.ui_styles.insert(name.clone());
            }
            UiNode::Theme { children, .. }
            | UiNode::BlockElement { children, .. }
            | UiNode::Component { children, .. }
            | UiNode::Variant { children, .. }
            | UiNode::Slot { children, .. } => {
                for child in children {
                    if let UiNodeOrExpr::Node(node) = child {
                        self.collect_ui_styles(node);
                    }
                }
            }
            UiNode::For { body, .. } => {
                for child in body {
                    if let UiNodeOrExpr::Node(node) = child {
                        self.collect_ui_styles(node);
                    }
                }
            }
            UiNode::BlockSelfClosing { .. } => {}
        }
    }

    fn collect_ui_variants(&mut self, node: &UiNode) {
        match node {
            UiNode::Variant { name, children, .. } => {
                self.ui_variants.insert(name.clone());
                for child in children {
                    if let UiNodeOrExpr::Node(node) = child {
                        self.collect_ui_variants(node);
                    }
                }
            }
            UiNode::Theme { children, .. }
            | UiNode::BlockElement { children, .. }
            | UiNode::Component { children, .. }
            | UiNode::Slot { children, .. } => {
                for child in children {
                    if let UiNodeOrExpr::Node(node) = child {
                        self.collect_ui_variants(node);
                    }
                }
            }
            UiNode::For { body, .. } => {
                for child in body {
                    if let UiNodeOrExpr::Node(node) = child {
                        self.collect_ui_variants(node);
                    }
                }
            }
            UiNode::Style { .. } | UiNode::BlockSelfClosing { .. } => {}
        }
    }

    fn visit_ui_style_declarations(&mut self, node: &UiNode) {
        match node {
            UiNode::Style { modifiers, .. } => self.visit_ui_modifiers(modifiers),
            UiNode::Theme { children, .. }
            | UiNode::BlockElement { children, .. }
            | UiNode::Component { children, .. }
            | UiNode::Variant { children, .. }
            | UiNode::Slot { children, .. } => {
                for child in children {
                    if let UiNodeOrExpr::Node(node) = child {
                        self.visit_ui_style_declarations(node);
                    }
                }
            }
            UiNode::For { body, .. } => {
                for child in body {
                    if let UiNodeOrExpr::Node(node) = child {
                        self.visit_ui_style_declarations(node);
                    }
                }
            }
            UiNode::BlockSelfClosing { .. } => {}
        }
    }

    fn check_ui_token_refs(&mut self, value: &UiModifierValue, span: Span) {
        match value {
            UiModifierValue::Ident(name) if name.starts_with('@') => {
                let token = &name[1..];
                if !self.ui_tokens.contains(token) {
                    self.error(span, SemanticErrorKind::UnknownUiToken(token.to_string()));
                }
            }
            UiModifierValue::Block(items) => {
                for item in items {
                    self.check_ui_token_refs(&item.value, item.span);
                }
            }
            UiModifierValue::Tuple(items) => {
                for item in items {
                    self.check_ui_token_refs(item, span);
                }
            }
            UiModifierValue::MiniMod { value, .. } => {
                self.check_ui_token_refs(value, span);
            }
            _ => {}
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
