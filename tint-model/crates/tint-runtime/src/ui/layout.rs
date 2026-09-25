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
                padding = Rect { left: lp, right: lp, top: lp, bottom: lp };
            }
            "padding-left" => padding.left = length_percentage_px(value),
            "padding-right" => padding.right = length_percentage_px(value),
            "padding-top" => padding.top = length_percentage_px(value),
            "padding-bottom" => padding.bottom = length_percentage_px(value),
            "margin" => {
                let m = length_percentage_auto_px(value);
                margin = Rect { left: m, right: m, top: m, bottom: m };
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

/// One node's computed layout, in ABSOLUTE pixel coordinates (i.e.
/// already accumulated from root, not just relative to its parent --
/// what a renderer actually needs to draw it).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NodeLayout {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Computes layout for `tree` rooted at `root`, inside a viewport of
/// `available_width` x `available_height` pixels. Returns every visited
/// node's absolute layout keyed by `UiNodeId`.
pub fn compute_layout(
    tree: &UiTree,
    root: UiNodeId,
    available_width: f32,
    available_height: f32,
) -> HashMap<UiNodeId, NodeLayout> {
    let mut taffy_tree: TaffyTree<()> = TaffyTree::new();
    let mut tint_to_taffy: HashMap<UiNodeId, NodeId> = HashMap::new();

    fn build(
        tree: &UiTree,
        id: UiNodeId,
        taffy_tree: &mut TaffyTree<()>,
        tint_to_taffy: &mut HashMap<UiNodeId, NodeId>,
    ) -> NodeId {
        let node = &tree.nodes[id];
        let child_ids: Vec<NodeId> = node
            .children
            .iter()
            .map(|&c| build(tree, c, taffy_tree, tint_to_taffy))
            .collect();
        let tid = taffy_tree
            .new_with_children(taffy_style(&node.style), &child_ids)
            .expect("taffy node creation");
        tint_to_taffy.insert(id, tid);
        tid
    }

    let taffy_root = build(tree, root, &mut taffy_tree, &mut tint_to_taffy);

    // The root has no parent to stretch against (an ordinary block
    // child's auto width fills its parent, but the root itself has
    // none), so taffy leaves an auto-sized root at its content size
    // instead of the viewport -- same reason a browser gives `html`/
    // `body` an explicit 100% size instead of leaving them auto. Force
    // the root to the full viewport here, the same role.
    let mut root_style = taffy_tree.style(taffy_root).expect("root style").clone();
    root_style.size = Size {
        width: Dimension::length(available_width),
        height: Dimension::length(available_height),
    };
    // A block box (which is what the root logically is, same as
    // `html`/`body` in a page -- children stack vertically and stretch
    // to fill its width by default) rather than taffy's own default
    // (a flex row), so an unstyled top-level node actually gets the
    // full viewport width instead of shrinking to its content.
    root_style.display = Display::Block;
    taffy_tree
        .set_style(taffy_root, root_style)
        .expect("root style override");

    taffy_tree
        .compute_layout(
            taffy_root,
            Size {
                width: AvailableSpace::Definite(available_width),
                height: AvailableSpace::Definite(available_height),
            },
        )
        .expect("taffy layout");

    // taffy hands back each node's rect relative to its own parent;
    // walk down from root accumulating parent offsets so callers get
    // absolute on-screen coordinates without having to know the tree
    // shape themselves.
    let mut out = HashMap::new();

    fn collect(
        tree: &UiTree,
        id: UiNodeId,
        taffy_tree: &TaffyTree<()>,
        tint_to_taffy: &HashMap<UiNodeId, NodeId>,
        parent_x: f32,
        parent_y: f32,
        out: &mut HashMap<UiNodeId, NodeLayout>,
    ) {
        let tid = tint_to_taffy[&id];
        let layout = taffy_tree.layout(tid).expect("taffy layout lookup");
        let x = parent_x + layout.location.x;
        let y = parent_y + layout.location.y;
        out.insert(
            id,
            NodeLayout { x, y, width: layout.size.width, height: layout.size.height },
        );
        for &child in &tree.nodes[id].children {
            collect(tree, child, taffy_tree, tint_to_taffy, x, y, out);
        }
    }

    collect(tree, root, &taffy_tree, &tint_to_taffy, 0.0, 0.0, &mut out);

    out
}
