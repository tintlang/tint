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
                UiModifierValue::String(s) | UiModifierValue::Ident(s) => {
                    out.push(("grid-template-columns".to_string(), s.clone()))
                }
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
