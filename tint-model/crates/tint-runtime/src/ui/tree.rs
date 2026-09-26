// ui/tree.rs

use super::style::StyleList;

pub type UiNodeId = usize;

#[derive(Debug, Clone)]
pub struct UiElement {
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
}

impl UiElement {
    pub fn new(id: UiNodeId, tag: String) -> Self {
        Self {
            id,
            tag,
            tint_source: String::new(),
            children: Vec::new(),
            style: Vec::new(),
            hover_style: Vec::new(),
            breakpoints: Vec::new(),
            text: None,
            on_click: None,
            on_hover_enter: None,
            on_hover_leave: None,
            svg: None,
            route: None,
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
        let id = self.create_node(tag);
        let (style, hover_style, breakpoints) = super::style::resolve_style(modifiers);
        let node = &mut self.nodes[id];
        node.tint_source = format!(
            "{} {{ {} }}",
            node.tag,
            super::style::format_modifier_source(modifiers)
        );
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
    ) {
        if let Some(node) = self.nodes.get_mut(id) {
            node.on_click = on_click;
            node.on_hover_enter = on_hover_enter;
            node.on_hover_leave = on_hover_leave;
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
