// tint-wasm/src/dom.rs
//
// A second rendering backend for the same UI tree tint_runtime::ui_session
// already produces, alongside the existing Svelte-based one (UiSession in
// lib.rs: hands a serialized Vec<UiRenderNode> to JS, UiPreviewNode.svelte
// turns it into DOM). This one skips JS/Svelte entirely -- Rust calls
// web-sys DOM APIs directly to build real elements from the tree.
//
// Deliberately NOT a diffing renderer: every render (initial mount, or
// after a click/hover_in/hover_out dispatch) tears down the container's
// children and rebuilds the whole subtree from scratch. That matches the
// existing Svelte path's own behavior (Preview.svelte swaps in the whole
// returned tree on every dispatch, no reconciliation) -- see
// tint_runtime::ui_session's doc comment. A real diff/patch pass (to keep
// focus/scroll/animation state across renders) is future work.
//
// Known cost of "rebuild + web-sys event closures": every build_node call
// that attaches a listener does `closure.forget()`, which is the standard
// wasm-bindgen idiom for a closure that must outlive the function that
// created it -- but it also means the closure's memory is never freed,
// even once its element is torn down by the next rebuild. Each click/hover
// therefore leaks the previous render's closures. Fine for a sandbox/demo
// session; not something to ship in a long-running app without adding
// real diffing (which reuses elements/listeners instead of recreating
// them) first.
//
// Additive: UiSession (the JS-facing, Svelte-driving type in lib.rs) is
// untouched. DomSession is a separate, new wasm-bindgen export the
// sandbox can opt into independently.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{Document, Element};

use tint_runtime::ui::render::UiRenderNode;
use tint_runtime::ui_session::UiSession as InnerSession;

/// Shared state a click/hover closure needs: the live session (`state`
/// lives inside it) and where in the DOM to rebuild. Cheap to clone
/// (it's an `Rc`) so every closure can hold its own copy.
struct Shared {
    session: RefCell<InnerSession>,
    container_id: String,
    /// Handle from `window.setTimeout`, used to debounce the `resize`
    /// listener bound in `bind_resize_listener` -- a resize storm
    /// (dragging a window edge) should trigger one rebuild after it
    /// settles, not one per event.
    resize_timeout: Cell<Option<i32>>,
}

/// Direct-DOM counterpart to `UiSession` in lib.rs. Wraps the exact same
/// `tint_runtime::ui_session::UiSession` (so `state`/`click||`/
/// `hover_in||`/`hover_out||` all behave identically), but instead of
/// handing JS a tree to render, this builds real DOM nodes into
/// `container_id` itself.
///
/// Construction never touches the DOM and can't fail loudly -- same
/// convention as `UiSession::new` (bad source / unknown `ui_fn_name` is
/// stored, not thrown). Call `rerender()` once after constructing to do
/// the first paint, exactly like `UiSession::new` + `.tree()`.
#[wasm_bindgen]
pub struct DomSession {
    shared: Option<Rc<Shared>>,
    init_error: Option<String>,
}

#[wasm_bindgen]
impl DomSession {
    #[wasm_bindgen(constructor)]
    pub fn new(source: &str, ui_fn_name: &str, container_id: &str) -> DomSession {
        match InnerSession::new(source, ui_fn_name) {
            Ok(session) => {
                let shared = Rc::new(Shared {
                    session: RefCell::new(session),
                    container_id: container_id.to_string(),
                    resize_timeout: Cell::new(None),
                });
                bind_resize_listener(&shared);
                DomSession {
                    shared: Some(shared),
                    init_error: None,
                }
            }
            Err(e) => DomSession {
                shared: None,
                init_error: Some(e),
            },
        }
    }

