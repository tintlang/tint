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
