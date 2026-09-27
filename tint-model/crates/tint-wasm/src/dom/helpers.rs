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
    if let Ok(mut session) = shared.session.try_borrow_mut() {
        session.set_viewport_width(width);
    }
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
            let tree = match shared_for_timeout.session.try_borrow_mut() {
                Ok(mut session) => match session.render() {
                    Ok(tree) => tree,
                    Err(e) => {
                        web_sys::console::error_1(&JsValue::from_str(&format!(
                            "tint: resize re-render failed: {}",
                            e
                        )));
                        return;
                    }
                },
                Err(_) => return,
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
    patch_children(&document, &container, tree, shared, &breakpoint)?;

    // `frame||` (see docs/guide/events.md) drives its own clock instead of
    // waiting for a click/hover/key event: refresh which handler (if any)
    // the tree just asked for, and make sure the rAF loop that calls it is
    // running. Checked on every mount, not just the first, so a handler
    // that appears/changes/disappears on a later render is picked up
    // without the host needing to do anything -- this is what replaces
    // pong.js's/pacman.js's hand-rolled `window.setInterval(..., tick)`.
    *shared.frame_handler.borrow_mut() = find_frame_handler(tree);
    // Bound to a local first (not `if shared.frame_handler.borrow().is_some() { .. }`)
    // so the `Ref` guard drops here, before `start_frame_loop` runs --
    // otherwise it stays alive for the whole `if` block under Rust's
    // temporary-lifetime rules, and the first frame tick's re-entrant
    // `mount_tree` -> `borrow_mut()` on this same RefCell panics with
    // "already borrowed".
    let has_frame_handler = shared.frame_handler.borrow().is_some();
    if has_frame_handler {
        start_frame_loop(shared);
    }

    Ok(())
}

/// Depth-first search for the first node in `nodes` (or its descendants)
/// that declares `frame||`. A `ui fn` is expected to put it on one root-ish
/// node (see docs/guide/events.md's `GameRoot` example), so "first found"
/// is enough -- this isn't trying to support multiple independent frame
/// loops in one tree.
fn find_frame_handler(nodes: &[UiRenderNode]) -> Option<String> {
    for node in nodes {
        if node.on_frame.is_some() {
            return node.on_frame.clone();
        }
        if let Some(handler) = find_frame_handler(&node.children) {
            return Some(handler);
        }
    }
    None
}

