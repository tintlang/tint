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
            if let Err(e) = mount_tree(tree, &shared_for_timeout) {
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
fn mount_tree(tree: Vec<Rc<UiRenderNode>>, shared: &Rc<Shared>) -> Result<(), JsValue> {
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
    let previous = shared.retained.borrow_mut().take().filter(|r| {
        r.breakpoint == breakpoint && container.child_element_count() as usize == r.mounts.len()
    });
    let (old_nodes, old_mounts) = match previous {
        Some(r) => (r.nodes, r.mounts),
        None => {
            // First mount, a breakpoint change, or someone else touched the
            // container: start from a clean slate.
            container.set_text_content(None);
            (Vec::new(), Vec::new())
        }
    };
    let mounts = patch_list(&document, &container, &old_nodes, old_mounts, &tree, shared, &breakpoint)?;

    // `frame||` (see docs/guide/events.md) drives its own clock instead of
    // waiting for a click/hover/key event: refresh which handler (if any)
    // the tree just asked for, and make sure the rAF loop that calls it is
    // running. Checked on every mount, not just the first, so a handler
    // that appears/changes/disappears on a later render is picked up
    // without the host needing to do anything -- this is what replaces
    // pong.js's/pacman.js's hand-rolled `window.setInterval(..., tick)`.
    *shared.frame_handler.borrow_mut() = find_frame_handler(&tree);
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

    // `tick||`/`every||` intervals follow the tree the same way: a timer
    // starts when a node asking for it is rendered and stops when that node
    // is gone (an `if{running}` that turned false, say).
    sync_tick_timers(shared, &tree);

    *shared.retained.borrow_mut() = Some(Retained {
        nodes: tree,
        mounts,
        breakpoint,
    });

    Ok(())
}

/// Collects the distinct `(handler, interval_ms)` pairs the tree asks for.
fn find_ticks(nodes: &[Rc<UiRenderNode>], out: &mut Vec<(String, i32)>) {
    for node in nodes {
        if let Some(handler) = &node.on_tick {
            let ms = node.every_ms.unwrap_or(1000.0).clamp(10.0, 86_400_000.0) as i32;
            let key = (handler.clone(), ms);
            if !out.contains(&key) {
                out.push(key);
            }
        }
        find_ticks(&node.children, out);
    }
}

/// Starts intervals the tree newly asks for and stops the ones it no longer
/// does. A stopped timer's closure is leaked on purpose (same convention as
/// the rest of this module): the stop can happen from inside that very
/// timer's own callback, and dropping a running closure would trap.
fn sync_tick_timers(shared: &Rc<Shared>, tree: &[Rc<UiRenderNode>]) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let mut wanted = Vec::new();
    find_ticks(tree, &mut wanted);

    let mut timers = shared.timers.borrow_mut();
    let mut kept = Vec::new();
    for timer in timers.drain(..) {
        if wanted
            .iter()
            .any(|(handler, ms)| *handler == timer.handler && *ms == timer.every_ms)
        {
            kept.push(timer);
        } else {
            window.clear_interval_with_handle(timer.id);
            std::mem::forget(timer.callback);
        }
    }
    *timers = kept;

    for (handler, every_ms) in wanted {
        if timers
            .iter()
            .any(|t| t.handler == handler && t.every_ms == every_ms)
        {
            continue;
        }
        let shared_for_tick = shared.clone();
        let handler_for_tick = handler.clone();
        let callback = Closure::wrap(Box::new(move || {
            run_tick(&shared_for_tick, &handler_for_tick);
        }) as Box<dyn FnMut()>);
        if let Ok(id) = window.set_interval_with_callback_and_timeout_and_arguments_0(
            callback.as_ref().unchecked_ref(),
            every_ms,
        ) {
            timers.push(TickTimer {
                handler,
                every_ms,
                id,
                callback,
            });
        }
    }
}

/// One `tick||` interval firing: run the handler, re-render.
fn run_tick(shared: &Rc<Shared>, handler: &str) {
    sync_viewport_width(shared);
    // Bound to a local first so the `RefMut` drops before `mount_tree`
    // (which reconciles timers and may touch the session again).
    let result = match shared.session.try_borrow_mut() {
        Ok(mut session) => Some(session.dispatch(handler)),
        Err(_) => None,
    };
    match result {
        None => {}
        Some(Ok(tree)) => {
            if let Err(e) = mount_tree(tree, shared) {
                web_sys::console::error_1(&e);
            }
        }
        Some(Err(e)) => web_sys::console::error_1(&JsValue::from_str(&format!(
            "tint: tick dispatch({}) failed: {}",
            handler, e
        ))),
    }
}

