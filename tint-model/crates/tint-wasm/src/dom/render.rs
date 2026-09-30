fn build_node(
    document: &Document,
    node: &UiRenderNode,
    shared: &Rc<Shared>,
    breakpoint: &str,
) -> Result<MNode, JsValue> {
    // Mirrors UiPreviewNode.svelte's own tag choice: a real <button> for
    // Button/MenuItem or anything with a click handler (so it gets free
    // keyboard/focus/AT behavior), a plain <div> otherwise.
    let tag_name = dom_tag_name(node);
    let is_button = tag_name == "button";
    let el = document.create_element(tag_name)?;
    el.set_attribute("data-tag", &node.tag)?;
    el.set_attribute("data-tint-source", &node.tint_source)?;
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

    apply_style_props(&el, &base_props)?;

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

    if let Some(handler) = node.on_click.clone() {
        // Use the browser's normal click activation here. The render tree
        // currently normalizes `click||` and `pointer_down||` into one
        // handler field, and a click listener is the reliable common
        // denominator across browser hosts.
        bind_dispatch(&el, "click", handler, shared, node.sound.clone());
    }
    if let Some(handler) = node.on_hover_enter.clone() {
        bind_dispatch(&el, "mouseenter", handler, shared, None);
    }
    if let Some(handler) = node.on_hover_leave.clone() {
        bind_dispatch(&el, "mouseleave", handler, shared, None);
    }
    if let Some(handler) = node.on_pointer_start.clone() {
        bind_pointer_start(&el, handler, shared);
    }
    if node.tag == "TextArea" {
        bind_text_area(&el, node.on_input.clone(), node.on_submit.clone(), shared);
    }
    if let Some(handler) = node.on_key_down.clone() {
        bind_key_dispatch("keydown", handler, shared);
    }
    if let Some(handler) = node.on_key_up.clone() {
        bind_key_dispatch("keyup", handler, shared);
    }

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
        if let Err(e) = mount_tree(tree, &shared) {
            web_sys::console::error_1(&e);
        }
    }) as Box<dyn FnMut(_)>);
    // Ignore add_event_listener's own Result: a failure here means the
    // element itself is broken, which document.create_element's Result
    // earlier in build_node would already have surfaced.
    let _ = el.add_event_listener_with_callback(event_name, cb.as_ref().unchecked_ref());
    cb.forget();
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
