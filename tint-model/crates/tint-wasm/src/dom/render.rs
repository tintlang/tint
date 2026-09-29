fn build_node(
    document: &Document,
    node: &UiRenderNode,
    shared: &Rc<Shared>,
    breakpoint: &str,
) -> Result<Element, JsValue> {
    // Mirrors UiPreviewNode.svelte's own tag choice: a real <button> for
    // Button/MenuItem or anything with a click handler (so it gets free
    // keyboard/focus/AT behavior), a plain <div> otherwise.
    let is_link = node.route.is_some();
    let is_inline = node.tag == "Inline";
    let is_text = node.tag == "Text";
    let is_image = node.asset.is_some() && node.tag == "Image";
    let is_audio = node.asset.is_some() && node.tag == "Audio";
    let is_button = !is_link
        && (node.tag == "Button"
            || node.tag == "MenuItem"
            || node.on_click.is_some()
            || node.on_js.is_some());
    let tag_name = if is_link {
        "a"
    } else if is_inline {
        "span"
    } else if is_text {
        "span"
    } else if is_image {
        "img"
    } else if is_audio {
        "audio"
    } else if is_button {
        "button"
    } else {
        "div"
    };
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
    if let Some((_, bp_style)) = node.breakpoints.iter().find(|(name, _)| name == breakpoint) {
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
    if let Some(handler) = node.on_key_down.clone() {
        bind_key_dispatch("keydown", handler, shared);
    }
    if let Some(handler) = node.on_key_up.clone() {
        bind_key_dispatch("keyup", handler, shared);
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

fn bind_key_dispatch(event_name: &'static str, handler: String, shared: &Rc<Shared>) {
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
        if let Err(e) = mount_tree(&tree, &shared) {
            web_sys::console::error_1(&e);
        }
    }) as Box<dyn FnMut(_)>);
    let _ = window.add_event_listener_with_callback(event_name, cb.as_ref().unchecked_ref());
    cb.forget();
}