    /// Re-renders the session's current tree into the container,
    /// replacing whatever was there. Returns `Some(error)` instead of
    /// throwing; `None` means it went fine.
    pub fn rerender(&mut self) -> Option<String> {
        let shared = match &self.shared {
            Some(s) => s.clone(),
            None => return self.init_error.clone(),
        };
        let tree = match shared.session.borrow_mut().render() {
            Ok(tree) => tree,
            Err(e) => return Some(e),
        };
        mount_tree(&tree, &shared).err().map(|e| js_error_to_string(&e))
    }

    /// Runs `handler` against this session's persistent state (same
    /// dispatch `tint_runtime::ui_session::UiSession::dispatch` already
    /// does for the Svelte path), then rebuilds the DOM from the result.
    pub fn dispatch(&mut self, handler: &str) -> Option<String> {
        let shared = match &self.shared {
            Some(s) => s.clone(),
            None => return self.init_error.clone(),
        };
        let tree = match shared.session.borrow_mut().dispatch(handler) {
            Ok(tree) => tree,
            Err(e) => return Some(e),
        };
        mount_tree(&tree, &shared).err().map(|e| js_error_to_string(&e))
    }
}

fn js_error_to_string(e: &JsValue) -> String {
    e.as_string().unwrap_or_else(|| format!("{:?}", e))
}

fn document() -> Result<Document, JsValue> {
    web_sys::window()
        .ok_or_else(|| JsValue::from_str("no window"))?
        .document()
        .ok_or_else(|| JsValue::from_str("no document"))
}

/// Breakpoint thresholds, in CSS pixels, mirroring the four names
/// ui/style.rs recognizes for `mobile::{}`/`tablet::{}`/`laptop::{}`/
/// `desktop::{}` modifiers: below 640px is "mobile", below 1024px is
/// "tablet", below 1440px is "laptop", anything wider is "desktop".
fn breakpoint_for_width(width: f64) -> &'static str {
    if width < 640.0 {
        "mobile"
    } else if width < 1024.0 {
        "tablet"
    } else if width < 1440.0 {
        "laptop"
    } else {
        "desktop"
    }
}

/// Reads the live viewport width from `window.innerWidth` and maps it
/// to a breakpoint name via `breakpoint_for_width`. Falls back to
/// "desktop" if there's no window or it can't be read, keeping this
/// infallible like the rest of the module.
fn current_breakpoint() -> String {
    let width = web_sys::window()
        .and_then(|w| w.inner_width().ok())
        .and_then(|v| v.as_f64())
        .unwrap_or(1440.0);
    breakpoint_for_width(width).to_string()
}

/// Binds a debounced `resize` listener (once per `DomSession`, from
/// the constructor) that re-renders and rebuilds the whole tree so
/// `mobile::{}`/`tablet::{}`/`laptop::{}`/`desktop::{}` styles pick up
/// a viewport change without any click/hover in between. Debounced via
/// `shared.resize_timeout` (150ms) so a window being dragged across a
/// breakpoint doesn't rebuild on every intermediate `resize` event.
///
/// Leaks its closure the same way every other listener in this module
/// does (see the module doc comment) -- fine for a session that lives
/// as long as the page, one listener total per `DomSession`.
fn bind_resize_listener(shared: &Rc<Shared>) {
    let window = match web_sys::window() {
        Some(w) => w,
        None => return,
    };

    let shared = shared.clone();
    let resize_cb = Closure::wrap(Box::new(move |_e: web_sys::Event| {
        let window = match web_sys::window() {
            Some(w) => w,
            None => return,
        };

        if let Some(existing) = shared.resize_timeout.take() {
            window.clear_timeout_with_handle(existing);
        }

        let shared_for_timeout = shared.clone();
        let fire_cb = Closure::wrap(Box::new(move || {
            let tree = match shared_for_timeout.session.borrow_mut().render() {
                Ok(tree) => tree,
                Err(e) => {
                    web_sys::console::error_1(&JsValue::from_str(&format!(
                        "tint: resize re-render failed: {}",
                        e
                    )));
                    return;
                }
            };
            if let Err(e) = mount_tree(&tree, &shared_for_timeout) {
                web_sys::console::error_1(&e);
            }
        }) as Box<dyn FnMut()>);

        if let Ok(handle) = window.set_timeout_with_callback_and_timeout_and_arguments_0(
            fire_cb.as_ref().unchecked_ref(),
            150,
        ) {
            shared.resize_timeout.set(Some(handle));
        }
        fire_cb.forget();
    }) as Box<dyn FnMut(_)>);

    let _ = window.add_event_listener_with_callback("resize", resize_cb.as_ref().unchecked_ref());
    resize_cb.forget();
}

