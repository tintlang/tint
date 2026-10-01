fn build_node(
    document: &Document,
    node: &UiRenderNode,
    shared: &Rc<Shared>,
    breakpoint: &str,
) -> Result<MNode, JsValue> {
    // Mirrors UiPreviewNode.svelte's own tag choice: a real <button> for
    // Button/MenuItem or anything with a click handler (so it gets free
    // keyboard/focus/AT behavior), a plain <div> otherwise.
    if let Some(mounted) = build_bulk(document, node, shared, breakpoint)? {
        return Ok(mounted);
    }
    let tag_name = dom_tag_name(node);
    let is_button = tag_name == "button";
    let el = document.create_element(tag_name)?;
    el.set_attribute("data-tag", &node.tag)?;
    if !node.tint_source.is_empty() {
        el.set_attribute("data-tint-source", &node.tint_source)?;
    }
    if let Some(key) = &node.key {
        el.set_attribute("data-tint-key", key)?;
    }
    if let Some(route) = &node.route {
        el.set_attribute("href", route)?;
        if let Some(target) = &node.target {
            el.set_attribute("target", target)?;
            if target == "_blank" {
                el.set_attribute("rel", "noopener noreferrer")?;
            }
        }
    }
    if let Some(asset) = &node.asset {
        el.set_attribute("src", asset)?;
    }
    if let Some(reference) = &node.reference {
        el.set_attribute("data-tint-ref", reference)?;
    }
    if let Some(class) = &node.class {
        el.set_attribute("class", class)?;
    }
    apply_html_attrs(&el, &[], &node.attrs);
    apply_state_styles(&el, node);
    apply_gestures(&el, node);
    if let Some(component) = &node.component {
        el.set_attribute("data-tint-component", component)?;
    }
    if let Some(props) = &node.props {
        el.set_attribute("data-tint-props", props)?;
    }
    if let Some(handler) = &node.on_js {
        el.set_attribute("data-tint-js", handler)?;
    }

    if node.tag == "Preview" {
        set_preview_attributes(&el, node)?;
    } else if let Some(svg) = &node.svg {
        el.set_inner_html(svg);
    } else if let Some(text) = &node.text {
        if let Some(area) = el.dyn_ref::<web_sys::HtmlTextAreaElement>() {
            area.set_value(text);
            let _ = el.set_attribute("spellcheck", "false");
            let _ = el.set_attribute("autocapitalize", "off");
            let _ = el.set_attribute("autocomplete", "off");
            let _ = el.set_attribute("wrap", "off");
        } else {
            el.set_text_content(Some(text));
        }
    }

    // Base = UA reset (buttons only) + whatever the language actually
    // resolved. Kept as one combined list (not two separate applies)
    // because mouseleave below needs to restore exactly this, including
    // the reset -- resetting the whole `style` attribute to just
    // `node.style` would bring the browser's button chrome right back.
    let mut base_props: Vec<(String, String)> = Vec::new();
    if is_button {
        base_props.extend(
            BUTTON_RESET
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string())),
        );
    }
    base_props.extend(node.style.iter().cloned());

    // Layer the current viewport's breakpoint style (if the node has
    // one) on top of the base style -- last-write-wins per property,
    // same as a CSS media query overriding a base rule.
    for bp_style in matching_breakpoints(node, breakpoint) {
        base_props.extend(bp_style.iter().cloned());
    }

    // A fresh element has no style yet: write it without reading it back first.
    let css = style_to_css_text(&base_props);
    if !css.is_empty() {
        el.set_attribute("style", &css)?;
    }

    if !node.hover_style.is_empty() {
        let base_css = style_to_css_text(&base_props);
        let mut hover_props = base_props.clone();
        hover_props.extend(node.hover_style.iter().cloned());
        let hover_css = style_to_css_text(&hover_props);

        // Stashed on the element as attributes (not just captured by the
        // closures below) because `patch_node` reuses this same element on
        // later renders (a keyed diff, not a rebuild) and can resolve a
        // different base/hover style for it -- a theme switch is the
        // common case. `patch_node` refreshes these same two attributes on
        // every patch, so mouseenter/mouseleave always read the current
        // style instead of whatever was active when this element was
        // first built. Before this, a theme switch left the closures
        // applying the old theme's hover color, racing against the
        // periodic tick reconciliation below (which used the current
        // theme) -- that race was the flicker.
        let _ = el.set_attribute("data-tint-base-css", &base_css);
        let _ = el.set_attribute("data-tint-hover-css", &hover_css);

        let el_enter = el.clone();
        let enter_cb = Closure::wrap(Box::new(move |_e: web_sys::Event| {
            let _ = el_enter.set_attribute("data-tint-hover", "true");
            // Apply the hover layer in one pass. This avoids briefly writing
            // a base value and then a hover value when a render tick lands
            // on the same frame as mouseenter. Read fresh off the element
            // rather than a captured string -- see the comment above.
            if let Some(hover_css) = el_enter.get_attribute("data-tint-hover-css") {
                let _ = el_enter.set_attribute("style", &hover_css);
            }
        }) as Box<dyn FnMut(_)>);
        el.add_event_listener_with_callback("mouseenter", enter_cb.as_ref().unchecked_ref())?;
        enter_cb.forget();

        let el_leave = el.clone();
        let leave_cb = Closure::wrap(Box::new(move |_e: web_sys::Event| {
            let _ = el_leave.remove_attribute("data-tint-hover");
            if let Some(base_css) = el_leave.get_attribute("data-tint-base-css") {
                let _ = el_leave.set_attribute("style", &base_css);
            }
        }) as Box<dyn FnMut(_)>);
        el.add_event_listener_with_callback("mouseleave", leave_cb.as_ref().unchecked_ref())?;
        leave_cb.forget();
    }

    bind_handlers(&el, node, shared);

    let mut children = Vec::new();
    if node.text.is_none() && node.svg.is_none() {
        for child in &node.children {
            let child_mount = build_node(document, child, shared, breakpoint)?;
            el.append_child(&child_mount.el)?;
            children.push(child_mount);
        }
    }

    let has_hover = !node.hover_style.is_empty() || children.iter().any(|c| c.has_hover);
    Ok(MNode {
        el,
        children,
        has_hover,
    })
}