/// Depth-first search for the first node in `nodes` (or its descendants)
/// that declares `frame||`. A `ui fn` is expected to put it on one root-ish
/// node (see docs/guide/events.md's `GameRoot` example), so "first found"
/// is enough -- this isn't trying to support multiple independent frame
/// loops in one tree.
fn find_frame_handler(nodes: &[Rc<UiRenderNode>]) -> Option<String> {
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
                    if let Err(e) = mount_tree(tree, &shared_for_loop) {
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

/// One mounted DOM element plus its mounted children, mirroring the shape of
/// the `UiRenderNode` it was built from. Diffing against the retained
/// previous tree happens in Rust, so an unchanged node costs no DOM calls.
struct MNode {
    el: Element,
    children: Vec<MNode>,
    has_hover: bool,
}

/// What is currently on screen: the previous render tree and its elements.
struct Retained {
    nodes: Vec<Rc<UiRenderNode>>,
    mounts: Vec<MNode>,
    breakpoint: String,
}

/// The HTML tag `build_node` picks for a render node.
fn dom_tag_name(node: &UiRenderNode) -> &'static str {
    let is_link = node.route.is_some();
    if is_link {
        "a"
    } else if node.tag == "Inline" || node.tag == "Text" {
        "span"
    } else if node.asset.is_some() && node.tag == "Image" {
        "img"
    } else if node.asset.is_some() && node.tag == "Audio" {
        "audio"
    } else if node.tag == "Button"
        || node.tag == "MenuItem"
        || node.on_click.is_some()
        || node.on_js.is_some()
    {
        "button"
    } else {
        "div"
    }
}

/// Keyed DOM patching against the retained previous tree. Keyed children are
/// matched by key, unkeyed ones by position; a match must keep both the
/// Tint tag and the HTML tag. Matched elements are patched in place (focus,
/// media playback and CSS animation state survive), unmatched old elements
/// are removed, new ones are inserted. No DOM reads are needed to decide.
fn patch_list(
    document: &Document,
    parent: &Element,
    old_nodes: &[Rc<UiRenderNode>],
    old_mounts: Vec<MNode>,
    nodes: &[Rc<UiRenderNode>],
    shared: &Rc<Shared>,
    breakpoint: &str,
) -> Result<Vec<MNode>, JsValue> {
    let mut old: Vec<Option<MNode>> = old_mounts.into_iter().map(Some).collect();
    // (mounted node, index it had in the old list if it was reused)
    let mut plan: Vec<(MNode, Option<usize>)> = Vec::with_capacity(nodes.len());

    for (index, node) in nodes.iter().enumerate() {
        let candidate = if let Some(key) = &node.key {
            old_nodes
                .iter()
                .enumerate()
                .position(|(i, o)| o.key.as_deref() == Some(key.as_str()) && old[i].is_some())
        } else if index < old.len() && old[index].is_some() {
            Some(index)
        } else {
            None
        };
        let candidate = candidate.filter(|&i| {
            old_nodes[i].tag == node.tag && dom_tag_name(&old_nodes[i]) == dom_tag_name(node)
        });
        match candidate {
            Some(i) => {
                let mounted = old[i].take().expect("candidate is unused");
                let mounted = patch_node(document, &old_nodes[i], node, mounted, shared, breakpoint)?;
                plan.push((mounted, Some(i)));
            }
            None => plan.push((build_node(document, node, shared, breakpoint)?, None)),
        }
    }

    // Unmatched old elements go away.
    for leftover in old.into_iter().flatten() {
        parent.remove_child(&leftover.el)?;
    }

    // Place elements. Reused elements already sit in their old relative
    // order; if that order still holds we only insert the new ones (and never
    // detach a reused element, which would swallow a pending `click`).
    // Otherwise (keyed reorder) fall back to re-appending everything in order.
    let mut last_reused: Option<usize> = None;
    let mut ordered = true;
    for (_, reused) in &plan {
        if let Some(i) = reused {
            if last_reused.is_some_and(|l| *i < l) {
                ordered = false;
                break;
            }
            last_reused = Some(*i);
        }
    }
    if ordered {
        let mut next: Option<web_sys::Node> = None;
        for (mounted, reused) in plan.iter().rev() {
            if reused.is_none() {
                parent.insert_before(&mounted.el, next.as_ref())?;
            }
            next = Some(mounted.el.clone().into());
        }
    } else {
        for (mounted, _) in &plan {
            parent.append_child(&mounted.el)?;
        }
    }

    Ok(plan.into_iter().map(|(m, _)| m).collect())
}

fn same_children(a: &[Rc<UiRenderNode>], b: &[Rc<UiRenderNode>]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| Rc::ptr_eq(x, y) || **x == **y)
}

