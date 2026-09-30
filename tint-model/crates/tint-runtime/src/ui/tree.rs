// ui/tree.rs

use super::style::StyleList;
use std::collections::HashMap;
use std::rc::Rc;
use tint_ast::UiModifierValue;

use super::render::UiRenderNode;

pub type UiNodeId = usize;

/// Whether nodes carry their `Tag { modifiers }` source text (for inspecting the
/// DOM). Off by default: building that string for every node on every render
/// costs more than the rest of the node.
pub static SOURCE_MAP: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Resolved styles shared between every node built from the same modifiers.
#[derive(Debug)]
pub struct StyleBundle {
    pub style: Rc<StyleList>,
    pub hover_style: Rc<StyleList>,
    pub breakpoints: Rc<Vec<(String, StyleList)>>,
}

#[derive(Debug, Clone)]
pub struct UiElement {
    /// When set, stands in for `style`/`hover_style`/`breakpoints` (left empty).
    pub shared_style: Option<Rc<StyleBundle>>,
    pub id: UiNodeId,
    pub tag: String,
    pub tint_source: String,
    pub children: Vec<UiNodeId>,
    /// Resolved from this node's `key::value` modifiers (background,
    /// padding, radius, ...) -- see ui/style.rs.
    pub style: StyleList,
    /// Resolved from this node's `hover::{ ... }` modifier, if any. Empty
    /// when the node has none -- a renderer should only emit a `:hover`
    /// rule when this is non-empty, rather than assuming every node (or
    /// every node of a given tag) animates on hover.
    pub hover_style: StyleList,
    /// Resolved from this node's `mobile::{ ... }`/`tablet::{ ... }`/
    /// `laptop::{ ... }`/`desktop::{ ... }` modifiers, if any -- each
    /// entry is (breakpoint name, its resolved style list). A renderer
    /// picks the entry matching the current viewport and layers it over
    /// `style` -- see ui/style.rs.
    pub breakpoints: Vec<(String, StyleList)>,
    /// Text for a `Text`-shaped child: literal parts verbatim, and any
    /// interpolation genuinely evaluated (see builder.rs's
    /// `render_ui_text`, which threads a real `EvalHost` through now).
    pub text: Option<String>,

    /// Handler (a plain `fn` name) from this node's `click||handler`
    /// attribute, if any -- see builder.rs's `find_handler`.
    pub on_click: Option<String>,
    /// Handler from `hover_in||handler`.
    pub on_hover_enter: Option<String>,
    /// Handler from `hover_out||handler`.
    pub on_hover_leave: Option<String>,
    pub on_key_down: Option<String>,
    pub on_key_up: Option<String>,
    pub on_frame: Option<String>,
    /// Handler from `tick||handler`, run every `every_ms` milliseconds.
    pub on_tick: Option<String>,
    pub on_pointer_start: Option<String>,
    pub on_pointer_move: Option<String>,
    pub on_pointer_up: Option<String>,
    pub on_input: Option<String>,
    pub preview_entry: Option<String>,
    pub on_submit: Option<String>,
    /// Interval from `every||ms` (used with `tick||`).
    pub every_ms: Option<f64>,
    pub asset: Option<String>,
    pub key: Option<String>,
    pub sound: Option<String>,

    /// Raw inline SVG markup from this node's `svg||"<svg ...>...</svg>"`
    /// attribute (a `UiAttrValue::Literal`, not `Ident` -- see
    /// builder.rs's `apply_svg`). When set, a renderer should show this
    /// verbatim instead of the node's text/children -- this is "reading
    /// SVG" the same way `text`/`style` are "reading" everything else:
    /// the markup comes straight from the .tint source, nothing invented
    /// on the sandbox side.
    pub svg: Option<String>,

    /// Internal route target from `route||"/path"`. Renderers expose this
    /// as a normal browser link instead of inventing navigation in the host.
    pub route: Option<String>,
    /// Optional link target from target||"_blank".
    pub target: Option<String>,
    /// Stable DOM escape-hatch name from ref||"name".
    pub reference: Option<String>,
    /// Host callback name from js||callback.
    pub on_js: Option<String>,

    /// Set when this node stands for a subtree reused from the previous
    /// render (see builder/cache.rs): its finished, shared render form.
    pub prebuilt: Option<Rc<UiRenderNode>>,
}