fn bind_handlers(el: &Element, node: &UiRenderNode, shared: &Rc<Shared>) {
    if let Some(handler) = node.on_click.clone() {
        // Use the browser's normal click activation here. The render tree
        // currently normalizes `click||` and `pointer_down||` into one
        // handler field, and a click listener is the reliable common
        // denominator across browser hosts.
        bind_dispatch(el, "click", handler, shared, node.sound.clone());
    }
    if let Some(handler) = node.on_hover_enter.clone() {
        bind_dispatch(el, "mouseenter", handler, shared, None);
    }
    if let Some(handler) = node.on_hover_leave.clone() {
        bind_dispatch(el, "mouseleave", handler, shared, None);
    }
    if let Some(handler) = node.on_pointer_start.clone() {
        bind_pointer_start(el, handler, shared);
    }
    if node.tag == "TextArea" {
        bind_text_area(el, node.on_input.clone(), node.on_submit.clone(), shared);
    }
    if let Some(handler) = node.on_key_down.clone() {
        bind_key_dispatch("keydown", handler, shared);
    }
    if let Some(handler) = node.on_key_up.clone() {
        bind_key_dispatch("keyup", handler, shared);
    }

}

// ---- bulk building ------------------------------------------------------
//
// Creating a node element by element costs a handful of JS calls each. A plain
// subtree (divs, buttons and spans with text, styles and click handlers only)
// is instead written out as one HTML string, parsed by the browser in a single
// call, and its elements are then picked up by walking the result in step with
// the render tree.

