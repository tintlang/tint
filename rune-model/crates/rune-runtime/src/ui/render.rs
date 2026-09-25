// ui/render.rs
//
// Converts a built `UiTree` (see tree.rs) into `UiRenderNode`: a plain,
// serde-serializable snapshot of one tree, which is what crosses the
// wasm boundary to the sandbox (see rune-wasm/src/lib.rs's `render_ui`/
// `UiSession`). It is a snapshot, not a live handle -- `style`/
// `hover_style` are exactly what ui/style.rs resolved from a node's
// literal modifiers (dynamic/expression-valued *style* modifiers still
// aren't evaluated, same limitation documented in style.rs), `text` is
// genuinely evaluated (interpolations included, not just literal parts
// -- see builder.rs's `render_ui_text`), and `on_click`/`on_hover_enter`/
// `on_hover_leave` are the handler names from `click||`/`hover_in||`/
// `hover_out||` attributes, if any -- calling them is the sandbox's job
// (via `UiSession::dispatch`, not this snapshot). `svg` is raw markup
// from a `svg||"..."` attribute, verbatim, when the node has one.

use serde::Serialize;

use super::tree::{UiNodeId, UiTree};

#[derive(Debug, Clone, Serialize)]
pub struct UiRenderNode {
    pub tag: String,
    pub text: Option<String>,
    pub style: Vec<(String, String)>,
    pub hover_style: Vec<(String, String)>,
    pub on_click: Option<String>,
    pub on_hover_enter: Option<String>,
    pub on_hover_leave: Option<String>,
    pub svg: Option<String>,
    pub children: Vec<UiRenderNode>,
}

/// Recursively converts the node at `id` (and its descendants) into a
/// `UiRenderNode`. `id` is expected to come from `tree` itself (the
/// synthetic root `UiRuntime::mount` produces, or one of its
/// descendants) -- every caller in this codebase satisfies that.
pub fn to_render_tree(tree: &UiTree, id: UiNodeId) -> UiRenderNode {
    let node = &tree.nodes[id];
    UiRenderNode {
        tag: node.tag.clone(),
        text: node.text.clone(),
        style: node.style.clone(),
        hover_style: node.hover_style.clone(),
        on_click: node.on_click.clone(),
        on_hover_enter: node.on_hover_enter.clone(),
        on_hover_leave: node.on_hover_leave.clone(),
        svg: node.svg.clone(),
        children: node
            .children
            .iter()
            .map(|&cid| to_render_tree(tree, cid))
            .collect(),
    }
}

pub struct Renderer;

impl Renderer {
    pub fn render() {
        // TODO: integrate with Lynbor GPU
    }
}
