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
    pub style: Rc<Vec<(String, String)>>,
    pub hover_style: Rc<Vec<(String, String)>>,
    pub breakpoints: Rc<Vec<(String, Vec<(String, String)>)>>,
    pub on_click: Option<String>,
    pub on_hover_enter: Option<String>,
    pub on_hover_leave: Option<String>,
    pub on_key_down: Option<String>,
    pub on_key_up: Option<String>,
    pub on_frame: Option<String>,
    pub on_tick: Option<String>,
    pub on_pointer_start: Option<String>,
    pub on_pointer_move: Option<String>,
    pub on_pointer_up: Option<String>,
    pub on_input: Option<String>,
    pub preview_entry: Option<String>,
    pub on_submit: Option<String>,
    pub every_ms: Option<f64>,
    pub asset: Option<String>,
    pub key: Option<String>,
    pub sound: Option<String>,
    pub svg: Option<String>,
    pub route: Option<String>,
    pub target: Option<String>,
    pub reference: Option<String>,
    pub class: Option<String>,
    pub component: Option<String>,
    pub props: Option<String>,
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
            && self.on_pointer_start == other.on_pointer_start
            && self.on_pointer_move == other.on_pointer_move
            && self.on_pointer_up == other.on_pointer_up
            && self.on_input == other.on_input
            && self.preview_entry == other.preview_entry
            && self.on_submit == other.on_submit
            && self.every_ms == other.every_ms
            && self.asset == other.asset
            && self.key == other.key
            && self.sound == other.sound
            && self.svg == other.svg
            && self.route == other.route
            && self.target == other.target
            && self.reference == other.reference
            && self.class == other.class
            && self.component == other.component
            && self.props == other.props
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
/// Hosts that render to a DOM turn this on: a node whose only child is a plain
/// string then carries that string as its own `text` (one element, not two).
/// Off by default so the tree keeps the shape the language describes.
pub fn fold_text_enabled() -> bool {
    FOLD_TEXT.load(std::sync::atomic::Ordering::Relaxed)
}

pub static FOLD_TEXT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

thread_local! {
    static EMPTY_STYLE: Rc<Vec<(String, String)>> = Rc::new(Vec::new());
    static EMPTY_BPS: Rc<Vec<(String, Vec<(String, String)>)>> = Rc::new(Vec::new());
}

fn empty_bps() -> Rc<Vec<(String, Vec<(String, String)>)>> {
    EMPTY_BPS.with(Rc::clone)
}

/// The shared style list if the node came from the style memo, else a fresh
/// copy (or the shared empty list).
fn share(
    bundle: &Option<Rc<super::tree::StyleBundle>>,
    pick: impl Fn(&super::tree::StyleBundle) -> &Rc<Vec<(String, String)>>,
    own: &Vec<(String, String)>,
) -> Rc<Vec<(String, String)>> {
    match bundle {
        Some(b) => Rc::clone(pick(b)),
        None if own.is_empty() => EMPTY_STYLE.with(Rc::clone),
        None => Rc::new(own.clone()),
    }
}

pub fn to_render_tree(tree: &UiTree, id: UiNodeId) -> UiRenderNode {
    let node = &tree.nodes[id];
    // A reused subtree stands in for its whole (already converted) shape.
    if let Some(prebuilt) = &node.prebuilt {
        return (**prebuilt).clone();
    }
    // `TextArea` and `Preview` carry their text as an attribute of the element
    // (value / source), so their string children fold into `text`.
    let folds_text = matches!(node.tag.as_str(), "TextArea" | "Preview");
    // A lone plain string child (`Text { "{x}" }`) becomes the node's own text:
    // one element instead of two.
    let lone_text = if FOLD_TEXT.load(std::sync::atomic::Ordering::Relaxed)
        && node.text.is_none()
        && node.children.len() == 1
        && !folds_text
    {
        let child = &tree.nodes[node.children[0]];
        let bare = child.text.is_some()
            && child.children.is_empty()
            && child.prebuilt.is_none()
            && child.tag == "Text"
            && child.style.is_empty()
            && child.shared_style.is_none()
            && child.hover_style.is_empty()
            && child.breakpoints.is_empty()
            && child.on_click.is_none()
            && child.key.is_none();
        bare.then(|| child.text.clone()).flatten()
    } else {
        None
    };
    let text = if lone_text.is_some() {
        lone_text.clone()
    } else if folds_text && node.text.is_none() {
        Some(
            node.children
                .iter()
                .filter_map(|&cid| tree.nodes[cid].text.as_deref())
                .collect::<String>(),
        )
    } else {
        node.text.clone()
    };
    UiRenderNode {
        tag: node.tag.clone(),
        tint_source: node.tint_source.clone(),
        text,
        style: share(&node.shared_style, |b| &b.style, &node.style),
        hover_style: share(&node.shared_style, |b| &b.hover_style, &node.hover_style),
        breakpoints: match &node.shared_style {
            Some(b) => Rc::clone(&b.breakpoints),
            None if node.breakpoints.is_empty() => empty_bps(),
            None => Rc::new(node.breakpoints.clone()),
        },
        on_click: node.on_click.clone(),
        on_hover_enter: node.on_hover_enter.clone(),
        on_hover_leave: node.on_hover_leave.clone(),
        on_key_down: node.on_key_down.clone(),
        on_key_up: node.on_key_up.clone(),
        on_frame: node.on_frame.clone(),
        on_tick: node.on_tick.clone(),
        on_pointer_start: node.on_pointer_start.clone(),
        on_pointer_move: node.on_pointer_move.clone(),
        on_pointer_up: node.on_pointer_up.clone(),
        on_input: node.on_input.clone(),
        preview_entry: node.preview_entry.clone(),
        on_submit: node.on_submit.clone(),
        every_ms: node.every_ms,
        asset: node.asset.clone(),
        key: node.key.clone(),
        sound: node.sound.clone(),
        svg: node.svg.clone(),
        route: node.route.clone(),
        target: node.target.clone(),
        reference: node.reference.clone(),
        class: node.class.clone(),
        component: node.component.clone(),
        props: node.props.clone(),
        on_js: node.on_js.clone(),
        children: if folds_text || lone_text.is_some() {
            Vec::new()
        } else {
            node.children
                .iter()
                .map(|&cid| to_render_rc(tree, cid))
                .collect()
        },
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