thread_local! {
    static TEMPLATE: RefCell<Option<web_sys::HtmlTemplateElement>> = const { RefCell::new(None) };
    static HOLDER: RefCell<Option<Element>> = const { RefCell::new(None) };
}

/// Nodes in the subtree when it can be built from HTML, else `None`.
fn bulk_size(node: &UiRenderNode, in_button: bool) -> Option<usize> {
    if node.hover_style.len() > 0
        || node.route.is_some()
        || node.asset.is_some()
        || node.reference.is_some()
        || node.class.is_some()
        || !node.attrs.is_empty()
        || node.style.iter().any(|(k, _)| k == "--tint-drag")
        || node.breakpoints.iter().any(|(name, _)| name.starts_with(':'))
        || node.component.is_some()
        || node.on_js.is_some()
        || node.svg.is_some()
        || node.tag == "Preview"
        || node.tag == "TextArea"
        || node.on_pointer_start.is_some()
    {
        return None;
    }
    let tag = dom_tag_name(node);
    if !matches!(tag, "div" | "button" | "span") || (tag == "button" && in_button) {
        return None;
    }
    if node.text.is_some() && !node.children.is_empty() {
        return None;
    }
    let mut size = 1;
    for child in &node.children {
        size += bulk_size(child, in_button || tag == "button")?;
    }
    Some(size)
}

fn push_escaped(out: &mut String, text: &str, quote: bool) {
    if !text.bytes().any(|b| matches!(b, b'&' | b'<' | b'>' | b'"')) {
        out.push_str(text);
        return;
    }
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' if quote => out.push_str("&quot;"),
            c => out.push(c),
        }
    }
}

thread_local! {
    /// CSS text per shared style list. The list itself is kept in the entry so
    /// its address cannot be reused by a different list while cached.
    static CSS_MEMO: RefCell<std::collections::HashMap<(usize, bool), (Rc<Vec<(String, String)>>, Rc<str>)>> =
        RefCell::new(std::collections::HashMap::new());
}

fn node_css(node: &UiRenderNode, is_button: bool, breakpoint: &str) -> Rc<str> {
    let compute = || -> Rc<str> {
        let mut props: Vec<(String, String)> = Vec::new();
        if is_button {
            props.extend(BUTTON_RESET.iter().map(|(k, v)| (k.to_string(), v.to_string())));
        }
        props.extend(node.style.iter().cloned());
        for bp_style in matching_breakpoints(node, breakpoint) {
            props.extend(bp_style.iter().cloned());
        }
        Rc::from(style_to_css_text(&props))
    };
    if !node.breakpoints.is_empty() || node.style.is_empty() {
        return compute();
    }
    let key = (Rc::as_ptr(&node.style) as usize, is_button);
    CSS_MEMO.with(|memo| {
        let mut memo = memo.borrow_mut();
        if let Some((_, css)) = memo.get(&key) {
            return Rc::clone(css);
        }
        if memo.len() > 4096 {
            memo.clear();
        }
        let css = compute();
        memo.insert(key, (Rc::clone(&node.style), Rc::clone(&css)));
        css
    })
}

fn write_html(node: &UiRenderNode, breakpoint: &str, out: &mut String) {
    let tag = dom_tag_name(node);
    out.push('<');
    out.push_str(tag);
    out.push_str(" data-tag=\"");
    push_escaped(out, &node.tag, true);
    out.push('"');
    if !node.tint_source.is_empty() {
        out.push_str(" data-tint-source=\"");
        push_escaped(out, &node.tint_source, true);
        out.push('"');
    }
    if let Some(key) = &node.key {
        out.push_str(" data-tint-key=\"");
        push_escaped(out, key, true);
        out.push('"');
    }
    let css = node_css(node, tag == "button", breakpoint);
    if !css.is_empty() {
        out.push_str(" style=\"");
        push_escaped(out, &css, true);
        out.push('"');
    }
    out.push('>');
    if let Some(text) = &node.text {
        push_escaped(out, text, false);
    }
    for child in &node.children {
        write_html(child, breakpoint, out);
    }
    out.push_str("</");
    out.push_str(tag);
    out.push('>');
}

