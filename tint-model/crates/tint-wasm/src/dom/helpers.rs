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

/// Every breakpoint that applies right now, space-separated: the named screen
/// size plus each `max-N` / `min-N` threshold the tree uses that currently
/// holds. Doubles as the rebuild key -- when this string changes the tree is
/// rebuilt so the new set of breakpoint styles is layered on.
/// The DOM host wants text-only nodes folded into one element.
fn dom_tree_shape() {
    tint_runtime::ui::render::FOLD_TEXT.store(true, std::sync::atomic::Ordering::Relaxed);
}

fn active_breakpoints(used: &[String]) -> String {
    let width = current_viewport_width();
    let mut active = vec![breakpoint_for_width(width).to_string()];
    active.extend(used.iter().filter(|name| match breakpoint_threshold(name) {
        Some((true, at)) => width >= f64::from(at),
        Some((false, at)) => width <= f64::from(at),
        None => false,
    }).cloned());
    active.join(" ")
}

/// The node's breakpoint styles that apply under `active`, in source order.
fn matching_breakpoints<'a>(
    node: &'a UiRenderNode,
    active: &'a str,
) -> impl Iterator<Item = &'a Vec<(String, String)>> {
    node.breakpoints
        .iter()
        .filter(move |(name, _)| active.split(' ').any(|a| a == name))
        .map(|(_, style)| style)
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
    let route = current_route_path();
    if let Ok(mut session) = shared.session.try_borrow_mut() {
        session.set_viewport_width(width);
        // The URL is the single source of truth for `route_path`, read on
        // every render, so back/forward and manual URL edits just work.
        session.set_route_path(&route);
        // `app { route.Ui::"/path" }`: the URL decides which `ui fn` renders.
        let target = session
            .route_for_path(&route)
            .map(|declared| (declared.ui_fn.clone(), declared.title.clone()));
        if let Some((ui_fn, title)) = target {
            if session.switch_ui_fn(&ui_fn).is_err() {
                web_sys::console::error_1(&JsValue::from_str(&format!("tint: route ui fn '{}' not found", ui_fn)));
            }
            if let (Some(title), Ok(document)) = (title, document()) {
                document.set_title(&title);
            }
        }
    }
}

fn is_file_protocol(location: &web_sys::Location) -> bool {
    location.protocol().ok().as_deref() == Some("file:")
}

/// The app's current path: `location.pathname`, or the part after `#` when
/// opened from disk (`file://` pages can't use the history API).
fn current_route_path() -> String {
    let Some(window) = web_sys::window() else {
        return "/".to_string();
    };
    let location = window.location();
    let path = if is_file_protocol(&location) {
        location.hash().unwrap_or_default().trim_start_matches('#').to_string()
    } else {
        location.pathname().unwrap_or_default()
    };
    if path.starts_with('/') {
        path
    } else {
        "/".to_string()
    }
}

fn rerender_now(shared: &Rc<Shared>) {
    sync_viewport_width(shared);
    let tree = match shared.session.try_borrow_mut() {
        Ok(mut session) => match session.render() {
            Ok(tree) => tree,
            Err(e) => {
                web_sys::console::error_1(&JsValue::from_str(&format!("tint: re-render failed: {}", e)));
                return;
            }
        },
        Err(_) => return,
    };
    if let Err(e) = mount_tree(tree, shared) {
        web_sys::console::error_1(&e);
    }
}

/// `@font-face` / `@keyframes` from `app { ... }`, into one `<style id=..>`
/// (rewritten if it exists). A `Preview` writes its own, so its animations run.
fn apply_app_css(shared: &Rc<Shared>, id: &str) {
    let Ok(document) = document() else { return };
    let css = shared.session.borrow().app_css();
    if css.is_empty() {
        return;
    }
    let existing = document.get_element_by_id(id);
    let element = match existing {
        Some(element) => Some(element),
        None => document.create_element("style").ok().inspect(|element| {
            element.set_id(id);
            if let Ok(Some(head)) = document.query_selector("head") {
                let _ = head.append_child(element);
            }
        }),
    };
    if let Some(element) = element {
        element.set_text_content(Some(&css));
    }
}

/// Applies the program's `app { title, lang }` to the host page.
fn apply_app_meta(shared: &Rc<Shared>) {
    let Ok(document) = document() else { return };
    let meta = shared.session.borrow().app_meta().clone();
    if let Some(title) = &meta.title {
        document.set_title(title);
    }
    if let (Some(lang), Some(root)) = (&meta.lang, document.document_element()) {
        let _ = root.set_attribute("lang", lang);
    }
    apply_app_css(shared, "tint-app-css");
    if let Some(body) = document.body() {
        let style = body.style();
        for (property, value) in shared.session.borrow().page_style() {
            let _ = style.set_property(&property, &value);
        }
    }
}