/// Starts this session's `requestAnimationFrame` loop the first time any
/// mounted tree declares `frame||` (see `mount_tree`'s call site), and
/// lets it keep rescheduling itself for the life of the page after that --
/// same "leak on purpose" convention `bind_resize_listener` documents in
/// this module's header comment: fine for a session that lives as long as
/// the page.
///
/// Reads `shared.frame_handler` fresh on every tick instead of closing
/// over a handler name, so it keeps dispatching whatever the CURRENT tree
/// asks for even as renders come and go (a handler name changing, or the
/// node carrying it disappearing entirely, from one render to the next).
fn start_frame_loop(shared: &Rc<Shared>) {
    if shared.raf_active.replace(true) {
        return; // already running
    }
    let Some(window) = web_sys::window() else {
        shared.raf_active.set(false);
        return;
    };

    // The standard wasm-bindgen self-rescheduling rAF pattern: the closure
    // needs to pass `request_animation_frame` a reference to ITSELF, which
    // is only possible through this `Rc<RefCell<Option<..>>>` indirection
    // -- a closure can't otherwise capture a `Closure` value that doesn't
    // exist yet (namely, itself).
    let tick_slot: Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>> = Rc::new(RefCell::new(None));
    let tick_slot_for_scheduling = tick_slot.clone();
    let shared_for_loop = shared.clone();

    *tick_slot_for_scheduling.borrow_mut() = Some(Closure::wrap(Box::new(move |timestamp: f64| {
        // Clamped to 250ms: a backgrounded/throttled tab can leave a huge
        // gap between rAF callbacks, and handing that straight to a
        // fixed-step accumulator (see pong.tn's `on_frame`) would make it
        // run hundreds of catch-up steps in one go on refocus. Capping `dt`
        // means state falls behind wall-clock time instead -- the standard
        // trade-off for a fixed-step loop riding a variable-rate clock.
        let dt = match shared_for_loop.last_frame_time.get() {
            Some(previous) => ((timestamp - previous) / 1000.0).clamp(0.0, 0.25),
            None => 0.0,
        };
        shared_for_loop.last_frame_time.set(Some(timestamp));

        // Same reasoning as `mount_tree`'s `has_frame_handler` local above:
        // bind the clone to a variable BEFORE the `if let`, so the `Ref`
        // guard from `.borrow()` drops immediately instead of staying
        // alive for the whole block (Rust extends an `if let` scrutinee's
        // temporaries through its body) -- `mount_tree` below does its own
        // `frame_handler.borrow_mut()`, which would otherwise panic here.
        let frame_handler = shared_for_loop.frame_handler.borrow().clone();
        if let Some(handler) = frame_handler {
            sync_viewport_width(&shared_for_loop);
            // Bound to a local first, same reasoning as `frame_handler`
            // above: a `match`'s scrutinee temporaries (here, the `RefMut`
            // from `.borrow_mut()`) stay alive for the whole match, and
            // `mount_tree` below does its own `session`-independent work
            // but would be one more thing sharing this session's call
            // stack while that guard is theoretically still live. A plain
            // `let` drops it right after this statement, before `mount_tree`
            // runs, instead of leaving that to reasoning about what a
            // match arm happens not to touch.
            let dispatch_result = match shared_for_loop.session.try_borrow_mut() {
                Ok(mut session) => Some(
                    session
                        .dispatch_with_args(&handler, &[tint_evaluator::Value::Number(dt)]),
                ),
                Err(_) => None,
            };
            match dispatch_result {
                None => {}
                Some(Ok(tree)) => {
                    if let Err(e) = mount_tree(&tree, &shared_for_loop) {
                        web_sys::console::error_1(&e);
                    }
                }
                Some(Err(e)) => web_sys::console::error_1(&JsValue::from_str(&format!(
                    "tint: frame dispatch({}) failed: {}",
                    handler, e
                ))),
            }
        }

        // Reschedule unconditionally -- cheap (one rAF callback that reads
        // an Option and does nothing else when it's empty) versus tearing
        // the loop down and having to notice a frame handler reappearing
        // on some later render.
        if let Some(window) = web_sys::window() {
            if let Some(cb) = tick_slot.borrow().as_ref() {
                let _ = window.request_animation_frame(cb.as_ref().unchecked_ref());
            }
        }
    }) as Box<dyn FnMut(f64)>));

    let _ = window.request_animation_frame(
        tick_slot_for_scheduling
            .borrow()
            .as_ref()
            .unwrap()
            .as_ref()
            .unchecked_ref(),
    );
}

