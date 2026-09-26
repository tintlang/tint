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

/// Reads the live viewport width from `window.innerWidth`. Falls back
/// to a desktop-ish 1440px if there's no window or it can't be read,
/// keeping this infallible like the rest of the module -- same
/// fallback `UiSession::new` seeds `viewport_width` with, so a session
/// that never gets a real width still renders consistently.
fn current_viewport_width() -> f64 {
    web_sys::window()
        .and_then(|w| w.inner_width().ok())
        .and_then(|v| v.as_f64())
        .unwrap_or(1440.0)
}

/// Maps the live viewport width to a breakpoint name via
/// `breakpoint_for_width`.
fn current_breakpoint() -> String {
    breakpoint_for_width(current_viewport_width()).to_string()
}

/// Pushes the current viewport width into the session's
/// `viewport_width` variable (see `UiSession::set_viewport_width`) so
/// an `if{}` directive in the .tint source itself can branch on
/// viewport size structurally -- not just the style-only breakpoints
/// `build_node` layers in separately. Called right before every
/// `render()`/`dispatch()` so the structural condition and the style
/// breakpoint applied to the very same tree always agree on one width.
fn sync_viewport_width(shared: &Rc<Shared>) {
    let width = current_viewport_width();
    shared.session.borrow_mut().set_viewport_width(width);
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
            sync_viewport_width(&shared_for_timeout);
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

    // Read the viewport once per rebuild (not once per node) so a whole
    // tree is consistent even if the resolution race between reading
    // innerWidth and finishing the rebuild -- not realistic, but cheap
    // to make free.
    let breakpoint = current_breakpoint();

    // Build the whole new subtree off-DOM first, into a fragment nothing
    // renders, instead of clearing the live container and then building
    // node-by-node into it. A slower device (mobile, in particular) can
    // take a visible number of milliseconds to build/style/attach
    // listeners for a whole tree -- doing that while the container is
    // already empty risks a real blank flash; building it here means the
    // container only ever goes from "old tree" to "new tree" in the two
    // tight calls below, with no empty state in between for the browser
    // to ever paint.
    let fragment = document.create_document_fragment();
    for node in tree {
        let el = build_node(&document, node, shared, &breakpoint)?;
        fragment.append_child(&el)?;
    }

    while let Some(child) = container.first_child() {
        container.remove_child(&child)?;
    }
    container.append_child(&fragment)?;

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