/// With `app { router::on }`: internal links (`route||"/x"`) navigate through
/// the history API (or `#/x` on `file://`) and re-render instead of loading a
/// page; back/forward re-render too. Leaks its listeners like the rest of
/// this module -- one set per session.
fn bind_navigation(shared: &Rc<Shared>) {
    if !shared.session.borrow().app_meta().router {
        return;
    }
    let Some(window) = web_sys::window() else { return };
    let Some(doc) = window.document() else { return };

    let on_click = {
        let shared = shared.clone();
        Closure::wrap(Box::new(move |event: web_sys::Event| {
            let Some(mouse) = event.dyn_ref::<web_sys::MouseEvent>() else { return };
            if event.default_prevented()
                || mouse.button() != 0
                || mouse.meta_key()
                || mouse.ctrl_key()
                || mouse.shift_key()
                || mouse.alt_key()
            {
                return;
            }
            let Some(target) = event.target().and_then(|t| t.dyn_into::<Element>().ok()) else {
                return;
            };
            let Ok(Some(link)) = target.closest("a[data-tag][href]") else { return };
            let Ok(page) = document() else { return };
            let Some(container) = page.get_element_by_id(&shared.container_id) else { return };
            if !container.contains(Some(&link)) {
                return;
            }
            let Some(href) = link.get_attribute("href") else { return };
            if !href.starts_with('/') || href.starts_with("//") {
                return;
            }
            if link.get_attribute("target").is_some_and(|t| t != "_self") {
                return;
            }
            event.prevent_default();
            navigate_to(&href);
            rerender_now(&shared);
        }) as Box<dyn FnMut(_)>)
    };
    let _ = doc.add_event_listener_with_callback("click", on_click.as_ref().unchecked_ref());
    on_click.forget();

    for name in ["popstate", "hashchange"] {
        let shared = shared.clone();
        let cb = Closure::wrap(Box::new(move |_e: web_sys::Event| rerender_now(&shared)) as Box<dyn FnMut(_)>);
        let _ = window.add_event_listener_with_callback(name, cb.as_ref().unchecked_ref());
        cb.forget();
    }
}