/// Keyed DOM patching. Keyed children are moved and updated in place instead
/// of being destroyed, preserving focus, media playback and CSS animation
/// state. Unkeyed children keep the simple positional behavior.
fn patch_children(
    document: &Document,
    parent: &Element,
    nodes: &[UiRenderNode],
    shared: &Rc<Shared>,
    breakpoint: &str,
) -> Result<(), JsValue> {
    let parent_node: web_sys::Node = parent.clone().into();
    let old: Vec<Element> = (0..parent_node.child_nodes().length())
        .filter_map(|i| parent_node.child_nodes().item(i))
        .filter_map(|node| node.dyn_into::<Element>().ok())
        .collect();
    let mut used = Vec::new();

    for (index, node) in nodes.iter().enumerate() {
        let mut candidate = None;
        if let Some(key) = &node.key {
            for (i, child) in old.iter().enumerate() {
                if child.get_attribute("data-tint-key").as_deref() == Some(key)
                    && !used.contains(&i)
                {
                    candidate = Some((i, child.clone()));
                    break;
                }
            }
        } else if let Some(child) = old.get(index) {
            candidate = Some((index, child.clone()));
        }

        // Only a same-tag candidate is safe to hand to `patch_node`.
        // `patch_node` itself falls back to `build_node` on a tag
        // mismatch, but if it were still marked `used` below, the cleanup
        // pass at the end of this function would never remove it -- the
        // stale old element (e.g. a Slot's "PlayerTop" after the paddle
        // moves down into "PlayerMid") would stay in the DOM forever,
        // orphaned alongside the freshly built replacement. Filtering it
        // out here instead makes a tag mismatch behave like any other
        // no-longer-present old child: cleaned up below like normal.
        let candidate = candidate.filter(|(_, child)| {
            child.get_attribute("data-tag").as_deref() == Some(node.tag.as_str())
        });

        let element = if let Some((old_index, child)) = candidate {
            used.push(old_index);
            patch_node(document, &child, node, shared, breakpoint)?
        } else {
            build_node(document, node, shared, breakpoint)?
        };
        // append_child moves an existing node, giving keyed reorder semantics --
        // but calling it when `element` is ALREADY this parent's child at
        // `index` still removes-and-reinserts it per spec, just to land back
        // in the same place. That happens synchronously inside every
        // dispatch (including the `pointerdown` handler `bind_dispatch`
        // binds for `click||`), and a synchronous detach/reattach of the
        // event target cancels the browser's own pending `click` on it --
        // which is what native `<a href>` navigation (`route::`) fires on.
        // A Sandbox/GitHub link with a `click||` handler stopped navigating
        // for exactly this reason: pressing it dispatched, dispatch
        // rebuilt/re-appended the whole tree including the link itself, and
        // that reinsertion swallowed the click that would have followed.
        let element_node: &web_sys::Node = element.as_ref();
        let already_placed = parent_node
            .child_nodes()
            .item(index as u32)
            .map(|existing| existing.is_same_node(Some(element_node)))
            .unwrap_or(false);
        if !already_placed {
            parent.append_child(&element)?;
        }
    }

    for i in (0..old.len()).rev() {
        if !used.contains(&i) {
            parent.remove_child(&old[i])?;
        }
    }
    Ok(())
}