/// Pairs a parsed element with its render node: binds handlers and collects
/// the mounted children in document order.
fn adopt(el: Element, node: &UiRenderNode, shared: &Rc<Shared>) -> Result<MNode, JsValue> {
    bind_handlers(&el, node, shared);
    let mut children = Vec::with_capacity(node.children.len());
    if node.text.is_none() {
        let mut next = el.first_element_child();
        for child in &node.children {
            let child_el = next.ok_or_else(|| JsValue::from_str("bulk build: missing element"))?;
            next = child_el.next_element_sibling();
            children.push(adopt(child_el, child, shared)?);
        }
    }
    Ok(MNode { el, children, has_hover: false })
}

/// Builds every bulk-able node among `nodes` with a single HTML parse; the
/// result has one entry per input (`None` where the node needs `build_node`).
/// The built elements sit in the returned fragment until placed.
fn build_bulk_many(
    document: &Document,
    nodes: &[&Rc<UiRenderNode>],
    shared: &Rc<Shared>,
    breakpoint: &str,
) -> Result<(Vec<Option<MNode>>, Option<web_sys::DocumentFragment>), JsValue> {
    let eligible: Vec<bool> = nodes
        .iter()
        .map(|n| !n.children.is_empty() && bulk_size(n, false).is_some())
        .collect();
    if eligible.iter().filter(|e| **e).count() < 2 {
        return Ok((nodes.iter().map(|_| None).collect(), None));
    }
    let mut html = String::new();
    for (node, ok) in nodes.iter().zip(&eligible) {
        if *ok {
            write_html(node, breakpoint, &mut html);
        }
    }
    let template = TEMPLATE.with(|t| -> Result<web_sys::HtmlTemplateElement, JsValue> {
        let mut t = t.borrow_mut();
        if t.is_none() {
            *t = Some(document.create_element("template")?.dyn_into::<web_sys::HtmlTemplateElement>()?);
        }
        Ok(t.as_ref().expect("just set").clone())
    })?;
    template.set_inner_html(&html);
    let fragment = template.content();
    let mut next = fragment.first_element_child();
    let mut out = Vec::with_capacity(nodes.len());
    for (node, ok) in nodes.iter().zip(&eligible) {
        if !*ok {
            out.push(None);
            continue;
        }
        let root = next.ok_or_else(|| JsValue::from_str("bulk build: missing element"))?;
        next = root.next_element_sibling();
        out.push(Some(adopt(root, node, shared)?));
    }
    let all = eligible.iter().all(|e| *e);
    Ok((out, all.then_some(fragment)))
}

/// When every new node is plain and none reuses an old element, the parent's
/// whole content is replaced by one parse straight into it: no per-element
/// removal, no per-element insertion.
fn build_bulk_into(
    parent: &Element,
    nodes: &[Rc<UiRenderNode>],
    shared: &Rc<Shared>,
    breakpoint: &str,
) -> Result<Option<Vec<MNode>>, JsValue> {
    if nodes.len() < 2 || nodes.iter().any(|n| bulk_size(n, false).is_none()) {
        return Ok(None);
    }
    let mut html = String::new();
    for node in nodes {
        write_html(node, breakpoint, &mut html);
    }
    parent.set_inner_html(&html);
    let mut next = parent.first_element_child();
    let mut out = Vec::with_capacity(nodes.len());
    for node in nodes {
        let el = next.ok_or_else(|| JsValue::from_str("bulk build: missing element"))?;
        next = el.next_element_sibling();
        out.push(adopt(el, node, shared)?);
    }
    Ok(Some(out))
}

