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
    let is_button =
        !is_link && (node.tag == "Button" || node.tag == "MenuItem" || node.on_click.is_some());
    let tag_name = if is_link {
        "a"
    } else if is_button {
        "button"
    } else {
        "div"
    };
    let el = document.create_element(tag_name)?;
    el.set_attribute("data-tag", &node.tag)?;
    el.set_attribute("data-tint-source", &node.tint_source)?;
    if let Some(route) = &node.route {
        el.set_attribute("href", route)?;
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