/// Clears `shared.container_id`'s children and rebuilds them from
/// `tree`. Whole-subtree teardown/rebuild, not a diff -- see this
/// module's doc comment.
fn mount_tree(tree: &[UiRenderNode], shared: &Rc<Shared>) -> Result<(), JsValue> {
    let document = document()?;
    let container = document
        .get_element_by_id(&shared.container_id)
        .ok_or_else(|| JsValue::from_str("container element not found"))?;

    while let Some(child) = container.first_child() {
        container.remove_child(&child)?;
    }

    // Read the viewport once per rebuild (not once per node) so a whole
    // tree is consistent even if the resolution race between reading
    // innerWidth and finishing the rebuild -- not realistic, but cheap
    // to make free.
    let breakpoint = current_breakpoint();
    for node in tree {
        let el = build_node(&document, node, shared, &breakpoint)?;
        container.append_child(&el)?;
    }
    Ok(())
}

fn apply_style_props(el: &Element, props: &[(String, String)]) -> Result<(), JsValue> {
    let html_el: &web_sys::HtmlElement = el
        .dyn_ref()
        .ok_or_else(|| JsValue::from_str("not an HtmlElement"))?;
    let style = html_el.style();
    for (k, v) in props {
        style.set_property(k, v)?;
    }
    Ok(())
}