fn holder(document: &Document) -> Result<Element, JsValue> {
    HOLDER.with(|h| -> Result<Element, JsValue> {
        let mut h = h.borrow_mut();
        if h.is_none() {
            *h = Some(document.create_element("div")?);
        }
        Ok(h.as_ref().expect("just set").clone())
    })
}

fn build_bulk(
    document: &Document,
    node: &UiRenderNode,
    shared: &Rc<Shared>,
    breakpoint: &str,
) -> Result<Option<MNode>, JsValue> {
    if node.children.is_empty() || bulk_size(node, false).is_none() {
        return Ok(None);
    }
    let mut html = String::new();
    write_html(node, breakpoint, &mut html);
    let holder = holder(document)?;
    holder.set_inner_html(&html);
    let root = holder
        .first_element_child()
        .ok_or_else(|| JsValue::from_str("bulk build: no element"))?;
    Ok(Some(adopt(root, node, shared)?))
}

/// Wires `event_name` on `el` to run `handler` against the session and
/// rebuild the DOM -- the same dispatch-then-rerender loop
/// `DomSession::dispatch` exposes to JS, just triggered by a real DOM
/// event. Bound as its own `addEventListener` call, alongside (not
/// instead of) any style-only hover listener already bound above for
/// the same event name.
fn bind_dispatch(
    el: &Element,
    event_name: &'static str,
    handler: String,
    shared: &Rc<Shared>,
    sound: Option<String>,
) {
    let shared = shared.clone();
    let cb = Closure::wrap(Box::new(move |_e: web_sys::Event| {
        if let Some(sound) = &sound {
            play_sound(sound);
        }
        let timing = profiling();
        let t0 = if timing { now() } else { 0.0 };
        let tree = match shared.session.try_borrow_mut() {
            Ok(mut session) => match session.dispatch(&handler) {
                Ok(tree) => tree,
                Err(e) => {
                    web_sys::console::error_1(&JsValue::from_str(&format!(
                        "tint: dispatch({}) failed: {}",
                        handler, e
                    )));
                    return;
                }
            },
            Err(_) => return,
        };
        let t1 = if timing { now() } else { 0.0 };
        if let Err(e) = mount_tree(tree, &shared) {
            web_sys::console::error_1(&e);
        }
        if timing {
            let t2 = now();
            web_sys::console::log_1(&JsValue::from_str(&format!(
                "TINT-TIMING {handler} eval {:.1} mount {:.1}",
                t1 - t0,
                t2 - t1
            )));
        }
    }) as Box<dyn FnMut(_)>);
    // Ignore add_event_listener's own Result: a failure here means the
    // element itself is broken, which document.create_element's Result
    // earlier in build_node would already have surfaced.
    let _ = el.add_event_listener_with_callback(event_name, cb.as_ref().unchecked_ref());
    cb.forget();
}

/// `<html data-tint-timing>` logs, per click, how long the program took to run
/// (`eval`) and how long patching the DOM took (`mount`).
fn profiling() -> bool {
    web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.document_element())
        .is_some_and(|e| e.has_attribute("data-tint-timing"))
}

fn now() -> f64 {
    web_sys::window().and_then(|w| w.performance()).map_or(0.0, |p| p.now())
}

fn play_sound(src: &str) {
    let Some(window) = web_sys::window() else { return };
    let Some(document) = window.document() else { return };
    let Ok(audio) = web_sys::HtmlAudioElement::new_with_src(src) else { return };
    let _ = audio.play();
    // Keep the element alive until playback ends without putting it into the
    // visible UI tree. The browser owns the detached media element while its
    // play promise is active; future audio pooling belongs to the resource
    // layer rather than the event bridge.
    let _ = document;
}

