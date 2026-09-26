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
