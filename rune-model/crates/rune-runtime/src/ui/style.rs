// ui/style.rs
//
// Resolves a UI node's `key::value` modifiers (rune_ast::UiModifier) into
// plain CSS property/value pairs a renderer can apply directly -- as an
// inline `style` attribute for the base list, and as a generated
// `:hover { ... }` rule for the hover list (a nested `hover::{ ... }`
// modifier is the language-level way to express what the sandbox used to
// fake with hardcoded, tag-name-based CSS -- see sandbox/src/components/
// UiPreviewNode.svelte's TAG_DEFAULTS comment for the effects this is
// meant to let Rune code state explicitly instead).
//
// Only LITERAL modifier values resolve here (numbers, idents, strings,
// and nested tuples/mini-mods of those). A modifier whose value is an
// arbitrary expression (`UiModifierValue::Expr`, e.g. a variable read, or
// what `if{}`/`for{}` carry) is skipped: resolving those needs the
// running VM's state/evaluator, which this pure AST-level pass over
// `UiNode::modifiers` doesn't have access to (see builder.rs). That's a
// separate, later piece of work, not a limitation of this resolver.

use rune_ast::{UiModifier, UiModifierValue};

/// Flat list of resolved CSS `(property, value)` pairs, in the order the
/// source modifiers were written.
pub type StyleList = Vec<(String, String)>;

/// Splits a node's modifiers into its base style and (if a `hover::{...}`
/// modifier is present) the style to apply on hover.
pub fn resolve_style(modifiers: &[UiModifier]) -> (StyleList, StyleList) {
    let mut style = Vec::new();
    let mut hover_style = Vec::new();

    for m in modifiers {
        if m.path.len() == 1 && m.path[0] == "hover" {
            if let UiModifierValue::Tuple(items) = &m.value {
                for item in items {
                    if let UiModifierValue::MiniMod { key, value } = item {
                        apply_property(key, value, &mut hover_style);
                    }
                }
            }
            continue;
        }

        apply_property(&m.path, &m.value, &mut style);
    }

    (style, hover_style)
}

// Named/hex colors are passed straight through as CSS already understands
// both ("white", "#6c5ce7", ...) -- no lookup table needed.
fn color_string(v: &UiModifierValue) -> Option<String> {
    match v {
        UiModifierValue::Ident(s) => Some(s.clone()),
        UiModifierValue::String(s) => Some(s.clone()),
        _ => None,
    }
}

fn trim_num(n: f64) -> String {
    if n == n.trunc() {
        format!("{}", n as i64)
    } else {
        format!("{}", n)
    }
}

fn px(n: f64) -> String {
    format!("{}px", trim_num(n))
}

fn push_color(out: &mut StyleList, prop: &str, value: &UiModifierValue) {
    if let Some(c) = color_string(value) {
        out.push((prop.to_string(), c));
    }
}

fn push_px(out: &mut StyleList, prop: &str, value: &UiModifierValue) {
    if let UiModifierValue::Number(n) = value {
        out.push((prop.to_string(), px(*n)));
    }
}

fn push_raw(out: &mut StyleList, prop: &str, value: &UiModifierValue) {
    if let UiModifierValue::Number(n) = value {
        out.push((prop.to_string(), trim_num(*n)));
    }
}

fn push_all_px(out: &mut StyleList, props: &[&str], value: &UiModifierValue) {
    if let UiModifierValue::Number(n) = value {
        for p in props {
            out.push((p.to_string(), px(*n)));
        }
    }
}

// `scale::1.04` -> `transform: scale(1.04)`.
fn push_transform_fn(out: &mut StyleList, func: &str, value: &UiModifierValue) {
    if let UiModifierValue::Number(n) = value {
        out.push(("transform".to_string(), format!("{}({})", func, trim_num(*n))));
    }
}

// Raw CSS values the language has no dedicated modifier for yet
// (`transform::"translateY(-2px)"`, `filter::"brightness(1.12)"`,
// `shadow::"0 6px 16px rgba(0,0,0,.28)"`) -- mainly useful inside
// `hover::{...}` to reproduce exactly the kind of lift/brighten/shadow
// effects the sandbox used to hardcode.
fn push_raw_string(out: &mut StyleList, prop: &str, value: &UiModifierValue) {
    match value {
        UiModifierValue::String(s) => out.push((prop.to_string(), s.clone())),
        UiModifierValue::Ident(s) => out.push((prop.to_string(), s.clone())),
        _ => {}
    }
}

