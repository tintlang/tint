// ui/style.rs
//
// Resolves a UI node's `key::value` modifiers (tint_ast::UiModifier) into
// plain CSS property/value pairs a renderer can apply directly -- as an
// inline `style` attribute for the base list, and as a generated
// `:hover { ... }` rule for the hover list (a nested `hover::{ ... }`
// modifier is the language-level way to express what the sandbox used to
// fake with hardcoded, tag-name-based CSS -- see sandbox/src/components/
// UiPreviewNode.svelte's TAG_DEFAULTS comment for the effects this is
// meant to let Tint code state explicitly instead).
//
// Only LITERAL modifier values resolve here (numbers, idents, strings,
// and nested tuples/mini-mods of those). A modifier whose value is an
// arbitrary expression (`UiModifierValue::Expr`, e.g. a variable read, or
// what `if{}`/`for{}` carry) is skipped: resolving those needs the
// running VM's state/evaluator, which this pure AST-level pass over
// `UiNode::modifiers` doesn't have access to (see builder.rs). That's a
// separate, later piece of work, not a limitation of this resolver.

use std::collections::HashMap;
use tint_ast::{UiModifier, UiModifierValue};
use tint_evaluator::{EvalHost, Value as EvalValue};

/// Reconstruct the Tint modifier syntax for browser Elements inspection.
pub fn format_modifier_source(modifiers: &[UiModifier]) -> String {
    modifiers
        .iter()
        .map(format_modifier)
        .collect::<Vec<_>>()
        .join(" ")
}

fn format_modifier(modifier: &UiModifier) -> String {
    format!(
        "{}::{}",
        modifier.path.join("."),
        format_modifier_value(&modifier.value)
    )
}

