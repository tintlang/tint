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

fn push_length_or_raw(out: &mut StyleList, prop: &str, value: &UiModifierValue) {
    match value {
        UiModifierValue::Number(n) => out.push((prop.to_string(), px(*n))),
        UiModifierValue::String(v) | UiModifierValue::Ident(v) => {
            out.push((prop.to_string(), v.clone()))
        }
        _ => {}
    }
}

// `flex::1`, `aspect-ratio::1.5` (a number) or `aspect-ratio::"16 / 9"` (a string).
fn push_number_or_raw(out: &mut StyleList, prop: &str, value: &UiModifierValue) {
    match value {
        UiModifierValue::Number(n) => out.push((prop.to_string(), trim_num(*n))),
        UiModifierValue::String(s) | UiModifierValue::Ident(s) => {
            out.push((prop.to_string(), s.clone()))
        }
        _ => {}
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

// `scale::1.04` -> `transform: scale(1.04)`. Typed transforms (`scale`,
// `rotate`, `translate.x`, `skew.x`, ...) combine into one `transform` value
// instead of overriding each other.
fn push_transform_fn(out: &mut StyleList, func: &str, value: &UiModifierValue) {
    push_transform_unit(out, func, "", value);
}

fn push_transform_unit(out: &mut StyleList, func: &str, unit: &str, value: &UiModifierValue) {
    if let UiModifierValue::Number(n) = value {
        let part = format!("{}({}{})", func, trim_num(*n), unit);
        match out.iter_mut().rev().find(|(property, _)| property == "transform") {
            Some((_, existing)) => {
                existing.push(' ');
                existing.push_str(&part);
            }
            None => out.push(("transform".to_string(), part)),
        }
    }
}

// `ring::{2, #6c5ce7}` -> `box-shadow: 0 0 0 2px #6c5ce7`, added to any
// shadow already on the node (a focus ring next to a drop shadow).
fn apply_ring(value: &UiModifierValue, out: &mut StyleList) {
    let ring = match value {
        UiModifierValue::String(s) => Some(s.clone()),
        other => border_value(other).map(|border| {
            let (width, rest) = border.split_once(" solid ").unwrap_or(("0px", "currentColor"));
            format!("0 0 0 {} {}", width, rest)
        }),
    };
    let Some(ring) = ring else { return };
    match out.iter_mut().rev().find(|(property, _)| property == "box-shadow") {
        Some((_, existing)) => {
            existing.push_str(", ");
            existing.push_str(&ring);
        }
        None => out.push(("box-shadow".to_string(), ring)),
    }
}

// `line-clamp::3` -- cut text after 3 lines with an ellipsis.
fn apply_line_clamp(value: &UiModifierValue, out: &mut StyleList) {
    if let UiModifierValue::Number(n) = value {
        out.push(("display".to_string(), "-webkit-box".to_string()));
        out.push(("-webkit-box-orient".to_string(), "vertical".to_string()));
        out.push(("-webkit-line-clamp".to_string(), trim_num(*n)));
        out.push(("overflow".to_string(), "hidden".to_string()));
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