fn apply_padding(value: &UiModifierValue, out: &mut StyleList) {
    match value {
        UiModifierValue::Number(_) => push_px(out, "padding", value),
        UiModifierValue::Tuple(items) => {
            for item in items {
                if let UiModifierValue::MiniMod { key, value } = item {
                    if let (Some(side), UiModifierValue::Number(n)) = (key.first(), value.as_ref()) {
                        if matches!(side.as_str(), "left" | "right" | "top" | "bottom") {
                            out.push((format!("padding-{}", side), px(*n)));
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

// `margin::16` (all sides) / `margin::{left::12, right::24}` (per side,
// same tuple-of-MiniMod shape as `padding` above) -- the way to get
// UNEVEN spacing between siblings, since CSS `gap` on the container is
// necessarily uniform. E.g. three cards in a `direction::row` with no
// `gap::` at all, each carrying its own `margin::{right::n}`, can sit
// with a different gap after each one.
fn apply_margin(value: &UiModifierValue, out: &mut StyleList) {
    match value {
        UiModifierValue::Number(_) => push_px(out, "margin", value),
        UiModifierValue::Tuple(items) => {
            for item in items {
                if let UiModifierValue::MiniMod { key, value } = item {
                    if let (Some(side), UiModifierValue::Number(n)) = (key.first(), value.as_ref()) {
                        if matches!(side.as_str(), "left" | "right" | "top" | "bottom") {
                            out.push((format!("margin-{}", side), px(*n)));
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

fn apply_border(value: &UiModifierValue, out: &mut StyleList) {
    if let UiModifierValue::Tuple(items) = value {
        let mut width = None;
        let mut color = None;

        for item in items {
            if let UiModifierValue::Number(n) = item {
                width = width.or(Some(*n));
            } else if let Some(c) = color_string(item) {
                color = color.or(Some(c));
            }
        }

        if let (Some(w), Some(c)) = (width, color) {
            out.push(("border".to_string(), format!("{}px solid {}", trim_num(w), c)));
        }
    }
}

// A known set of font-weight keywords, so a bare color ident (like
// `white`/`red`/`#6c5ce7`) in the same tuple isn't mistaken for one.
// ("black" is deliberately NOT here -- it's overwhelmingly used as a
// color in this codebase's own examples, e.g. `background::black`.)
const WEIGHT_KEYWORDS: &[&str] = &[
    "thin", "light", "regular", "normal", "medium", "semibold", "bold",
];

fn apply_text(value: &UiModifierValue, out: &mut StyleList) {
    let items: Vec<&UiModifierValue> = match value {
        UiModifierValue::Tuple(items) => items.iter().collect(),
        other => vec![other],
    };

    let mut size_set = false;

    for item in items {
        match item {
            UiModifierValue::Number(n) if !size_set => {
                out.push(("font-size".to_string(), px(*n)));
                size_set = true;
            }
            UiModifierValue::Ident(s) if WEIGHT_KEYWORDS.contains(&s.as_str()) => {
                out.push(("font-weight".to_string(), s.clone()));
            }
            _ => push_color(out, "color", item),
        }
    }
}

// `direction::row` / `direction::column` (and the CSS reverses) -- the
// language's first real layout primitive. Resolves to `display: flex` +
// `flex-direction`, so a node's children actually lay out side by side
// or stacked, and its `gap::n` (already resolved above) finally has
// something to apply to -- CSS `gap` is a no-op on a plain block
// container. No default direction is invented for an unannotated node;
// without this modifier a node's children just stack as plain blocks,
// which is the honest current behavior, not a bug to hide.
fn apply_direction(value: &UiModifierValue, out: &mut StyleList) {
    if let UiModifierValue::Ident(s) = value {
        if matches!(s.as_str(), "row" | "column" | "row-reverse" | "column-reverse") {
            out.push(("display".to_string(), "flex".to_string()));
            out.push(("flex-direction".to_string(), s.clone()));
        }
    }
}

// `align::center` (and start/end/stretch/baseline) -- cross-axis
// alignment for a `direction::row`/`direction::column` flex container,
// e.g. so a heading and an icon badge next to it share the same visual
// center instead of both stretching to the container's full height
// (flexbox's default `align-items: stretch`).
fn apply_align(value: &UiModifierValue, out: &mut StyleList) {
    if let UiModifierValue::Ident(s) = value {
        if matches!(s.as_str(), "start" | "center" | "end" | "stretch" | "baseline") {
            out.push(("align-items".to_string(), s.clone()));
        }
    }
}

// `justify::start|center|end|space-between|space-around|space-evenly` --
// MAIN-axis distribution inside a `direction::row`/`direction::column`
// flex container (as opposed to `align`, which is cross-axis). This is
// what spreads a row of items evenly across the full width instead of
// clumping them at the start, e.g. `justify::space-between` on a row of
// account cards -> CSS `justify-content: space-between`.
fn apply_justify(value: &UiModifierValue, out: &mut StyleList) {
    if let UiModifierValue::Ident(s) = value {
        if matches!(
            s.as_str(),
            "start" | "center" | "end" | "space-between" | "space-around" | "space-evenly"
        ) {
            out.push(("justify-content".to_string(), s.clone()));
        }
    }
}

// `position::relative` / `position::absolute` (and static/fixed/sticky)
// -- lets a node opt out of normal document flow. Paired with
// `top`/`left`/`right`/`bottom::n` and `z::n` below, this is what makes
// a real floating popover possible: an `absolute` node positions itself
// against its nearest `relative` ancestor instead of pushing its
// siblings around, and `z` controls stacking order above everything
// else. No popover-specific concept is baked into the language --
// these are the same raw CSS primitives every other modifier here maps
// to, composed by the .rune source itself.
fn apply_position(value: &UiModifierValue, out: &mut StyleList) {
    if let UiModifierValue::Ident(s) = value {
        if matches!(s.as_str(), "static" | "relative" | "absolute" | "fixed" | "sticky") {
            out.push(("position".to_string(), s.clone()));
        }
    }
}

fn apply_property(path: &[String], value: &UiModifierValue, out: &mut StyleList) {
    match path.join(".").as_str() {
        "padding" => apply_padding(value, out),
        "margin" => apply_margin(value, out),
        "direction" => apply_direction(value, out),
        "align" => apply_align(value, out),
        "justify" => apply_justify(value, out),
        "position" => apply_position(value, out),
        "top" => push_px(out, "top", value),
        "left" => push_px(out, "left", value),
        "right" => push_px(out, "right", value),
        "bottom" => push_px(out, "bottom", value),
        "z" => push_raw(out, "z-index", value),
        "radius" => push_all_px(out, &["border-radius"], value),
        "radius.top" => push_all_px(out, &["border-top-left-radius", "border-top-right-radius"], value),
        "radius.bottom" => push_all_px(out, &["border-bottom-left-radius", "border-bottom-right-radius"], value),
        "radius.left" => push_all_px(out, &["border-top-left-radius", "border-bottom-left-radius"], value),
        "radius.right" => push_all_px(out, &["border-top-right-radius", "border-bottom-right-radius"], value),
        "gap" => push_px(out, "gap", value),
        // `grow::n` -> CSS `flex-grow: n` -- lets a flex child take a SHARE
        // of a row/column's leftover space instead of sizing to its own
        // content, e.g. three `MiniCard`s each with `grow::1` inside a
        // `direction::row gap::n` container split the row's full width
        // evenly between them (with real gaps, via `gap`) instead of
        // sitting at their content width with dead space after them --
        // the equal-width-columns counterpart to `justify::space-between`
        // (which spreads CONTENT-sized items apart instead of resizing
        // them).
        "grow" => push_raw(out, "flex-grow", value),
        "opacity" => push_raw(out, "opacity", value),
        "background" => push_color(out, "background-color", value),
        "color" => push_color(out, "color", value),
        "size" => push_px(out, "font-size", value),
        "border" => apply_border(value, out),
        "text" => apply_text(value, out),
        "scale" => push_transform_fn(out, "scale", value),
        "transform" => push_raw_string(out, "transform", value),
        "filter" => push_raw_string(out, "filter", value),
        "shadow" | "box-shadow" => push_raw_string(out, "box-shadow", value),
        // A base (non-hover) `transition::"transform .25s ease"` is what
        // makes a `hover::{...}` style change (a squeeze transform, a
        // color shift, ...) actually animate instead of snapping --
        // there's no dedicated `transition` concept in the language, just
        // this raw CSS passthrough, the same as `transform`/`filter`/
        // `shadow` above.
        "transition" => push_raw_string(out, "transition", value),
        // Anything else (e.g. a modifier the grammar accepts generically
        // but this resolver doesn't map to CSS yet) is left unresolved
        // rather than guessed at.
        _ => {}
    }
}