/// Runs `handler(text)` against the session and re-mounts the result.
fn dispatch_text(shared: &Rc<Shared>, handler: &str, text: String) {
    let args = [tint_evaluator::Value::String(text)];
    let tree = match shared.session.try_borrow_mut() {
        Ok(mut session) => match session.dispatch_with_args(handler, &args) {
            Ok(tree) => tree,
            Err(e) => {
                web_sys::console::error_1(&JsValue::from_str(&format!("tint: input handler {} failed: {}", handler, e)));
                return;
            }
        },
        Err(_) => return,
    };
    if let Err(e) = mount_tree(tree, shared) {
        web_sys::console::error_1(&e);
    }
}

/// `TextArea { input||on_edit submit||run }`: `input` gets the new text after
/// every edit, `submit` fires on Ctrl/Cmd+Enter, and Tab inserts two spaces
/// instead of leaving the field.
fn bind_text_area(el: &Element, on_input: Option<String>, on_submit: Option<String>, shared: &Rc<Shared>) {
    let Some(area) = el.dyn_ref::<web_sys::HtmlTextAreaElement>().cloned() else { return };
    if let Some(handler) = on_input.clone() {
        let shared = shared.clone();
        let area = area.clone();
        let cb = Closure::wrap(Box::new(move |_e: web_sys::Event| {
            dispatch_text(&shared, &handler, area.value());
        }) as Box<dyn FnMut(_)>);
        let _ = el.add_event_listener_with_callback("input", cb.as_ref().unchecked_ref());
        cb.forget();
    }
    let shared = shared.clone();
    let key_area = area.clone();
    let cb = Closure::wrap(Box::new(move |event: web_sys::KeyboardEvent| {
        let key = event.key();
        if key == "Tab" && !event.shift_key() && !event.ctrl_key() && !event.meta_key() && !event.alt_key() {
            event.prevent_default();
            let start = key_area.selection_start().ok().flatten().unwrap_or(0);
            let end = key_area.selection_end().ok().flatten().unwrap_or(start);
            let _ = key_area.set_range_text_with_start_and_end_and_mode("  ", start, end, "end");
            if let Some(handler) = &on_input {
                dispatch_text(&shared, handler, key_area.value());
            }
        } else if key == "Enter" && (event.ctrl_key() || event.meta_key()) {
            if let Some(handler) = &on_submit {
                event.prevent_default();
                let tree = match shared.session.try_borrow_mut() {
                    Ok(mut session) => session.dispatch(handler),
                    Err(_) => return,
                };
                match tree {
                    Ok(tree) => {
                        if let Err(e) = mount_tree(tree, &shared) {
                            web_sys::console::error_1(&e);
                        }
                    }
                    Err(e) => web_sys::console::error_1(&JsValue::from_str(&format!("tint: submit failed: {}", e))),
                }
            }
        }
    }) as Box<dyn FnMut(_)>);
    let _ = el.add_event_listener_with_callback("keydown", cb.as_ref().unchecked_ref());
    cb.forget();
}

/// Runs `handler(x, y)` against the session and re-mounts the result.
fn dispatch_pointer(shared: &Rc<Shared>, handler: &str, x: f64, y: f64) {
    let args = [tint_evaluator::Value::Number(x), tint_evaluator::Value::Number(y)];
    let tree = match shared.session.try_borrow_mut() {
        Ok(mut session) => match session.dispatch_with_args(handler, &args) {
            Ok(tree) => tree,
            Err(e) => {
                web_sys::console::error_1(&JsValue::from_str(&format!(
                    "tint: pointer handler {} failed: {}",
                    handler, e
                )));
                return;
            }
        },
        Err(_) => return,
    };
    if let Err(e) = mount_tree(tree, shared) {
        web_sys::console::error_1(&e);
    }
}