impl UiElement {
    pub fn new(id: UiNodeId, tag: String) -> Self {
        Self {
            id,
            tag,
            tint_source: String::new(),
            shared_style: None,
            children: Vec::new(),
            style: Vec::new(),
            hover_style: Vec::new(),
            breakpoints: Vec::new(),
            text: None,
            on_click: None,
            on_hover_enter: None,
            on_hover_leave: None,
            on_key_down: None,
            on_key_up: None,
            on_frame: None,
            on_tick: None,
            on_pointer_start: None,
            on_pointer_move: None,
            on_pointer_up: None,
            on_input: None,
            preview_entry: None,
            on_submit: None,
            every_ms: None,
            asset: None,
            key: None,
            sound: None,
            svg: None,
            route: None,
            target: None,
            reference: None,
            on_js: None,
            prebuilt: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct UiTree {
    pub nodes: Vec<UiElement>,
}

impl UiTree {
    pub fn empty() -> Self {
        Self { nodes: Vec::new() }
    }

    pub fn create_node(&mut self, tag: String) -> UiNodeId {
        let id = self.nodes.len();
        self.nodes.push(UiElement::new(id, tag));
        id
    }

    /// Same as `create_node`, but also resolves `modifiers` (via
    /// ui/style.rs) into the new node's `style`/`hover_style`.
    pub fn create_styled_node(
        &mut self,
        tag: String,
        modifiers: &[tint_ast::UiModifier],
    ) -> UiNodeId {
        let id = self.create_node(tag.clone());
        let (mut style, hover_style, breakpoints) = super::style::resolve_style(modifiers);
        super::style::apply_container_defaults(&tag, &mut style);
        let node = &mut self.nodes[id];
        if SOURCE_MAP.load(std::sync::atomic::Ordering::Relaxed) {
            node.tint_source = format!(
                "{} {{ {} }}",
                node.tag,
                super::style::format_modifier_source(modifiers)
            );
        }
        node.style = style;
        node.hover_style = hover_style;
        node.breakpoints = breakpoints;
        id
    }

    pub fn create_styled_node_with_host<H: tint_evaluator::EvalHost>(
        &mut self,
        tag: String,
        modifiers: &[tint_ast::UiModifier],
        host: &mut H,
    ) -> UiNodeId {
        let id = self.create_node(tag.clone());
        let (mut style, hover_style, breakpoints) =
            super::style::resolve_style_with_host(modifiers, host);
        super::style::apply_container_defaults(&tag, &mut style);
        let node = &mut self.nodes[id];
        if SOURCE_MAP.load(std::sync::atomic::Ordering::Relaxed) {
            node.tint_source = format!(
                "{} {{ {} }}",
                node.tag,
                super::style::format_modifier_source(modifiers)
            );
        }
        node.style = style;
        node.hover_style = hover_style;
        node.breakpoints = breakpoints;
        id
    }

    pub fn create_styled_node_with_host_and_tokens<H: tint_evaluator::EvalHost>(
        &mut self,
        tag: String,
        modifiers: &[tint_ast::UiModifier],
        host: &mut H,
        tokens: &HashMap<String, UiModifierValue>,
    ) -> UiNodeId {
        self.create_styled_node_with_eval(tag, modifiers, &mut |expr| host.eval_expr(expr), tokens)
    }

    /// A node with an already-resolved, shared style (see `builder::cache::StyleMemo`).
    pub fn create_node_with_bundle(&mut self, tag: String, bundle: &Rc<StyleBundle>) -> UiNodeId {
        let id = self.create_node(tag);
        self.nodes[id].shared_style = Some(Rc::clone(bundle));
        id
    }

    pub fn create_styled_node_with_eval(
        &mut self,
        tag: String,
        modifiers: &[tint_ast::UiModifier],
        eval: &mut dyn FnMut(&tint_ast::Expr) -> tint_evaluator::Value,
        tokens: &HashMap<String, UiModifierValue>,
    ) -> UiNodeId {
        let id = self.create_node(tag.clone());
        let (mut style, hover_style, breakpoints) =
            super::style::resolve_style_with_eval(modifiers, eval, tokens);
        super::style::apply_container_defaults(&tag, &mut style);
        let node = &mut self.nodes[id];
        if SOURCE_MAP.load(std::sync::atomic::Ordering::Relaxed) {
            node.tint_source = format!(
                "{} {{ {} }}",
                node.tag,
                super::style::format_modifier_source(modifiers)
            );
        }
        node.style = style;
        node.hover_style = hover_style;
        node.breakpoints = breakpoints;
        id
    }

    /// Sets the event handlers resolved from a node's `click||`/
    /// `hover_in||`/`hover_out||` attributes (see builder.rs's
    /// `find_handler`). A `None` leaves that slot untouched -- there's
    /// nothing to clear, every node starts with all three `None`.
    pub fn set_events(
        &mut self,
        id: UiNodeId,
        on_click: Option<String>,
        on_hover_enter: Option<String>,
        on_hover_leave: Option<String>,
        on_key_down: Option<String>,
        on_key_up: Option<String>,
        on_frame: Option<String>,
    ) {
        if let Some(node) = self.nodes.get_mut(id) {
            node.on_click = on_click;
            node.on_hover_enter = on_hover_enter;
            node.on_hover_leave = on_hover_leave;
            node.on_key_down = on_key_down;
            node.on_key_up = on_key_up;
            node.on_frame = on_frame;
        }
    }

    pub fn set_preview_entry(&mut self, id: UiNodeId, entry: Option<String>) {
        if let Some(node) = self.nodes.get_mut(id) {
            node.preview_entry = entry;
        }
    }

    pub fn set_input(&mut self, id: UiNodeId, on_input: Option<String>, on_submit: Option<String>) {
        if let Some(node) = self.nodes.get_mut(id) {
            node.on_input = on_input;
            node.on_submit = on_submit;
        }
    }

    pub fn set_pointer(
        &mut self,
        id: UiNodeId,
        start: Option<String>,
        on_move: Option<String>,
        up: Option<String>,
    ) {
        if let Some(node) = self.nodes.get_mut(id) {
            node.on_pointer_start = start;
            node.on_pointer_move = on_move;
            node.on_pointer_up = up;
        }
    }

    pub fn set_tick(&mut self, id: UiNodeId, handler: Option<String>, every_ms: Option<f64>) {
        if let Some(node) = self.nodes.get_mut(id) {
            node.on_tick = handler;
            node.every_ms = every_ms;
        }
    }

    pub fn set_asset(&mut self, id: UiNodeId, asset: Option<String>) {
        if let Some(node) = self.nodes.get_mut(id) {
            node.asset = asset;
        }
    }

    pub fn set_key(&mut self, id: UiNodeId, key: Option<String>) {
        if let Some(node) = self.nodes.get_mut(id) {
            node.key = key;
        }
    }

    pub fn set_sound(&mut self, id: UiNodeId, sound: Option<String>) {
        if let Some(node) = self.nodes.get_mut(id) {
            node.sound = sound;
        }
    }

    /// Sets the raw SVG markup resolved from a node's `svg||"..."`
    /// attribute (see builder.rs's `apply_svg`). `None` leaves it unset,
    /// same convention as `set_events`.
    pub fn set_svg(&mut self, id: UiNodeId, svg: Option<String>) {
        if let Some(node) = self.nodes.get_mut(id) {
            node.svg = svg;
        }
    }

    pub fn set_route(&mut self, id: UiNodeId, route: Option<String>) {
        if let Some(node) = self.nodes.get_mut(id) {
            node.route = route;
        }
    }

    pub fn set_target(&mut self, id: UiNodeId, target: Option<String>) {
        if let Some(node) = self.nodes.get_mut(id) {
            node.target = target;
        }
    }

    pub fn set_reference(&mut self, id: UiNodeId, reference: Option<String>) {
        if let Some(node) = self.nodes.get_mut(id) {
            node.reference = reference;
        }
    }

    pub fn set_js_handler(&mut self, id: UiNodeId, handler: Option<String>) {
        if let Some(node) = self.nodes.get_mut(id) {
            node.on_js = handler;
        }
    }

    pub fn add_child(&mut self, parent: UiNodeId, child: UiNodeId) {
        if let Some(p) = self.nodes.get_mut(parent) {
            p.children.push(child);
        }
    }

    /// Creates a `Text` node carrying `text` directly (see
    /// `UiElement::text`), skipping style resolution since a plain text
    /// node has no modifiers of its own.
    pub fn create_text_node(&mut self, text: String) -> UiNodeId {
        let id = self.create_node("Text".to_string());
        self.nodes[id].text = Some(text);
        id
    }
}
