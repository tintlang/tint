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

use tint_ast::{UiModifier, UiModifierValue};

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

/// Screen-size names a `mobile::{...}`/`tablet::{...}`/`laptop::{...}`/
/// `desktop::{...}` modifier can use -- same nested-modifier shape as
/// `hover::{...}` (a `key::{ sub_key::value, ... }` block), just keyed by
/// viewport width instead of pointer state. The actual pixel ranges each
/// name maps to live with the renderer that has a real viewport to
/// measure (tint-wasm's `DomSession`), not here -- this module only
/// resolves modifiers into style lists, it has no notion of "the current
/// window size".
const BREAKPOINT_NAMES: [&str; 4] = ["mobile", "tablet", "laptop", "desktop"];

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

        if m.path.len() == 1 && BREAKPOINT_NAMES.contains(&m.path[0].as_str()) {
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

fn apply_nested_style(value: &UiModifierValue, out: &mut StyleList) {
    if let UiModifierValue::Tuple(items) = value {
        for item in items {
            if let UiModifierValue::MiniMod { key, value } = item {
                apply_property(key, value, out);
            }
        }
    }
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

// `radius::full` -- a pill/circle shortcut so a fully-rounded element
// (a pill button, a circular avatar) doesn't need a made-up magic number
// like `radius::999` that only works because it happens to be bigger
// than the element. CSS has no "auto-max" radius keyword of its own, so
// this resolves to a value large enough to always hit the browser's own
// clamp (border-radius maxes out at 50% of the box) -- same numeric
// trick as `999`, just named for what it means instead of why it works.
fn resolve_radius(value: &UiModifierValue) -> Option<String> {
    match value {
        UiModifierValue::Number(n) => Some(px(*n)),
        UiModifierValue::Ident(s) if s == "full" => Some("9999px".to_string()),
        _ => None,
    }
}

fn push_radius(out: &mut StyleList, props: &[&str], value: &UiModifierValue) {
    if let Some(v) = resolve_radius(value) {
        for p in props {
            out.push((p.to_string(), v.clone()));
        }
    }
}

// `scale::1.04` -> `transform: scale(1.04)`.
fn push_transform_fn(out: &mut StyleList, func: &str, value: &UiModifierValue) {
    if let UiModifierValue::Number(n) = value {
        out.push((
            "transform".to_string(),
            format!("{}({})", func, trim_num(*n)),
        ));
    }
}

// `blur::8` and `backdrop-blur::14` are typed Tint shorthands for the two
// browser blur primitives. Keeping the numeric form in the language avoids
// requiring authors to spell CSS function strings by hand.
fn push_blur(out: &mut StyleList, property: &str, value: &UiModifierValue) {
    if let UiModifierValue::Number(n) = value {
        out.push((property.to_string(), format!("blur({})", px(*n))));
    }
}

// `gradient::{ angle::135, from::#6c5ce7, to::#8b7cf0 }` is the typed Tint
// form for a linear background gradient. Direct color items are also accepted
// and use a left-to-right gradient: `gradient::{#111, #333}`.
fn apply_gradient(value: &UiModifierValue, out: &mut StyleList) {
    let UiModifierValue::Tuple(items) = value else {
        return;
    };
    let mut angle = "90deg".to_string();
    let mut colors = Vec::new();

    for item in items {
        match item {
            UiModifierValue::MiniMod { key, value }
                if key.first().is_some_and(|k| k == "angle") =>
            {
                match value.as_ref() {
                    UiModifierValue::Number(n) => angle = format!("{}deg", trim_num(*n)),
                    UiModifierValue::String(s) | UiModifierValue::Ident(s) => angle = s.clone(),
                    _ => {}
                }
            }
            UiModifierValue::MiniMod { key, value }
                if key
                    .first()
                    .is_some_and(|k| matches!(k.as_str(), "from" | "to" | "color" | "stop")) =>
            {
                if let Some(color) = color_string(value) {
                    colors.push(color);
                }
            }
            other => {
                if let Some(color) = color_string(other) {
                    colors.push(color);
                }
            }
        }
    }

    if colors.len() >= 2 {
        out.push((
            "background-image".to_string(),
            format!("linear-gradient({}, {})", angle, colors.join(", ")),
        ));
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
                    if let (Some(side), UiModifierValue::Number(n)) = (key.first(), value.as_ref())
                    {
                        match side.as_str() {
                            "left" | "right" | "top" | "bottom" => {
                                out.push((format!("padding-{}", side), px(*n)));
                            }
                            "l" => out.push(("padding-left".to_string(), px(*n))),
                            "r" => out.push(("padding-right".to_string(), px(*n))),
                            "t" => out.push(("padding-top".to_string(), px(*n))),
                            "b" => out.push(("padding-bottom".to_string(), px(*n))),
                            "x" => {
                                out.push(("padding-left".to_string(), px(*n)));
                                out.push(("padding-right".to_string(), px(*n)));
                            }
                            "y" => {
                                out.push(("padding-top".to_string(), px(*n)));
                                out.push(("padding-bottom".to_string(), px(*n)));
                            }
                            _ => {}
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
                    if let (Some(side), UiModifierValue::Number(n)) = (key.first(), value.as_ref())
                    {
                        let sides: &[&str] = match side.as_str() {
                            "left" | "l" => &["left"],
                            "right" | "r" => &["right"],
                            "top" | "t" => &["top"],
                            "bottom" | "b" => &["bottom"],
                            "x" => &["left", "right"],
                            "y" => &["top", "bottom"],
                            _ => &[],
                        };
                        for edge in sides {
                            out.push((format!("margin-{}", edge), px(*n)));
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

fn apply_border(value: &UiModifierValue, out: &mut StyleList) {
    if let Some(border) = border_value(value) {
        out.push(("border".to_string(), border));
    }
}

fn border_value(value: &UiModifierValue) -> Option<String> {
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
            return Some(format!("{}px solid {}", trim_num(w), c));
        }
    }
    None
}

fn apply_border_edges(value: &UiModifierValue, out: &mut StyleList, edges: &[&str]) {
    if let Some(border) = border_value(value) {
        for edge in edges {
            out.push((format!("border-{}", edge), border.clone()));
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
        if matches!(
            s.as_str(),
            "row" | "column" | "row-reverse" | "column-reverse"
        ) {
            out.push(("display".to_string(), "flex".to_string()));
            out.push(("flex-direction".to_string(), s.clone()));
        }
    }
}

/// `grid::{ columns::2, gap::24 }` is the compact Tint form for a responsive
/// CSS grid. A string can provide an explicit track list, for example
/// `columns::"minmax(0, 1fr) minmax(0, 1fr)"`.
fn apply_grid(value: &UiModifierValue, out: &mut StyleList) {
    out.push(("display".to_string(), "grid".to_string()));
    let UiModifierValue::Tuple(items) = value else {
        return;
    };

    for item in items {
        let UiModifierValue::MiniMod { key, value } = item else {
            continue;
        };
        match key.join(".").as_str() {
            "columns" | "template-columns" => match value.as_ref() {
                UiModifierValue::Number(n) => out.push((
                    "grid-template-columns".to_string(),
                    format!("repeat({}, minmax(0, 1fr))", trim_num(*n)),
                )),
                UiModifierValue::String(s) | UiModifierValue::Ident(s) => out.push((
                    "grid-template-columns".to_string(),
                    s.clone(),
                )),
                _ => {}
            },
            "rows" | "template-rows" => push_raw_string(out, "grid-template-rows", value),
            "gap" => push_px(out, "gap", value),
            "column-gap" => push_px(out, "column-gap", value),
            "row-gap" => push_px(out, "row-gap", value),
            "auto-flow" => push_raw_string(out, "grid-auto-flow", value),
            _ => {}
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
        if matches!(
            s.as_str(),
            "start" | "center" | "end" | "stretch" | "baseline"
        ) {
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
// to, composed by the .tint source itself.
fn apply_position(value: &UiModifierValue, out: &mut StyleList) {
    if let UiModifierValue::Ident(s) = value {
        if matches!(
            s.as_str(),
            "static" | "relative" | "absolute" | "fixed" | "sticky"
        ) {
            out.push(("position".to_string(), s.clone()));
        }
    }
}

fn apply_property(path: &[String], value: &UiModifierValue, out: &mut StyleList) {
    match path.join(".").as_str() {
        "padding" => apply_padding(value, out),
        "padding.x" => {
            push_px(out, "padding-left", value);
            push_px(out, "padding-right", value);
        }
        "padding.y" => {
            push_px(out, "padding-top", value);
            push_px(out, "padding-bottom", value);
        }
        "padding.t" => push_px(out, "padding-top", value),
        "padding.b" => push_px(out, "padding-bottom", value),
        "padding.l" => push_px(out, "padding-left", value),
        "padding.r" => push_px(out, "padding-right", value),
        "margin" => apply_margin(value, out),
        "margin.x" => {
            push_px(out, "margin-left", value);
            push_px(out, "margin-right", value);
        }
        "margin.y" => {
            push_px(out, "margin-top", value);
            push_px(out, "margin-bottom", value);
        }
        "margin.t" => push_px(out, "margin-top", value),
        "margin.b" => push_px(out, "margin-bottom", value),
        "margin.l" => push_px(out, "margin-left", value),
        "margin.r" => push_px(out, "margin-right", value),
        "direction" => apply_direction(value, out),
        "grid" => apply_grid(value, out),
        "display" => push_raw_string(out, "display", value),
        "align" => apply_align(value, out),
        "justify" => apply_justify(value, out),
        "position" => apply_position(value, out),
        "top" => push_px(out, "top", value),
        "left" => push_px(out, "left", value),
        "right" => push_px(out, "right", value),
        "bottom" => push_px(out, "bottom", value),
        "z" => push_raw(out, "z-index", value),
        "radius" => push_radius(out, &["border-radius"], value),
        "radius.top" => push_radius(
            out,
            &["border-top-left-radius", "border-top-right-radius"],
            value,
        ),
        "radius.bottom" => push_radius(
            out,
            &["border-bottom-left-radius", "border-bottom-right-radius"],
            value,
        ),
        "radius.left" => push_radius(
            out,
            &["border-top-left-radius", "border-bottom-left-radius"],
            value,
        ),
        "radius.right" => push_radius(
            out,
            &["border-top-right-radius", "border-bottom-right-radius"],
            value,
        ),
        "gap" => push_px(out, "gap", value),
        "min-width" => push_px(out, "min-width", value),
        "min-height" => push_px(out, "min-height", value),
        "overflow" => push_raw_string(out, "overflow", value),
        "overflow-x" => push_raw_string(out, "overflow-x", value),
        "overflow-y" => push_raw_string(out, "overflow-y", value),
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
        "gradient" => apply_gradient(value, out),
        "color" => push_color(out, "color", value),
        "size" => push_px(out, "font-size", value),
        "font-family" => push_raw_string(out, "font-family", value),
        "border" => apply_border(value, out),
        "border.x" => apply_border_edges(value, out, &["left", "right"]),
        "border.y" => apply_border_edges(value, out, &["top", "bottom"]),
        "border.t" | "border.top" => apply_border_edges(value, out, &["top"]),
        "border.b" | "border.bottom" => apply_border_edges(value, out, &["bottom"]),
        "border.l" | "border.left" => apply_border_edges(value, out, &["left"]),
        "border.r" | "border.right" => apply_border_edges(value, out, &["right"]),
        "text" => apply_text(value, out),
        "scale" => push_transform_fn(out, "scale", value),
        "transform" => push_raw_string(out, "transform", value),
        "blur" => push_blur(out, "filter", value),
        "backdrop-blur" => push_blur(out, "backdrop-filter", value),
        "filter" => push_raw_string(out, "filter", value),
        "shadow" | "box-shadow" => push_raw_string(out, "box-shadow", value),
        // A base (non-hover) `transition::"transform .25s ease"` is what
        // makes a `hover::{...}` style change (a squeeze transform, a
        // color shift, ...) actually animate instead of snapping --
        // there's no dedicated `transition` concept in the language, just
        // this raw CSS passthrough, the same as `transform`/`filter`/
        // `shadow` above.
        "transition" => push_raw_string(out, "transition", value),
        // `text-decoration::"underline"` -- mainly meant for inside a
        // `hover::{...}` block (a link that underlines on hover), the same
        // raw CSS passthrough as `transform`/`filter`/`shadow`/`transition`
        // above rather than a dedicated modifier, since underline is a
        // one-off need, not a family of values worth its own resolver.
        "text-decoration" => push_raw_string(out, "text-decoration", value),
        // Anything else (e.g. a modifier the grammar accepts generically
        // but this resolver doesn't map to CSS yet) is left unresolved
        // rather than guessed at.
        _ => {}
    }
}