fn format_modifier_value(value: &UiModifierValue) -> String {
    match value {
        UiModifierValue::Number(n) => format!("{}", n),
        UiModifierValue::String(s) => format!("\"{}\"", s.replace('"', "\\\"")),
        UiModifierValue::Ident(s) => s.clone(),
        UiModifierValue::Expr(_) => "<expr>".to_string(),
        UiModifierValue::Range(a, b) => format!("{}..{}", a, b),
        UiModifierValue::Block(items) => format!(
            "{{ {} }}",
            items
                .iter()
                .map(format_modifier)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        UiModifierValue::Tuple(items) => format!(
            "{{ {} }}",
            items
                .iter()
                .map(|item| match item {
                    UiModifierValue::MiniMod { key, value } =>
                        format!("{}::{}", key.join("."), format_modifier_value(value)),
                    other => format_modifier_value(other),
                })
                .collect::<Vec<_>>()
                .join(", ")
        ),
        UiModifierValue::MiniMod { key, value } => {
            format!("{}::{}", key.join("."), format_modifier_value(value))
        }
    }
}

/// Flat list of resolved CSS `(property, value)` pairs, in the order the
/// source modifiers were written.
pub type StyleList = Vec<(String, String)>;

/// `Column` and `Row` are semantic layout containers, so they must be flex
/// containers for child spacing (`gap`) and their conventional axis to work.
/// An explicit `direction::...` or `display::...` modifier always wins.
pub fn apply_container_defaults(tag: &str, style: &mut StyleList) {
    if style
        .iter()
        .any(|(property, _)| property == "display" || property == "flex-direction")
    {
        return;
    }

    let direction = match tag {
        "Column" => "column",
        "Row" => "row",
        _ => return,
    };

    style.push(("display".to_string(), "flex".to_string()));
    style.push(("flex-direction".to_string(), direction.to_string()));
}

/// Screen-size names a `mobile::{...}`/`tablet::{...}`/`laptop::{...}`/
/// `desktop::{...}` modifier can use -- same nested-modifier shape as
/// `hover::{...}` (a `key::{ sub_key::value, ... }` block), just keyed by
/// viewport width instead of pointer state. The actual pixel ranges each
/// name maps to live with the renderer that has a real viewport to
/// measure (tint-wasm's `DomSession`), not here -- this module only
/// resolves modifiers into style lists, it has no notion of "the current
/// window size".
const BREAKPOINT_NAMES: [&str; 4] = ["mobile", "tablet", "laptop", "desktop"];

/// A named screen size (`mobile`, ...) or a width threshold: `max-560` applies
/// while the viewport is at most 560px wide, `min-800` from 800px up. Matching
/// blocks are layered over the base style in source order, like CSS media
/// queries, so write `max-*` blocks from the largest to the smallest.
pub fn is_breakpoint_name(name: &str) -> bool {
    BREAKPOINT_NAMES.contains(&name) || breakpoint_threshold(name).is_some()
}

/// `max-560` -> `Some((false, 560))`, `min-800` -> `Some((true, 800))`.
pub fn breakpoint_threshold(name: &str) -> Option<(bool, u32)> {
    let (is_min, digits) = match name.strip_prefix("max-") {
        Some(rest) => (false, rest),
        None => (true, name.strip_prefix("min-")?),
    };
    digits.parse().ok().map(|width| (is_min, width))
}

/// Splits a node's modifiers into: its base style, its `hover::{...}`
/// style (if any), and one resolved `StyleList` per breakpoint name that
/// was actually used (`mobile::{...}` etc. -- see `BREAKPOINT_NAMES`).
/// The breakpoint list only contains entries for names the source
/// actually wrote; a node with no `mobile::{...}` etc. gets an empty
/// `Vec`, same "don't invent what wasn't asked for" rule as `hover_style`.
pub fn resolve_style(modifiers: &[UiModifier]) -> (StyleList, StyleList, Vec<(String, StyleList)>) {
    let mut style = Vec::new();
    let mut hover_style = Vec::new();
    let mut breakpoints: Vec<(String, StyleList)> = Vec::new();

    for m in modifiers {
        if is_style_group(&m.path) {
            if let UiModifierValue::Tuple(items) = &m.value {
                for item in items {
                    if let UiModifierValue::MiniMod { key, value } = item {
                        if m.path[0] == "motion" && key.len() == 1 && key[0] == "hover" {
                            apply_nested_style(value, &mut hover_style);
                            continue;
                        }
                        let key = grouped_property(&m.path[0], key);
                        apply_property(&key, value, &mut style);
                    }
                }
            }
            continue;
        }

        if m.path.len() == 1 && m.path[0] == "hover" {
            if let UiModifierValue::Tuple(items) = &m.value {
                apply_group_items("motion", items, &mut hover_style);
            }
            continue;
        }

        if m.path.len() == 1 && is_breakpoint_name(&m.path[0]) {
            let mut bp_style = Vec::new();
            if let UiModifierValue::Tuple(items) = &m.value {
                apply_group_items("layout", items, &mut bp_style);
            }
            breakpoints.push((m.path[0].clone(), bp_style));
            continue;
        }

        apply_property(&m.path, &m.value, &mut style);
    }

    (style, hover_style, breakpoints)
}

/// Resolves style expressions against the live UI scope.  Literal styles keep
/// using `resolve_style`; this variant is used by the stateful UI builder so a
/// game can bind `left::{ball_px}`/`top::{ball_py}` without rebuilding a grid
/// cell for every movement.
pub fn resolve_style_with_host<H: EvalHost>(
    modifiers: &[UiModifier],
    host: &mut H,
) -> (StyleList, StyleList, Vec<(String, StyleList)>) {
    resolve_style_with_host_and_tokens(modifiers, host, &HashMap::new())
}

pub fn resolve_style_with_host_and_tokens<H: EvalHost>(
    modifiers: &[UiModifier],
    host: &mut H,
    tokens: &HashMap<String, UiModifierValue>,
) -> (StyleList, StyleList, Vec<(String, StyleList)>) {
    resolve_style_with_eval(modifiers, &mut |expr| host.eval_expr(expr), tokens)
}

/// Like `resolve_style_with_host_and_tokens`, with the expression evaluator
/// given as a closure (the typed-IR host supplies values it already computed).
pub fn resolve_style_with_eval(
    modifiers: &[UiModifier],
    eval: &mut dyn FnMut(&tint_ast::Expr) -> EvalValue,
    tokens: &HashMap<String, UiModifierValue>,
) -> (StyleList, StyleList, Vec<(String, StyleList)>) {
    let evaluated = modifiers
        .iter()
        .map(|modifier| UiModifier {
            path: modifier.path.clone(),
            value: evaluate_modifier_value(&modifier.value, eval, tokens),
            span: modifier.span,
        })
        .collect::<Vec<_>>();
    resolve_style(&evaluated)
}

fn evaluate_modifier_value(
    value: &UiModifierValue,
    eval: &mut dyn FnMut(&tint_ast::Expr) -> EvalValue,
    tokens: &HashMap<String, UiModifierValue>,
) -> UiModifierValue {
    match value {
        UiModifierValue::Expr(expr) => match eval(expr) {
            EvalValue::Number(n) => UiModifierValue::Number(n),
            EvalValue::String(s) => UiModifierValue::String(s),
            EvalValue::Bool(b) => UiModifierValue::Ident(b.to_string()),
            _ => UiModifierValue::Ident("auto".to_string()),
        },
        UiModifierValue::Ident(name) if name.starts_with('@') => tokens
            .get(&name[1..])
            .map(|resolved| evaluate_modifier_value(resolved, eval, tokens))
            .unwrap_or_else(|| value.clone()),
        UiModifierValue::Block(items) => UiModifierValue::Block(
            items
                .iter()
                .map(|item| UiModifier {
                    path: item.path.clone(),
                    value: evaluate_modifier_value(&item.value, eval, tokens),
                    span: item.span,
                })
                .collect(),
        ),
        UiModifierValue::Tuple(items) => UiModifierValue::Tuple(
            items
                .iter()
                .map(|item| evaluate_modifier_value(item, eval, tokens))
                .collect(),
        ),
        UiModifierValue::MiniMod { key, value } => UiModifierValue::MiniMod {
            key: key.clone(),
            value: Box::new(evaluate_modifier_value(value, eval, tokens)),
        },
        other => other.clone(),
    }
}

fn apply_nested_style(value: &UiModifierValue, out: &mut StyleList) {
    if let UiModifierValue::Tuple(items) = value {
        for item in items {
            if let UiModifierValue::MiniMod { key, value } = item {
                apply_property(key, value, out);
            }
        }
    }
}

/// Resolves the items of an `app { page::{ ... } }` block (plain modifiers,
/// like inside `layout::{ ... }`) into CSS properties for the host `<body>`.
/// `@font-face` and `@keyframes` rules from `app { font.X::{ } keyframes.y::{ } }`.
pub fn app_at_rules(meta: &tint_ast::AppMeta) -> String {
    fn clean(text: &str) -> String {
        text.replace(['"', '\\', '{', '}', ';'], "")
    }
    fn item<'a>(items: &'a [UiModifierValue], name: &str) -> Option<&'a UiModifierValue> {
        items.iter().find_map(|item| match item {
            UiModifierValue::MiniMod { key, value } if key.len() == 1 && key[0] == name => {
                Some(&**value)
            }
            _ => None,
        })
    }
    fn text(value: &UiModifierValue) -> Option<String> {
        match value {
            UiModifierValue::String(s) | UiModifierValue::Ident(s) => Some(clean(s)),
            UiModifierValue::Number(n) => Some(n.to_string()),
            _ => None,
        }
    }
    let mut css = String::new();
    for (family, items) in &meta.fonts {
        let Some(src) = item(items, "src").and_then(text) else {
            continue;
        };
        let src = if src.contains("url(") || src.contains("local(") {
            src
        } else {
            let format = match src.rsplit('.').next() {
                Some("woff2") => " format(\"woff2\")",
                Some("woff") => " format(\"woff\")",
                Some("ttf") => " format(\"truetype\")",
                Some("otf") => " format(\"opentype\")",
                _ => "",
            };
            format!("url(\"{src}\"){format}")
        };
        css.push_str(&format!(
            "@font-face{{font-family:\"{}\";src:{src};font-display:{};",
            clean(family),
            item(items, "display")
                .and_then(text)
                .unwrap_or_else(|| "swap".into())
        ));
        if let Some(weight) = item(items, "weight").and_then(text) {
            css.push_str(&format!("font-weight:{weight};"));
        }
        if let Some(style) = item(items, "style").and_then(text) {
            css.push_str(&format!("font-style:{style};"));
        }
        css.push('}');
    }
    for (name, frames) in &meta.keyframes {
        css.push_str(&format!("@keyframes {}{{", clean(name)));
        for (selector, items) in frames {
            let selector = match selector.strip_prefix('p') {
                Some(percent) if !percent.is_empty() => {
                    format!("{}%", clean(&percent.replace('_', ".")))
                }
                _ => clean(selector),
            };
            css.push_str(&selector);
            css.push('{');
            for (property, value) in resolve_page_style(items) {
                css.push_str(&format!("{property}:{value};"));
            }
            css.push('}');
        }
        css.push('}');
    }
    css
}