fn patch_node(
    document: &Document,
    element: &Element,
    node: &UiRenderNode,
    shared: &Rc<Shared>,
    breakpoint: &str,
) -> Result<Element, JsValue> {
    if element.get_attribute("data-tag").as_deref() != Some(node.tag.as_str()) {
        return build_node(document, node, shared, breakpoint);
    }
    // Every write below is skipped when the value hasn't actually changed.
    // `set_attribute`/`set_text_content` record a DOM mutation -- and flash
    // the element in DevTools -- even when writing back the same value it
    // already holds, and `patch_node` runs on every render (including the
    // pong demo's 120ms game tick) for every node in a mostly-static tree,
    // so without these guards nearly everything appears to flicker for no
    // reason.
    if element.get_attribute("data-tint-source").as_deref() != Some(node.tint_source.as_str()) {
        element.set_attribute("data-tint-source", &node.tint_source)?;
    }
    if let Some(key) = &node.key {
        if element.get_attribute("data-tint-key").as_deref() != Some(key.as_str()) {
            element.set_attribute("data-tint-key", key)?;
        }
    }
    if let Some(asset) = &node.asset {
        if element.get_attribute("src").as_deref() != Some(asset.as_str()) {
            element.set_attribute("src", asset)?;
        }
    }
    if let Some(svg) = &node.svg {
        element.set_inner_html(svg);
    } else if let Some(text) = &node.text {
        if element.text_content().as_deref() != Some(text.as_str()) {
            element.set_text_content(Some(text));
        }
    } else {
        patch_children(document, element, &node.children, shared, breakpoint)?;
    }
    let is_button = element.tag_name().eq_ignore_ascii_case("button");
    let mut props = Vec::new();
    if is_button { props.extend(BUTTON_RESET.iter().map(|(k, v)| (k.to_string(), v.to_string()))); }
    props.extend(node.style.iter().cloned());
    if let Some((_, styles)) = node.breakpoints.iter().find(|(name, _)| name == breakpoint) {
        props.extend(styles.iter().cloned());
    }
    // Keep the hover closures bound in `build_node` current: they read
    // these two attributes at event time instead of a string captured once
    // when the element was first built, which otherwise went stale the
    // moment a patch (a theme switch, e.g.) resolved a different style for
    // this same reused element -- see the comment in `build_node`.
    if !node.hover_style.is_empty() {
        let base_css = style_to_css_text(&props);
        let mut hover_props = props.clone();
        hover_props.extend(node.hover_style.iter().cloned());
        let hover_css = style_to_css_text(&hover_props);
        if element.get_attribute("data-tint-base-css").as_deref() != Some(base_css.as_str()) {
            let _ = element.set_attribute("data-tint-base-css", &base_css);
        }
        if element.get_attribute("data-tint-hover-css").as_deref() != Some(hover_css.as_str()) {
            let _ = element.set_attribute("data-tint-hover-css", &hover_css);
        }
    }
    // Reconcile hover from the browser's actual pointer state on every patch.
    // The attribute is only a fast path between events; trusting it forever
    // leaves a stale hover after a missed mouseleave (and causes flicker when
    // a game tick patches the node while the pointer is over it).
    let is_hovered = !node.hover_style.is_empty()
        && element
            .matches(":hover")
            .unwrap_or_else(|_| {
                element.get_attribute("data-tint-hover").as_deref() == Some("true")
            });
    if is_hovered {
        if element.get_attribute("data-tint-hover").as_deref() != Some("true") {
            let _ = element.set_attribute("data-tint-hover", "true");
        }
    } else if element.has_attribute("data-tint-hover") {
        let _ = element.remove_attribute("data-tint-hover");
    }
    if is_hovered {
        props.extend(node.hover_style.iter().cloned());
    }
    apply_style_props(element, &props)?;
    Ok(element.clone())
}

fn apply_style_props(el: &Element, props: &[(String, String)]) -> Result<(), JsValue> {
    // Replace the complete inline style in one DOM operation. Besides
    // avoiding base->hover transitions during a patch, this removes hover
    // properties (for example box-shadow) that do not exist in the base
    // style when a stale mouseleave was missed.
    let css = style_to_css_text(props);
    // Skipped when nothing changed -- see patch_node's comment on the same
    // pattern for data-tint-source/data-tint-key/src above; `style` is the
    // highest-traffic attribute here since every render rewrites it.
    if el.get_attribute("style").as_deref() != Some(css.as_str()) {
        el.set_attribute("style", &css)?;
    }
    Ok(())
}

fn merge_style_props(props: &[(String, String)]) -> Vec<(String, String)> {
    let mut merged = Vec::with_capacity(props.len());
    for (key, value) in props {
        if let Some((_, current)) = merged.iter_mut().find(|(existing, _)| existing == key) {
            *current = value.clone();
        } else {
            merged.push((key.clone(), value.clone()));
        }
    }
    merged
}

/// `node.style` as a single CSS text string, for resetting an element's
/// whole `style` attribute on `mouseleave`. Needed because `mouseenter`
/// only ever ADDS properties on top via `set_property` (see
/// `build_node`) -- leaving has to wipe those additions, not just
/// re-apply the base list on top of an already-hover-dirty style.
fn style_to_css_text(props: &[(String, String)]) -> String {
    merge_style_props(props)
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
