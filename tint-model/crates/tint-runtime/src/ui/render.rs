// ui/render.rs
//
// Converts a built `UiTree` (see tree.rs) into `UiRenderNode`: a plain,
// serde-serializable snapshot of one tree, which is what crosses the
// wasm boundary to the sandbox (see tint-wasm/src/lib.rs's `render_ui`/
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

use std::rc::Rc;

use serde::Serialize;

use super::tree::{UiNodeId, UiTree};

#[derive(Debug, Clone, Serialize)]
pub struct UiRenderNode {
    pub tag: String,
    pub tint_source: String,
    pub text: Option<String>,
    pub style: Vec<(String, String)>,
    pub hover_style: Vec<(String, String)>,
    pub breakpoints: Vec<(String, Vec<(String, String)>)>,
    pub on_click: Option<String>,
    pub on_hover_enter: Option<String>,
    pub on_hover_leave: Option<String>,
    pub on_key_down: Option<String>,
    pub on_key_up: Option<String>,
    pub on_frame: Option<String>,
    pub on_tick: Option<String>,
    pub every_ms: Option<f64>,
    pub asset: Option<String>,
    pub key: Option<String>,
    pub sound: Option<String>,
    pub svg: Option<String>,
    pub route: Option<String>,
    pub target: Option<String>,
    pub reference: Option<String>,
    pub on_js: Option<String>,
    /// Shared so a subtree that did not change between two renders is the
    /// very same allocation both times (`Rc::ptr_eq`), which lets a renderer
    /// skip it without comparing anything.
    pub children: Vec<Rc<UiRenderNode>>,
}

impl PartialEq for UiRenderNode {
    fn eq(&self, other: &Self) -> bool {
        self.tag == other.tag
            && self.tint_source == other.tint_source
            && self.text == other.text
            && self.style == other.style
            && self.hover_style == other.hover_style
            && self.breakpoints == other.breakpoints
            && self.on_click == other.on_click
            && self.on_hover_enter == other.on_hover_enter
            && self.on_hover_leave == other.on_hover_leave
            && self.on_key_down == other.on_key_down
            && self.on_key_up == other.on_key_up
            && self.on_frame == other.on_frame
            && self.on_tick == other.on_tick
            && self.every_ms == other.every_ms
            && self.asset == other.asset
            && self.key == other.key
            && self.sound == other.sound
            && self.svg == other.svg
            && self.route == other.route
            && self.target == other.target
            && self.reference == other.reference
            && self.on_js == other.on_js
            && self.children.len() == other.children.len()
            && self
                .children
                .iter()
                .zip(&other.children)
                .all(|(a, b)| Rc::ptr_eq(a, b) || **a == **b)
    }
}

/// Recursively converts the node at `id` (and its descendants) into a
/// `UiRenderNode`. `id` is expected to come from `tree` itself (the
/// synthetic root `UiRuntime::mount` produces, or one of its
/// descendants) -- every caller in this codebase satisfies that.
pub fn to_render_tree(tree: &UiTree, id: UiNodeId) -> UiRenderNode {
    let node = &tree.nodes[id];
    // A reused subtree stands in for its whole (already converted) shape.
    if let Some(prebuilt) = &node.prebuilt {
        return (**prebuilt).clone();
    }
    UiRenderNode {
        tag: node.tag.clone(),
        tint_source: node.tint_source.clone(),
        text: node.text.clone(),
        style: node.style.clone(),
        hover_style: node.hover_style.clone(),
        breakpoints: node.breakpoints.clone(),
        on_click: node.on_click.clone(),
        on_hover_enter: node.on_hover_enter.clone(),
        on_hover_leave: node.on_hover_leave.clone(),
        on_key_down: node.on_key_down.clone(),
        on_key_up: node.on_key_up.clone(),
        on_frame: node.on_frame.clone(),
        on_tick: node.on_tick.clone(),
        every_ms: node.every_ms,
        asset: node.asset.clone(),
        key: node.key.clone(),
        sound: node.sound.clone(),
        svg: node.svg.clone(),
        route: node.route.clone(),
        target: node.target.clone(),
        reference: node.reference.clone(),
        on_js: node.on_js.clone(),
        children: node
            .children
            .iter()
            .map(|&cid| to_render_rc(tree, cid))
            .collect(),
    }
}

/// Like `to_render_tree`, but shared: a node standing in for a reused
/// subtree hands back the same `Rc` it was cached under.
pub fn to_render_rc(tree: &UiTree, id: UiNodeId) -> Rc<UiRenderNode> {
    match &tree.nodes[id].prebuilt {
        Some(prebuilt) => Rc::clone(prebuilt),
        None => Rc::new(to_render_tree(tree, id)),
    }
}

pub struct Renderer;

impl Renderer {
    pub fn render() {
        // TODO: integrate with Lynbor GPU
    }
}