fn patch_node(
    document: &Document,
    old: &Rc<UiRenderNode>,
    new: &Rc<UiRenderNode>,
    mut m: MNode,
    shared: &Rc<Shared>,
    breakpoint: &str,
) -> Result<MNode, JsValue> {
    // Nothing changed anywhere below: no DOM work at all. Hoverable nodes
    // below still need their pointer state reconciled, so they opt out.
    if !m.has_hover && (Rc::ptr_eq(old, new) || **old == **new) {
        return Ok(m);
    }
    let el = m.el.clone();

    if old.tint_source != new.tint_source {
        el.set_attribute("data-tint-source", &new.tint_source)?;
    }
    if old.key != new.key {
        match &new.key {
            Some(key) => el.set_attribute("data-tint-key", key)?,
            None => el.remove_attribute("data-tint-key")?,
        }
    }
    if old.asset != new.asset {
        if let Some(asset) = &new.asset {
            el.set_attribute("src", asset)?;
        }
    }
    if old.route != new.route || old.target != new.target {
        if let Some(route) = &new.route {
            el.set_attribute("href", route)?;
            match &new.target {
                Some(target) => {
                    el.set_attribute("target", target)?;
                    if target == "_blank" {
                        el.set_attribute("rel", "noopener noreferrer")?;
                    } else {
                        el.remove_attribute("rel")?;
                    }
                }
                None => {
                    el.remove_attribute("target")?;
                    el.remove_attribute("rel")?;
                }
            }
        }
    }
    if old.reference != new.reference {
        match &new.reference {
            Some(r) => el.set_attribute("data-tint-ref", r)?,
            None => el.remove_attribute("data-tint-ref")?,
        }
    }
    if old.on_js != new.on_js {
        match &new.on_js {
            Some(r) => el.set_attribute("data-tint-js", r)?,
            None => el.remove_attribute("data-tint-js")?,
        }
    }

    let old_had_leaf = old.svg.is_some() || old.text.is_some();
    if let Some(svg) = &new.svg {
        if old.svg.as_ref() != Some(svg) {
            el.set_inner_html(svg);
        }
        m.children.clear();
    } else if let Some(text) = &new.text {
        if old.text.as_ref() != Some(text) || old.svg.is_some() || !m.children.is_empty() {
            el.set_text_content(Some(text));
        }
        m.children.clear();
    } else {
        let (old_kids, old_mounts): (&[Rc<UiRenderNode>], Vec<MNode>) = if old_had_leaf {
            el.set_text_content(None);
            (&[], Vec::new())
        } else {
            (&old.children, std::mem::take(&mut m.children))
        };
        if !old_mounts.iter().any(|c| c.has_hover) && same_children(old_kids, &new.children) {
            m.children = old_mounts;
        } else {
            m.children = patch_list(
                document,
                &el,
                old_kids,
                old_mounts,
                &new.children,
                shared,
                breakpoint,
            )?;
        }
    }

    let hoverable = !new.hover_style.is_empty() || !old.hover_style.is_empty();
    let style_changed = old.style != new.style
        || old.breakpoints != new.breakpoints
        || old.hover_style != new.hover_style;
    if style_changed || hoverable {
        let mut props = Vec::new();
        if dom_tag_name(new) == "button" {
            props.extend(BUTTON_RESET.iter().map(|(k, v)| (k.to_string(), v.to_string())));
        }
        props.extend(new.style.iter().cloned());
        if let Some((_, styles)) = new.breakpoints.iter().find(|(name, _)| name == breakpoint) {
            props.extend(styles.iter().cloned());
        }
        if !hoverable {
            el.set_attribute("style", &style_to_css_text(&props))?;
        } else {
            // Keep the hover closures bound in `build_node` current: they
            // read these attributes at event time.
            if !new.hover_style.is_empty() {
                let base_css = style_to_css_text(&props);
                let mut hover_props = props.clone();
                hover_props.extend(new.hover_style.iter().cloned());
                let hover_css = style_to_css_text(&hover_props);
                if el.get_attribute("data-tint-base-css").as_deref() != Some(base_css.as_str()) {
                    let _ = el.set_attribute("data-tint-base-css", &base_css);
                }
                if el.get_attribute("data-tint-hover-css").as_deref() != Some(hover_css.as_str()) {
                    let _ = el.set_attribute("data-tint-hover-css", &hover_css);
                }
            }
            // Reconcile hover from the browser's actual pointer state; the
            // attribute is only a fast path between events.
            let is_hovered = !new.hover_style.is_empty()
                && el.matches(":hover").unwrap_or_else(|_| {
                    el.get_attribute("data-tint-hover").as_deref() == Some("true")
                });
            if is_hovered {
                if el.get_attribute("data-tint-hover").as_deref() != Some("true") {
                    let _ = el.set_attribute("data-tint-hover", "true");
                }
            } else if el.has_attribute("data-tint-hover") {
                let _ = el.remove_attribute("data-tint-hover");
            }
            if is_hovered {
                props.extend(new.hover_style.iter().cloned());
            }
            apply_style_props(&el, &props)?;
        }
    }

    m.has_hover = !new.hover_style.is_empty() || m.children.iter().any(|c| c.has_hover);
    Ok(m)
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