fn navigate_to(href: &str) {
    let Some(window) = web_sys::window() else { return };
    let location = window.location();
    if is_file_protocol(&location) {
        let _ = location.set_hash(href);
    } else if let Ok(history) = window.history() {
        let _ = history.push_state_with_url(&JsValue::NULL, "", Some(href));
        window.scroll_to_with_x_and_y(0.0, 0.0);
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

/// Tint's built-in reset, injected once per document so apps need no CSS file
/// for it. Wrapped in `:where(...)` (specificity 0) so any app stylesheet still
/// wins, and scoped to Tint's own nodes (`[data-tag]`) so embedding Tint in a
/// larger page never restyles the host page.
const BASE_CSS: &str = concat!(
    ":where([data-tag]){box-sizing:border-box}",
    ":where(a[data-tag]){color:inherit;text-decoration:none}",
    ":where(textarea[data-tag]){appearance:none;border:0;margin:0;padding:0;background:transparent;color:inherit;font:inherit;resize:none;outline:none;overflow:hidden}",
    ":where(button[data-tag]){-webkit-tap-highlight-color:transparent;appearance:none;background:none;border:none;padding:0;margin:0;font:inherit;color:inherit;text-align:inherit}",
);

fn ensure_base_styles(document: &Document) -> Result<(), JsValue> {
    if document.get_element_by_id("tint-base-style").is_some() {
        return Ok(());
    }
    let Some(head) = document.query_selector("head")? else {
        return Ok(());
    };
    let style = document.create_element("style")?;
    style.set_id("tint-base-style");
    style.set_text_content(Some(BASE_CSS));
    // First child: any stylesheet the app adds later comes after and wins.
    head.insert_before(&style, head.first_child().as_ref())?;
    Ok(())
}

/// Clears `shared.container_id`'s children and rebuilds them from
/// `tree`. Whole-subtree teardown/rebuild, not a diff -- see this
/// module's doc comment.
fn mount_tree(tree: Vec<Rc<UiRenderNode>>, shared: &Rc<Shared>) -> Result<(), JsValue> {
    let document = document()?;
    ensure_base_styles(&document)?;
    let container = document
        .get_element_by_id(&shared.container_id)
        .ok_or_else(|| JsValue::from_str("container element not found"))?;

    // Read the viewport once per rebuild (not once per node) so a whole
    // tree is consistent even if the resolution race between reading
    // innerWidth and finishing the rebuild -- not realistic, but cheap
    // to make free.
    let prof = profiling();
    let q0 = if prof { now() } else { 0.0 };
    let scan = scan_tree(&tree);
    let breakpoint = active_breakpoints(&scan.breakpoints);

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
    let q2 = if prof { now() } else { 0.0 };
    sync_previews(shared);
    save_browser_storage(shared);
    let q3 = if prof { now() } else { 0.0 };
    // One walk over the tree (it used to be six) for every "first node that
    // declares ..." lookup and the timers.
    *shared.key_down_handler.borrow_mut() = scan.key_down;
    *shared.key_up_handler.borrow_mut() = scan.key_up;
    *shared.pointer_move_handler.borrow_mut() = scan.pointer_move;
    *shared.pointer_up_handler.borrow_mut() = scan.pointer_up;
    *shared.frame_handler.borrow_mut() = scan.frame;
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
    sync_tick_timers(shared, scan.ticks);

    let q4 = if prof { now() } else { 0.0 };
    if prof {
        web_sys::console::log_1(&JsValue::from_str(&format!("TINT-TIMING-PHASES patch {:.1} previews+storage {:.1} scan+timers {:.1}", q2 - q0, q3 - q2, q4 - q3)));
    }
    *shared.retained.borrow_mut() = Some(Retained {
        nodes: tree,
        mounts,
        breakpoint,
    });

    Ok(())
}

/// What a mounted tree asks the host for, found in a single depth-first walk:
/// the first handler of each kind (document order) and the distinct
/// `(handler, interval_ms)` timers.
#[derive(Default, Clone)]
struct TreeScan {
    key_down: Option<String>,
    key_up: Option<String>,
    pointer_move: Option<String>,
    pointer_up: Option<String>,
    frame: Option<String>,
    ticks: Vec<(String, i32)>,
    /// Distinct breakpoint names with a known threshold, first use first.
    breakpoints: Vec<String>,
}

impl TreeScan {
    fn is_empty(&self) -> bool {
        self.key_down.is_none()
            && self.key_up.is_none()
            && self.pointer_move.is_none()
            && self.pointer_up.is_none()
            && self.frame.is_none()
            && self.ticks.is_empty()
            && self.breakpoints.is_empty()
    }

    /// `other` comes later in document order: first-found wins.
    fn merge(&mut self, other: &TreeScan) {
        fn first(a: &mut Option<String>, b: &Option<String>) {
            if a.is_none() {
                a.clone_from(b);
            }
        }
        first(&mut self.key_down, &other.key_down);
        first(&mut self.key_up, &other.key_up);
        first(&mut self.pointer_move, &other.pointer_move);
        first(&mut self.pointer_up, &other.pointer_up);
        first(&mut self.frame, &other.frame);
        for t in &other.ticks {
            if !self.ticks.contains(t) {
                self.ticks.push(t.clone());
            }
        }
        for b in &other.breakpoints {
            if !self.breakpoints.contains(b) {
                self.breakpoints.push(b.clone());
            }
        }
    }
}

type ScanCache = std::collections::HashMap<usize, (Rc<UiRenderNode>, Rc<TreeScan>), tint_runtime::scope::BuildFx>;

thread_local! {
    /// Summaries of subtrees by node address. Render nodes are shared between
    /// renders, so an unchanged row costs one lookup instead of a walk.
    static SCAN_CACHE: std::cell::RefCell<ScanCache> = std::cell::RefCell::new(ScanCache::default());
}

thread_local! {
    static EMPTY_SCAN: Rc<TreeScan> = Rc::new(TreeScan::default());
}

fn scan_node(node: &Rc<UiRenderNode>, old: &ScanCache, new: &mut ScanCache) -> Rc<TreeScan> {
    // Leaves are summarized directly: caching them costs more than looking at them.
    if node.children.is_empty() {
        let plain = node.on_key_down.is_none()
            && node.on_key_up.is_none()
            && node.on_pointer_move.is_none()
            && node.on_pointer_up.is_none()
            && node.on_frame.is_none()
            && node.on_tick.is_none()
            && node.breakpoints.is_empty();
        if plain {
            return EMPTY_SCAN.with(Rc::clone);
        }
    }
    let addr = Rc::as_ptr(node) as usize;
    if let Some((_, s)) = new.get(&addr) {
        return Rc::clone(s);
    }
    if let Some((_, s)) = old.get(&addr) {
        let s = Rc::clone(s);
        new.insert(addr, (Rc::clone(node), Rc::clone(&s)));
        return s;
    }
    let mut out = TreeScan {
        key_down: node.on_key_down.clone(),
        key_up: node.on_key_up.clone(),
        pointer_move: node.on_pointer_move.clone(),
        pointer_up: node.on_pointer_up.clone(),
        frame: node.on_frame.clone(),
        ..TreeScan::default()
    };
    if let Some(handler) = &node.on_tick {
        let ms = node.every_ms.unwrap_or(1000.0).clamp(10.0, 86_400_000.0) as i32;
        out.ticks.push((handler.clone(), ms));
    }
    for (name, _) in node.breakpoints.iter() {
        if breakpoint_threshold(name).is_some() && !out.breakpoints.contains(name) {
            out.breakpoints.push(name.clone());
        }
    }
    for child in &node.children {
        let c = scan_node(child, old, new);
        if !c.is_empty() {
            out.merge(&c);
        }
    }
    let out = if out.is_empty() { EMPTY_SCAN.with(Rc::clone) } else { Rc::new(out) };
    new.insert(addr, (Rc::clone(node), Rc::clone(&out)));
    out
}

fn scan_tree(nodes: &[Rc<UiRenderNode>]) -> TreeScan {
    SCAN_CACHE.with(|cache| {
        let old = std::mem::take(&mut *cache.borrow_mut());
        let mut new = ScanCache::default();
        new.reserve(old.len());
        let mut out = TreeScan::default();
        for node in nodes {
            let s = scan_node(node, &old, &mut new);
            if !s.is_empty() {
                out.merge(&s);
            }
        }
        *cache.borrow_mut() = new;
        out
    })
}

/// Collects the distinct `(handler, interval_ms)` pairs the tree asks for.
/// Starts intervals the tree newly asks for and stops the ones it no longer
/// does. A stopped timer's closure is leaked on purpose (same convention as
/// the rest of this module): the stop can happen from inside that very
/// timer's own callback, and dropping a running closure would trap.
fn sync_tick_timers(shared: &Rc<Shared>, wanted: Vec<(String, i32)>) {
    let Some(window) = web_sys::window() else {
        return;
    };

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
/// `Preview { entry||"App" "<source>" }`: the source and entry go on the
/// element; `sync_previews` (after each mount) runs them in a nested session.
fn set_preview_attributes(el: &Element, node: &UiRenderNode) -> Result<(), JsValue> {
    el.set_attribute("data-tint-preview-source", node.text.as_deref().unwrap_or(""))?;
    el.set_attribute("data-tint-preview-entry", node.preview_entry.as_deref().unwrap_or("App"))?;
    Ok(())
}

/// `storage_set` values live in `localStorage` under this prefix, so a program
/// keeps them across reloads without any host code.
const STORAGE_PREFIX: &str = "tint:";

fn browser_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

fn load_browser_storage() -> std::collections::HashMap<String, String> {
    let mut values = std::collections::HashMap::new();
    let Some(storage) = browser_storage() else { return values };
    for index in 0..storage.length().unwrap_or(0) {
        let Ok(Some(key)) = storage.key(index) else { continue };
        let Some(name) = key.strip_prefix(STORAGE_PREFIX) else { continue };
        if let Ok(Some(value)) = storage.get_item(&key) {
            values.insert(name.to_string(), value);
        }
    }
    values
}

/// Writes changed `storage_set` values back to `localStorage` after a render.
fn save_browser_storage(shared: &Rc<Shared>) {
    if shared.nested {
        return;
    }
    // Touching `localStorage` is slow the first time, so a program that never
    // stored anything never does.
    let snapshot = shared.session.borrow().storage_snapshot();
    if snapshot.is_empty() {
        return;
    }
    let Some(storage) = browser_storage() else { return };
    for (name, value) in snapshot {
        let key = format!("{STORAGE_PREFIX}{name}");
        if storage.get_item(&key).ok().flatten().as_deref() != Some(value.as_str()) {
            let _ = storage.set_item(&key, &value);
        }
    }
}

fn show_preview_error(el: &Element, message: &str) {
    el.set_text_content(Some(message));
    let _ = el.set_attribute(
        "style",
        "padding:16px;color:#ff8a80;white-space:pre-wrap;font:12px ui-monospace,Menlo,Consolas,monospace",
    );
}

/// Starts, reloads or stops the nested sessions of every `Preview` in the
/// container so each shows exactly its current source.
/// `Preview` runs its source in a nested interpreter session; the compiled-only runtime has none.
#[cfg(not(feature = "interpreter"))]
fn sync_previews(_shared: &Rc<Shared>) {}

#[cfg(feature = "interpreter")]
fn sync_previews(shared: &Rc<Shared>) {
    let Ok(document) = document() else { return };
    let Some(container) = document.get_element_by_id(&shared.container_id) else { return };
    let Ok(found) = container.query_selector_all("[data-tag=\"Preview\"]") else { return };
    let mut alive = std::collections::HashSet::new();
    for index in 0..found.length() {
        let Some(el) = found.item(index).and_then(|n| n.dyn_into::<Element>().ok()) else { continue };
        let source = el.get_attribute("data-tint-preview-source").unwrap_or_default();
        let entry = el.get_attribute("data-tint-preview-entry").unwrap_or_else(|| "App".to_string());
        let mut id = el.id();
        if id.is_empty() {
            let n = shared.preview_seq.get() + 1;
            shared.preview_seq.set(n);
            id = format!("tint-preview-{}-{}", shared.container_id, n);
            el.set_id(&id);
        }
        alive.insert(id.clone());
        let mut slots = shared.previews.borrow_mut();
        let unchanged = slots.get(&id).is_some_and(|s| s.source == source && s.entry == entry);
        if unchanged {
            continue;
        }
        let healthy = slots.get_mut(&id).and_then(|s| s.session.as_mut());
        let session = match healthy {
            Some(session) => match session.reload(&source, &entry) {
                None => Some(None),
                Some(error) => {
                    show_preview_error(&el, &error);
                    Some(Some(error))
                }
            },
            None => None,
        };
        let (kept, failed) = match session {
            Some(None) => (true, false),
            Some(Some(_)) => (false, true),
            None => (false, false),
        };
        if kept {
            if let Some(slot) = slots.get_mut(&id) {
                slot.source = source;
                slot.entry = entry;
            }
            continue;
        }
        if failed {
            slots.insert(id, PreviewSlot { source, entry, session: None });
            continue;
        }
        // No healthy session yet: start one on a clean element.
        el.set_text_content(None);
        let _ = el.remove_attribute("style");
        let mut fresh = DomSession::new_nested(&source, &entry, &id);
        match fresh.rerender() {
            None => {
                slots.insert(id, PreviewSlot { source, entry, session: Some(fresh) });
            }
            Some(error) => {
                show_preview_error(&el, &error);
                slots.insert(id, PreviewSlot { source, entry, session: None });
            }
        }
    }
    shared.previews.borrow_mut().retain(|id, _| alive.contains(id));
}

fn dom_tag_name(node: &UiRenderNode) -> &'static str {
    let is_link = node.route.is_some();
    if is_link {
        "a"
    } else if node.tag == "TextArea" {
        "textarea"
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

    // Old indices per key, built once: looking each key up by scanning the old
    // list made a keyed list of n rows cost O(n^2) string comparisons.
    let mut by_key: std::collections::HashMap<&str, std::collections::VecDeque<usize>> =
        std::collections::HashMap::new();
    if nodes.iter().any(|n| n.key.is_some()) {
        for (i, o) in old_nodes.iter().enumerate() {
            if let Some(k) = o.key.as_deref() {
                by_key.entry(k).or_default().push_back(i);
            }
        }
    }

    // Decide which old element (if any) each new node reuses.
    let mut candidates: Vec<Option<usize>> = Vec::with_capacity(nodes.len());
    for (index, node) in nodes.iter().enumerate() {
        let candidate = if let Some(key) = &node.key {
            by_key.get_mut(key.as_str()).and_then(|v| v.pop_front())
        } else if index < old.len() && old[index].is_some() {
            Some(index)
        } else {
            None
        };
        candidates.push(candidate.filter(|&i| {
            old_nodes[i].tag == node.tag && dom_tag_name(&old_nodes[i]) == dom_tag_name(node)
        }));
    }

    // Nothing reused and everything plain: replace the parent's content outright.
    if candidates.iter().all(|c| c.is_none()) && !nodes.is_empty() {
        if let Some(mounts) = build_bulk_into(parent, nodes, shared, breakpoint)? {
            return Ok(mounts);
        }
    }

    // New plain subtrees are built together from one HTML string.
    let fresh: Vec<&Rc<UiRenderNode>> = nodes
        .iter()
        .zip(&candidates)
        .filter(|(_, c)| c.is_none())
        .map(|(n, _)| n)
        .collect();
    let (bulk, fragment) = build_bulk_many(document, &fresh, shared, breakpoint)?;
    let mut bulk = bulk.into_iter();

    for (node, candidate) in nodes.iter().zip(candidates) {
        match candidate {
            Some(i) => {
                let mounted = old[i].take().expect("candidate is unused");
                let mounted = patch_node(document, &old_nodes[i], node, mounted, shared, breakpoint)?;
                plan.push((mounted, Some(i)));
            }
            None => {
                let built = match bulk.next().expect("one entry per fresh node") {
                    Some(m) => m,
                    None => build_node(document, node, shared, breakpoint)?,
                };
                plan.push((built, None));
            }
        }
    }

    // Unmatched old elements go away.
    for leftover in old.into_iter().flatten() {
        parent.remove_child(&leftover.el)?;
    }

    // Place elements. Reused elements that already sit in increasing old order
    // (the longest such run) stay where they are; only new elements and the
    // ones outside that run are (re)inserted, walking backwards so each lands
    // before its successor. A plain insert or removal moves nothing, a swap
    // moves two.
    let stay = longest_increasing_run(&plan);
    // The common "rows appended at the end" case: everything new came out of
    // one parse and sits after every reused element, so it moves in one call.
    let first_new = plan.iter().position(|(_, reused)| reused.is_none());
    let mut placed_from = plan.len();
    if let (Some(fragment), Some(k)) = (&fragment, first_new) {
        if plan[k..].iter().all(|(_, reused)| reused.is_none()) && stay[..k].iter().all(|s| *s) {
            parent.append_child(fragment)?;
            placed_from = k;
        }
    }
    let mut next: Option<web_sys::Node> = None;
    for (pos, (mounted, _)) in plan.iter().enumerate().rev() {
        if pos >= placed_from {
            next = Some(mounted.el.clone().into());
            continue;
        }
        if !stay[pos] {
            parent.insert_before(&mounted.el, next.as_ref())?;
        }
        next = Some(mounted.el.clone().into());
    }

    Ok(plan.into_iter().map(|(m, _)| m).collect())
}

/// For each plan position, whether it belongs to the longest run of reused
/// elements whose old indices increase (those need no DOM move).
fn longest_increasing_run(plan: &[(MNode, Option<usize>)]) -> Vec<bool> {
    let mut stay = vec![false; plan.len()];
    // tails[k] = plan position ending the best run of length k+1
    let mut tails: Vec<usize> = Vec::new();
    let mut prev: Vec<Option<usize>> = vec![None; plan.len()];
    for (pos, (_, old)) in plan.iter().enumerate() {
        let Some(old) = *old else { continue };
        let at = tails.partition_point(|&t| plan[t].1.unwrap() < old);
        prev[pos] = if at > 0 { Some(tails[at - 1]) } else { None };
        if at == tails.len() {
            tails.push(pos);
        } else {
            tails[at] = pos;
        }
    }
    let mut cur = tails.last().copied();
    while let Some(pos) = cur {
        stay[pos] = true;
        cur = prev[pos];
    }
    stay
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

    if old.tint_source != new.tint_source && !new.tint_source.is_empty() {
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
    if old.attrs != new.attrs {
        apply_html_attrs(&el, &old.attrs, &new.attrs);
    }
    if old.class != new.class {
        match &new.class {
            Some(c) => el.set_attribute("class", c)?,
            None => el.remove_attribute("class")?,
        }
    }
    if old.component != new.component {
        match &new.component {
            Some(c) => el.set_attribute("data-tint-component", c)?,
            None => el.remove_attribute("data-tint-component")?,
        }
    }
    if old.props != new.props {
        match &new.props {
            Some(p) => el.set_attribute("data-tint-props", p)?,
            None => el.remove_attribute("data-tint-props")?,
        }
    }
    if old.on_js != new.on_js {
        match &new.on_js {
            Some(r) => el.set_attribute("data-tint-js", r)?,
            None => el.remove_attribute("data-tint-js")?,
        }
    }

    let old_had_leaf = old.svg.is_some() || old.text.is_some();
    if new.tag == "Preview" {
        // The element's children belong to the nested session.
        set_preview_attributes(&el, new)?;
        m.children.clear();
    } else if let Some(svg) = &new.svg {
        if old.svg.as_ref() != Some(svg) {
            el.set_inner_html(svg);
        }
        m.children.clear();
    } else if let Some(text) = &new.text {
        if let Some(area) = el.dyn_ref::<web_sys::HtmlTextAreaElement>() {
            // Typing already put this text in the field; rewriting it would
            // move the caret. Only write when the program changed it.
            if area.value() != *text {
                area.set_value(text);
            }
        } else if old.text.as_ref() != Some(text) || old.svg.is_some() || !m.children.is_empty() {
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
        props.extend(new.style.iter().cloned());
        for styles in matching_breakpoints(new, breakpoint) {
            props.extend(styles.iter().cloned());
        }
        if !hoverable {
            apply_style_props(&el, &props)?;
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

    if style_changed {
        apply_state_styles(&el, new);
        apply_gestures(&el, new);
    }
    m.has_hover = !new.hover_style.is_empty() || m.children.iter().any(|c| c.has_hover);
    Ok(m)
}

fn apply_style_props(el: &Element, props: &[(String, String)]) -> Result<(), JsValue> {
    // Replace the complete inline style in one DOM operation. Besides
    // avoiding base->hover transitions during a patch, this removes hover
    // properties (for example box-shadow) that do not exist in the base
    // style when a stale mouseleave was missed.
    let mut css = style_to_css_text(props);
    if el.has_attribute("data-tint-drag") {
        // Keep a dragged node where it was dropped.
        let (x, y) = read_drag_offset(el);
        css.push_str(&format!(" translate: {}px {}px;", x, y));
    }
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

// The `<button>` UA reset lives in BASE_CSS (zero specificity), not inline on every button.

/// Callbacks that fire after their host call returned (a settled Promise, a
/// JS timer) run against the live session and re-render it.
#[cfg(feature = "interpreter")]
fn install_deferred_runner(shared: &Rc<Shared>) {
    let weak = Rc::downgrade(shared);
    tint_runtime::vm::set_deferred_runner(Some(Rc::new(move |function, args| {
        let Some(shared) = weak.upgrade() else { return };
        let tree = match shared.session.try_borrow_mut() {
            Ok(mut session) => session.call_value(function, &args),
            Err(_) => Err("session is busy".to_string()),
        };
        match tree {
            Ok(tree) => {
                if let Err(e) = mount_tree(tree, &shared) {
                    web_sys::console::error_1(&e);
                }
            }
            Err(e) => web_sys::console::error_1(&JsValue::from_str(&format!("tint: async callback failed: {e}"))),
        }
    })));
}


thread_local! {
    static STATE_RULES: RefCell<std::collections::HashSet<u64>> = RefCell::new(std::collections::HashSet::new());
}

/// `focus::{..}`, `active::{..}`, `disabled::{..}`, `before::{..}` ...: written
/// once as `[data-tint-st="<hash>"]:focus { .. !important }` rules into a shared
/// `<style>`; identical state styles on many nodes share one rule set. Inline
/// styles outrank sheet rules, hence `!important`.
fn apply_state_styles(el: &Element, node: &UiRenderNode) {
    let states: Vec<&(String, Vec<(String, String)>)> =
        node.breakpoints.iter().filter(|(name, _)| name.starts_with(':')).collect();
    if states.is_empty() {
        if el.has_attribute("data-tint-st") {
            let _ = el.remove_attribute("data-tint-st");
        }
        return;
    }
    let body = |style: &Vec<(String, String)>| {
        merge_style_props(style)
            .iter()
            .map(|(k, v)| format!("{}:{} !important;", k, v))
            .collect::<String>()
    };
    let bodies: Vec<(String, String)> = states.iter().map(|(n, s)| (n.clone(), body(s))).collect();
    let mut hash: u64 = 0xcbf29ce484222325;
    for (name, css) in &bodies {
        for byte in name.bytes().chain(css.bytes()).chain([0u8]) {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    let id = format!("{:x}", hash);
    if el.get_attribute("data-tint-st").as_deref() != Some(id.as_str()) {
        let _ = el.set_attribute("data-tint-st", &id);
    }
    let fresh = STATE_RULES.with(|rules| rules.borrow_mut().insert(hash));
    if !fresh {
        return;
    }
    let Ok(doc) = document() else { return };
    let sheet = match doc.get_element_by_id("tint-states") {
        Some(sheet) => sheet,
        None => {
            let Ok(sheet) = doc.create_element("style") else { return };
            let _ = sheet.set_attribute("id", "tint-states");
            if let Ok(Some(head)) = doc.query_selector("head") {
                let _ = head.append_child(&sheet);
            }
            sheet
        }
    };
    let mut css = sheet.text_content().unwrap_or_default();
    for (name, body) in &bodies {
        css.push_str(&format!("[data-tint-st=\"{}\"]{}{{{}}}\n", id, name, body));
    }
    sheet.set_text_content(Some(&css));
}

/// Plain HTML attributes (`placeholder`, `disabled`, ...).
fn apply_html_attrs(el: &Element, old: &[(String, String)], new: &[(String, String)]) {
    for (name, _) in old {
        if !new.iter().any(|(n, _)| n == name) {
            let _ = el.remove_attribute(name);
        }
    }
    for (name, value) in new {
        if el.get_attribute(name).as_deref() != Some(value.as_str()) {
            let _ = el.set_attribute(name, value);
        }
    }
}

/// `drag::...` (see tint-runtime style/spring.rs): binds pointer listeners once
/// per element. The offset is applied as the CSS `translate` property and
/// remembered in `data-tint-drag` so a re-render keeps the node where it was
/// dropped (see `apply_style_props`).
fn apply_gestures(el: &Element, node: &UiRenderNode) {
    let mode = node.style.iter().rev().find(|(k, _)| k == "--tint-drag").map(|(_, v)| v.clone());
    let Some(mode) = mode else { return };
    let back = node.style.iter().any(|(k, _)| k == "--tint-drag-return");
    let _ = el.set_attribute("data-tint-drag-mode", &mode);
    if back {
        let _ = el.set_attribute("data-tint-drag-return", "1");
    } else {
        let _ = el.remove_attribute("data-tint-drag-return");
    }
    if el.has_attribute("data-tint-drag-bound") {
        return;
    }
    let _ = el.set_attribute("data-tint-drag-bound", "1");
    let start: Rc<Cell<Option<(f64, f64, f64, f64)>>> = Rc::new(Cell::new(None));

    let (target, st) = (el.clone(), start.clone());
    let down = Closure::wrap(Box::new(move |e: web_sys::PointerEvent| {
        let (ox, oy) = read_drag_offset(&target);
        st.set(Some((f64::from(e.client_x()), f64::from(e.client_y()), ox, oy)));
        let _ = target.set_pointer_capture(e.pointer_id());
        let _ = target.set_attribute("data-tint-dragging", "1");
    }) as Box<dyn FnMut(_)>);
    let _ = el.add_event_listener_with_callback("pointerdown", down.as_ref().unchecked_ref());
    down.forget();

    let (target, st) = (el.clone(), start.clone());
    let moved = Closure::wrap(Box::new(move |e: web_sys::PointerEvent| {
        let Some((sx, sy, ox, oy)) = st.get() else { return };
        let mode = target.get_attribute("data-tint-drag-mode").unwrap_or_default();
        let dx = if mode == "y" { 0.0 } else { f64::from(e.client_x()) - sx };
        let dy = if mode == "x" { 0.0 } else { f64::from(e.client_y()) - sy };
        write_drag_offset(&target, ox + dx, oy + dy);
    }) as Box<dyn FnMut(_)>);
    let _ = el.add_event_listener_with_callback("pointermove", moved.as_ref().unchecked_ref());
    moved.forget();

    let (target, st) = (el.clone(), start);
    let up = Closure::wrap(Box::new(move |_e: web_sys::PointerEvent| {
        if st.take().is_none() {
            return;
        }
        let _ = target.remove_attribute("data-tint-dragging");
        if target.has_attribute("data-tint-drag-return") {
            write_drag_offset(&target, 0.0, 0.0);
        }
    }) as Box<dyn FnMut(_)>);
    let _ = el.add_event_listener_with_callback("pointerup", up.as_ref().unchecked_ref());
    let _ = el.add_event_listener_with_callback("pointercancel", up.as_ref().unchecked_ref());
    up.forget();
}

fn read_drag_offset(el: &Element) -> (f64, f64) {
    let text = el.get_attribute("data-tint-drag").unwrap_or_default();
    let mut parts = text.split(',').map(|p| p.parse::<f64>().unwrap_or(0.0));
    (parts.next().unwrap_or(0.0), parts.next().unwrap_or(0.0))
}

fn write_drag_offset(el: &Element, x: f64, y: f64) {
    let _ = el.set_attribute("data-tint-drag", &format!("{},{}", x, y));
    if let Some(html) = el.dyn_ref::<web_sys::HtmlElement>() {
        let _ = html.style().set_property("translate", &format!("{}px {}px", x, y));
    }
}