/// `pointer_start||handler`: a primary-button press on `el` starts a drag.
/// Until the pointer is released, window-level `pointer_move||` and
/// `pointer_up||` handlers (declared anywhere in the tree) receive its
/// position, so a drag survives leaving the element.
fn bind_pointer_start(el: &Element, handler: String, shared: &Rc<Shared>) {
    bind_pointer_window(shared);
    let shared = shared.clone();
    let cb = Closure::wrap(Box::new(move |event: web_sys::PointerEvent| {
        if event.button() != 0 {
            return;
        }
        event.prevent_default();
        shared.dragging.set(true);
        dispatch_pointer(&shared, &handler, event.client_x() as f64, event.client_y() as f64);
    }) as Box<dyn FnMut(_)>);
    let _ = el.add_event_listener_with_callback("pointerdown", cb.as_ref().unchecked_ref());
    cb.forget();
}

fn bind_pointer_window(shared: &Rc<Shared>) {
    if shared.pointer_bound.replace(true) {
        return;
    }
    let Some(window) = web_sys::window() else { return };
    for event_name in ["pointermove", "pointerup", "pointercancel"] {
        let shared = shared.clone();
        let is_move = event_name == "pointermove";
        let cb = Closure::wrap(Box::new(move |event: web_sys::PointerEvent| {
            if !shared.dragging.get() {
                return;
            }
            let handler = if is_move {
                shared.pointer_move_handler.borrow().clone()
            } else {
                shared.dragging.set(false);
                shared.pointer_up_handler.borrow().clone()
            };
            if let Some(handler) = handler {
                dispatch_pointer(&shared, &handler, event.client_x() as f64, event.client_y() as f64);
            }
        }) as Box<dyn FnMut(_)>);
        let _ = window.add_event_listener_with_callback(event_name, cb.as_ref().unchecked_ref());
        cb.forget();
    }
}

fn bind_key_dispatch(event_name: &'static str, handler: String, shared: &Rc<Shared>) {
    if shared.nested {
        return;
    }
    let (handler_slot, already_bound) = match event_name {
        "keydown" => {
            shared.key_down_handler.replace(Some(handler));
            ("keydown", shared.key_down_bound.replace(true))
        }
        "keyup" => {
            shared.key_up_handler.replace(Some(handler));
            ("keyup", shared.key_up_bound.replace(true))
        }
        _ => return,
    };
    if already_bound {
        return;
    }

    let window = match web_sys::window() {
        Some(window) => window,
        None => return,
    };
    let shared = shared.clone();
    let cb = Closure::wrap(Box::new(move |event: web_sys::KeyboardEvent| {
        // A `key_down||`/`key_up||` handler is an explicit, deliberate
        // opt-in by the .tint source (unlike the plain global listener
        // this replaces in pong.js/pacman.js, which guarded this with an
        // `instanceof HTMLInputElement` check before calling it). Prevent
        // the browser's own default for the key here the same way that
        // host-side JS did -- otherwise Up/Down/Space still scroll the
        // page underneath a game that's consuming them as controls.
        event.prevent_default();
        // Read the current handler because mount_tree rebuilds the DOM after
        // every dispatch. One stable window listener is enough for the whole
        // session and avoids multiplying W/S input after every frame.
        let handler = match handler_slot {
            "keydown" => shared.key_down_handler.borrow().clone(),
            "keyup" => shared.key_up_handler.borrow().clone(),
            _ => None,
        };
        let Some(handler) = handler else { return };
        let args = [tint_evaluator::Value::String(event.key())];
        let tree = match shared.session.try_borrow_mut() {
            Ok(mut session) => match session.dispatch_with_args(&handler, &args) {
                Ok(tree) => tree,
                Err(e) => {
                    web_sys::console::error_1(&JsValue::from_str(&format!("tint: {} failed: {}", event_name, e)));
                    return;
                }
            },
            Err(_) => return,
        };
        if let Err(e) = mount_tree(tree, &shared) {
            web_sys::console::error_1(&e);
        }
    }) as Box<dyn FnMut(_)>);
    let _ = window.add_event_listener_with_callback(event_name, cb.as_ref().unchecked_ref());
    cb.forget();
}
