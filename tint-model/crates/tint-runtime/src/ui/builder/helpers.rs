use super::*;

pub(super) fn theme_matches(name: &str, host: &mut impl EvalHost) -> bool {
    match host.load_var("theme", Span::dummy()) {
        EvalValue::String(active) => active == name,
        _ => false,
    }
}

/// Finds this node's `if{cond}` modifier, if any, evaluates it through
/// `host`, and reports whether the node should be built at all. Only an
/// explicit `false` (or `()`/unit, e.g. an unresolved variable) hides the
/// node; a node with no `if{}` modifier always renders.
pub(super) fn check_if<H: EvalHost>(modifiers: &[UiModifier], host: &mut H) -> bool {
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
pub(super) fn find_for(modifiers: &[UiModifier]) -> Option<(String, &Expr)> {
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
pub(super) fn apply_events(tree: &mut UiTree, id: UiNodeId, attributes: &[UiAttribute]) {
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
pub(super) fn apply_svg(tree: &mut UiTree, id: UiNodeId, attributes: &[UiAttribute]) {
    tree.set_svg(id, find_literal(attributes, "svg"));
}

/// Reads `route||"/path"` and leaves navigation as a renderer concern.
/// Keeping this as a first-class field makes the syntax portable to DOM,
/// native, and future renderers without treating the route as CSS.
pub(super) fn apply_route(tree: &mut UiTree, id: UiNodeId, attributes: &[UiAttribute]) {
    tree.set_route(id, find_literal(attributes, "route"));
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
pub(super) fn render_ui_text<H: EvalHost>(text: &UiText, host: &mut H) -> String {
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
