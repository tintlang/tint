// ui/layout.rs
//
// Computes real flexbox layout (position + size, in pixels) for a
// UiTree using taffy -- the same flexbox algorithm a browser runs, but
// driven directly from Rust instead of through DOM/CSS.
//
// This is the first slice of a second rendering backend, alongside the
// existing DOM backend (style.rs resolves modifiers to CSS strings,
// which sandbox/ hands to the browser). It deliberately reads the SAME
// resolved `UiElement.style`/`hover_style` (a `StyleList`, i.e.
// `Vec<(property, value)>`) that the DOM path already produces via
// `style::resolve_style`, rather than re-reading raw modifiers a second
// time. That keeps exactly one place where a node's modifiers are
// resolved, and two consumers of the result: CSS strings for the DOM
// backend, typed taffy values here for a future GPU backend -- so the
// two backends can never see different styles, and nothing about the
// existing DOM/CSS path changes to make this possible.
//
// Only layout-affecting properties are read here (display/flex-direction,
// gap, padding-*, margin-*, align-items, justify-content). Paint-only
// properties (background, color, shadow, border color, ...) are a later
// renderer slice's concern, not layout's.

use std::collections::HashMap;

use taffy::prelude::*;

use super::style::StyleList;
use super::tree::{UiNodeId, UiTree};

fn parse_px(v: &str) -> Option<f32> {
    v.strip_suffix("px").and_then(|n| n.parse::<f32>().ok())
}

fn length_percentage_px(v: &str) -> LengthPercentage {
    LengthPercentage::length(parse_px(v).unwrap_or(0.0))
}

fn length_percentage_auto_px(v: &str) -> LengthPercentageAuto {
    LengthPercentageAuto::length(parse_px(v).unwrap_or(0.0))
}

fn flex_direction(v: &str) -> FlexDirection {
    match v {
        "row" => FlexDirection::Row,
        "row-reverse" => FlexDirection::RowReverse,
        "column" => FlexDirection::Column,
        "column-reverse" => FlexDirection::ColumnReverse,
        _ => FlexDirection::Row,
    }
}

fn align_items(v: &str) -> Option<AlignItems> {
    match v {
        "start" => Some(AlignItems::FLEX_START),
        "center" => Some(AlignItems::CENTER),
        "end" => Some(AlignItems::FLEX_END),
        "stretch" => Some(AlignItems::STRETCH),
        "baseline" => Some(AlignItems::BASELINE),
        _ => None,
    }
}

fn justify_content(v: &str) -> Option<JustifyContent> {
    match v {
        "start" => Some(JustifyContent::FLEX_START),
        "center" => Some(JustifyContent::CENTER),
        "end" => Some(JustifyContent::FLEX_END),
        "space-between" => Some(JustifyContent::SPACE_BETWEEN),
        "space-around" => Some(JustifyContent::SPACE_AROUND),
        "space-evenly" => Some(JustifyContent::SPACE_EVENLY),
        _ => None,
    }
}

/// Reads a resolved `StyleList` (as produced by `style::resolve_style`)
/// into a taffy `Style`. Unrecognized or paint-only properties are
/// ignored -- this never fails, a node with no layout-affecting
/// modifiers just gets taffy's defaults (a plain block, like the DOM
/// backend gives it too when `direction::` is absent).
fn taffy_style(style: &StyleList) -> Style {
    let mut s = Style::default();
    let mut padding = Rect::<LengthPercentage>::zero();
    let mut margin = Rect::<LengthPercentageAuto>::zero();

    for (prop, value) in style {
        match prop.as_str() {
            "display" if value == "flex" => s.display = Display::Flex,
            "flex-direction" => s.flex_direction = flex_direction(value),
            "gap" => {
                if let Some(px) = parse_px(value) {
                    s.gap = Size {
                        width: LengthPercentage::length(px),
                        height: LengthPercentage::length(px),
                    };
                }
            }
            "align-items" => s.align_items = align_items(value),
            "justify-content" => s.justify_content = justify_content(value),
            "padding" => {
                let lp = length_percentage_px(value);
                padding = Rect {
                    left: lp,
                    right: lp,
                    top: lp,
                    bottom: lp,
                };
            }
            "padding-left" => padding.left = length_percentage_px(value),
            "padding-right" => padding.right = length_percentage_px(value),
            "padding-top" => padding.top = length_percentage_px(value),
            "padding-bottom" => padding.bottom = length_percentage_px(value),
            "margin" => {
                let m = length_percentage_auto_px(value);
                margin = Rect {
                    left: m,
                    right: m,
                    top: m,
                    bottom: m,
                };
            }
            "margin-left" => margin.left = length_percentage_auto_px(value),
            "margin-right" => margin.right = length_percentage_auto_px(value),
            "margin-top" => margin.top = length_percentage_auto_px(value),
            "margin-bottom" => margin.bottom = length_percentage_auto_px(value),
            "flex-grow" => {
                if let Ok(n) = value.parse::<f32>() {
                    s.flex_grow = n;
                }
            }
            _ => {}
        }
    }

    s.padding = padding;
    s.margin = margin;
    s
}