/// `node.style` as a single CSS text string, for resetting an element's
/// whole `style` attribute on `mouseleave`. Needed because `mouseenter`
/// only ever ADDS properties on top via `set_property` (see
/// `build_node`) -- leaving has to wipe those additions, not just
/// re-apply the base list on top of an already-hover-dirty style.
fn style_to_css_text(props: &[(String, String)]) -> String {
    props
        .iter()
        .map(|(k, v)| format!("{}: {};", k, v))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Neutralizes the browser's own UA stylesheet for `<button>` (border,
/// background, padding, font -- the "grey embossed" default look), the
/// same reset `UiPreviewNode.svelte` applies for the same reason: without
/// it, a Button with nothing style-related set by the language still
/// shows the browser's chrome, which looks like *our* renderer invented a
/// button style when it didn't. Deliberately NOT adding `cursor: pointer`
/// here either -- that was explicitly removed on the Svelte side as an
/// invented default the language itself never asked for; see this
/// crate's sibling ui/style.rs and the Svelte component's own history.
const BUTTON_RESET: &[(&str, &str)] = &[
    ("appearance", "none"),
    ("background", "none"),
    ("border", "none"),
    ("padding", "0"),
    ("margin", "0"),
    ("font", "inherit"),
    ("color", "inherit"),
    ("text-align", "inherit"),
];

fn build_node(
    document: &Document,
    node: &UiRenderNode,
    shared: &Rc<Shared>,
    breakpoint: &str,
) -> Result<Element, JsValue> {
    // Mirrors UiPreviewNode.svelte's own tag choice: a real <button> for
    // Button/MenuItem or anything with a click handler (so it gets free
    // keyboard/focus/AT behavior), a plain <div> otherwise.
    let is_button = node.tag == "Button" || node.tag == "MenuItem" || node.on_click.is_some();
    let tag_name = if is_button { "button" } else { "div" };
    let el = document.create_element(tag_name)?;
    el.set_attribute("data-tag", &node.tag)?;

    if let Some(svg) = &node.svg {
        el.set_inner_html(svg);
    } else if let Some(text) = &node.text {
        el.set_text_content(Some(text));
    }

    // Base = UA reset (buttons only) + whatever the language actually
    // resolved. Kept as one combined list (not two separate applies)
    // because mouseleave below needs to restore exactly this, including
    // the reset -- resetting the whole `style` attribute to just
    // `node.style` would bring the browser's button chrome right back.
    let mut base_props: Vec<(String, String)> = Vec::new();
    if is_button {
        base_props.extend(BUTTON_RESET.iter().map(|(k, v)| (k.to_string(), v.to_string())));
    }
    base_props.extend(node.style.iter().cloned());

    // Layer the current viewport's breakpoint style (if the node has
    // one) on top of the base style -- last-write-wins per property,
    // same as a CSS media query overriding a base rule.
    if let Some((_, bp_style)) = node.breakpoints.iter().find(|(name, _)| name == breakpoint) {
        base_props.extend(bp_style.iter().cloned());
    }

    apply_style_props(&el, &base_props)?;

    if !node.hover_style.is_empty() {
        let base_css = style_to_css_text(&base_props);
        let hover_style = node.hover_style.clone();

        let el_enter = el.clone();
        let enter_cb = Closure::wrap(Box::new(move |_e: web_sys::Event| {
            let _ = apply_style_props(&el_enter, &hover_style);
        }) as Box<dyn FnMut(_)>);
        el.add_event_listener_with_callback("mouseenter", enter_cb.as_ref().unchecked_ref())?;
        enter_cb.forget();

        let el_leave = el.clone();
        let leave_cb = Closure::wrap(Box::new(move |_e: web_sys::Event| {
            let _ = el_leave.set_attribute("style", &base_css);
        }) as Box<dyn FnMut(_)>);
        el.add_event_listener_with_callback("mouseleave", leave_cb.as_ref().unchecked_ref())?;
        leave_cb.forget();
    }

    if let Some(handler) = node.on_click.clone() {
        bind_dispatch(&el, "click", handler, shared);
    }
    if let Some(handler) = node.on_hover_enter.clone() {
        bind_dispatch(&el, "mouseenter", handler, shared);
    }
    if let Some(handler) = node.on_hover_leave.clone() {
        bind_dispatch(&el, "mouseleave", handler, shared);
    }

    if node.text.is_none() && node.svg.is_none() {
        for child in &node.children {
            let child_el = build_node(document, child, shared, breakpoint)?;
            el.append_child(&child_el)?;
        }
    }

    Ok(el)
}

/// Wires `event_name` on `el` to run `handler` against the session and
/// rebuild the DOM -- the same dispatch-then-rerender loop
/// `DomSession::dispatch` exposes to JS, just triggered by a real DOM
/// event. Bound as its own `addEventListener` call, alongside (not
/// instead of) any style-only hover listener already bound above for
/// the same event name.
fn bind_dispatch(el: &Element, event_name: &'static str, handler: String, shared: &Rc<Shared>) {
    let shared = shared.clone();
    let cb = Closure::wrap(Box::new(move |_e: web_sys::Event| {
        let tree = match shared.session.borrow_mut().dispatch(&handler) {
            Ok(tree) => tree,
            Err(e) => {
                web_sys::console::error_1(&JsValue::from_str(&format!(
                    "tint: dispatch({}) failed: {}",
                    handler, e
                )));
                return;
            }
        };
        if let Err(e) = mount_tree(&tree, &shared) {
            web_sys::console::error_1(&e);
        }
    }) as Box<dyn FnMut(_)>);
    // Ignore add_event_listener's own Result: a failure here means the
    // element itself is broken, which document.create_element's Result
    // earlier in build_node would already have surfaced.
    let _ = el.add_event_listener_with_callback(event_name, cb.as_ref().unchecked_ref());
    cb.forget();
}