pub fn resolve_page_style(items: &[UiModifierValue]) -> StyleList {
    let mut out = Vec::new();
    apply_group_items("layout", items, &mut out);
    out
}

fn apply_group_items(group: &str, items: &[UiModifierValue], out: &mut StyleList) {
    for item in items {
        if let UiModifierValue::MiniMod { key, value } = item {
            if key.len() == 1 && STYLE_GROUPS.contains(&key[0].as_str()) {
                if let UiModifierValue::Tuple(nested) = value.as_ref() {
                    apply_group_items(&key[0], nested, out);
                }
            } else {
                let key = grouped_property(group, key);
                apply_property(&key, value, out);
            }
        }
    }
}

const STYLE_GROUPS: [&str; 3] = ["layout", "paint", "motion"];

fn is_style_group(path: &[String]) -> bool {
    path.len() == 1 && STYLE_GROUPS.contains(&path[0].as_str())
}

/// Groups are syntax only: their children resolve through the same property
/// table as the flat form. This keeps the parser small while giving Tint a
/// language-level organization that is not tied to CSS's property ordering.
fn grouped_property(group: &str, path: &[String]) -> Vec<String> {
    let mut out = path.to_vec();

    if group == "paint" && out.len() == 1 && out[0] == "fill" {
        out[0] = "background".to_string();
    }
    if group == "paint" && out.len() == 1 && out[0] == "outline" {
        out[0] = "border".to_string();
    }

    out
}

include!("properties_base.rs");
include!("properties_box.rs");
include!("layout.rs");
include!("dispatch.rs");
