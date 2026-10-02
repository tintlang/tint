// `mount||h`, `unmount||h`, `effect||h deps||{..}`: handlers the host runs when a node
// enters the page, leaves it, or when its `deps` change. The nodes carry them as
// `data-tint-mount` / `-unmount` / `-effect` / `-deps`; one MutationObserver per
// container watches for them. A node moved by a keyed reorder is removed and added in
// one batch and counts as neither.

const LIFE_SELECTOR: &str = "[data-tint-mount],[data-tint-effect],[data-tint-unmount],[data-tint-dismiss],[data-tint-anchor],[data-tint-trap],[data-tint-poll]";

fn life_flag(el: &Element, key: &str) -> Option<String> {
    js_sys::Reflect::get(el, &JsValue::from_str(key)).ok().and_then(|v| v.as_string())
}

fn life_set(el: &Element, key: &str, value: Option<&str>) {
    let v = value.map_or(JsValue::UNDEFINED, JsValue::from_str);
    let _ = js_sys::Reflect::set(el, &JsValue::from_str(key), &v);
}

/// An element that belongs to a nested `Preview`'s own session, not to `root`'s.
fn in_other_session(el: &Element, root: &Element) -> bool {
    match el.closest("[data-tint-preview-source]") {
        Ok(Some(p)) => !p.contains(Some(root)),
        _ => false,
    }
}

fn life_elements(node: &web_sys::Node) -> Vec<Element> {
    let mut out = Vec::new();
    let Some(el) = node.dyn_ref::<Element>() else { return out };
    if el.matches(LIFE_SELECTOR).unwrap_or(false) {
        out.push(el.clone());
    }
    if let Ok(list) = el.query_selector_all(LIFE_SELECTOR) {
        for i in 0..list.length() {
            if let Some(e) = list.item(i).and_then(|n| n.dyn_into::<Element>().ok()) {
                out.push(e);
            }
        }
    }
    out
}

fn life_enter(el: &Element, root: &Element, shared: &Rc<Shared>, run: &mut Vec<String>) {
    if !el.is_connected() || in_other_session(el, root) || life_flag(el, "__tintMounted").is_some() {
        return;
    }
    life_set(el, "__tintMounted", Some("1"));
    life_set(el, "__tintDeps", Some(&el.get_attribute("data-tint-deps").unwrap_or_default()));
    for attr in ["data-tint-mount", "data-tint-effect"] {
        if let Some(h) = el.get_attribute(attr) {
            run.push(h);
        }
    }
    overlay_open(el, shared);
    poll_start(el, shared);
}

/// `poll||h every||ms`: `h` runs every `ms` while the node is on the page.
fn poll_start(el: &Element, shared: &Rc<Shared>) {
    let (Some(h), Some(window)) = (el.get_attribute("data-tint-poll"), web_sys::window()) else { return };
    let ms = el.get_attribute("data-tint-every").and_then(|v| v.parse::<i32>().ok()).unwrap_or(1000).max(50);
    let (node, shared, id) = (el.clone(), shared.clone(), Rc::new(Cell::new(0)));
    let id2 = id.clone();
    let cb = Closure::wrap(Box::new(move || {
        if !node.is_connected() {
            if let Some(w) = web_sys::window() {
                w.clear_interval_with_handle(id2.get());
            }
            return;
        }
        run_click(&shared, &h);
    }) as Box<dyn FnMut()>);
    if let Ok(n) = window.set_interval_with_callback_and_timeout_and_arguments_0(cb.as_ref().unchecked_ref(), ms) {
        id.set(n);
        life_set(el, "__tintPoll", Some(&n.to_string()));
    }
    cb.forget();
}

fn life_leave(el: &Element, root: &Element, run: &mut Vec<String>) {
    if el.is_connected() || life_flag(el, "__tintMounted").is_none() || in_other_session(el, root) {
        return;
    }
    life_set(el, "__tintMounted", None);
    overlay_close(el);
    if let (Some(n), Some(w)) = (life_flag(el, "__tintPoll").and_then(|v| v.parse::<i32>().ok()), web_sys::window()) {
        w.clear_interval_with_handle(n);
        life_set(el, "__tintPoll", None);
    }
    if let Some(h) = el.get_attribute("data-tint-unmount") {
        run.push(h);
    }
}

fn life_deps(el: &Element, run: &mut Vec<String>) {
    if life_flag(el, "__tintMounted").is_none() {
        return;
    }
    let now = el.get_attribute("data-tint-deps").unwrap_or_default();
    if life_flag(el, "__tintDeps").as_deref() == Some(now.as_str()) {
        return;
    }
    life_set(el, "__tintDeps", Some(&now));
    if let Some(h) = el.get_attribute("data-tint-effect") {
        run.push(h);
    }
}

fn bind_lifecycle(container: &Element, shared: &Rc<Shared>) {
    if shared.life_bound.replace(true) {
        return;
    }
    let root = container.clone();
    let shared2 = shared.clone();
    let cb = Closure::wrap(Box::new(move |records: js_sys::Array| {
        let mut run: Vec<String> = Vec::new();
        let mut added: Vec<Element> = Vec::new();
        for r in records.iter() {
            let r: web_sys::MutationRecord = r.unchecked_into();
            if r.type_() == "attributes" {
                if let Some(el) = r.target().and_then(|t| t.dyn_into::<Element>().ok()) {
                    life_deps(&el, &mut run);
                }
                continue;
            }
            let removed = r.removed_nodes();
            for i in 0..removed.length() {
                if let Some(n) = removed.item(i) {
                    for el in life_elements(&n) {
                        life_leave(&el, &root, &mut run);
                    }
                }
            }
            let adds = r.added_nodes();
            for i in 0..adds.length() {
                if let Some(n) = adds.item(i) {
                    added.extend(life_elements(&n));
                }
            }
        }
        for el in added {
            life_enter(&el, &root, &shared2, &mut run);
        }
        overlay_place_all();
        for h in run {
            run_click(&shared2, &h);
        }
    }) as Box<dyn FnMut(js_sys::Array)>);
    let Ok(observer) = web_sys::MutationObserver::new(cb.as_ref().unchecked_ref()) else { return };
    cb.forget();
    let init = web_sys::MutationObserverInit::new();
    init.set_child_list(true);
    init.set_subtree(true);
    init.set_attributes(true);
    init.set_attribute_filter(&js_sys::Array::of1(&JsValue::from_str("data-tint-deps")));
    let _ = observer.observe_with_options(container, &init);
}

// ---- overlays -------------------------------------------------------------
//
// A node with `dismiss||h`, `anchor||"ref"` / `place||".."` or `trap||true` is an overlay while
// it is on the page: Escape or a pointer press outside it runs `h` (the topmost overlay only),
// `anchor` keeps it next to the node marked `ref||"name"` (flipping to the other side when it
// would leave the window), and `trap` moves focus into it, keeps Tab inside it and gives
// the focus back when it closes.

struct Overlay {
    el: Element,
    shared: Rc<Shared>,
    previous_focus: Option<web_sys::HtmlElement>,
}

thread_local! {
    static OVERLAYS: RefCell<Vec<Overlay>> = const { RefCell::new(Vec::new()) };
    static OVERLAY_EVENTS: Cell<bool> = const { Cell::new(false) };
}

const FOCUSABLE: &str = "a[href],button:not([disabled]),input:not([disabled]),select:not([disabled]),textarea:not([disabled]),[tabindex]:not([tabindex=\"-1\"])";

fn focusables(el: &Element) -> Vec<web_sys::HtmlElement> {
    let mut out = Vec::new();
    if let Ok(list) = el.query_selector_all(FOCUSABLE) {
        for i in 0..list.length() {
            if let Some(e) = list.item(i).and_then(|n| n.dyn_into::<web_sys::HtmlElement>().ok()) {
                out.push(e);
            }
        }
    }
    out
}

fn overlay_open(el: &Element, shared: &Rc<Shared>) {
    let is_overlay = ["data-tint-dismiss", "data-tint-anchor", "data-tint-trap"].iter().any(|a| el.has_attribute(a));
    if !is_overlay {
        return;
    }
    let document = web_sys::window().and_then(|w| w.document());
    let previous_focus = document
        .as_ref()
        .and_then(|d| d.active_element())
        .and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok());
    if el.has_attribute("data-tint-trap") {
        match focusables(el).first() {
            Some(first) => {
                let _ = first.focus();
            }
            None => {
                let _ = el.set_attribute("tabindex", "-1");
                if let Some(h) = el.dyn_ref::<web_sys::HtmlElement>() {
                    let _ = h.focus();
                }
            }
        }
    }
    OVERLAYS.with(|o| o.borrow_mut().push(Overlay { el: el.clone(), shared: shared.clone(), previous_focus }));
    overlay_events();
    overlay_place(el);
}

fn overlay_close(el: &Element) {
    let found = OVERLAYS.with(|o| {
        let mut list = o.borrow_mut();
        list.iter().position(|x| &x.el == el).map(|i| list.remove(i))
    });
    if let Some(Overlay { previous_focus: Some(prev), .. }) = found {
        if el.has_attribute("data-tint-trap") && prev.is_connected() {
            let _ = prev.focus();
        }
    }
}

fn overlay_events() {
    if OVERLAY_EVENTS.with(|b| b.replace(true)) {
        return;
    }
    let Some(window) = web_sys::window() else { return };
    let Some(document) = window.document() else { return };
    let down = Closure::wrap(Box::new(move |e: web_sys::Event| {
        let target = e.target().and_then(|t| t.dyn_into::<web_sys::Node>().ok());
        let top = OVERLAYS.with(|o| {
            o.borrow().iter().rev().find(|x| x.el.has_attribute("data-tint-dismiss")).map(|x| (x.el.clone(), x.shared.clone()))
        });
        let (Some((el, shared)), Some(target)) = (top, target) else { return };
        if el.contains(Some(&target)) {
            return;
        }
        let anchor = el.get_attribute("data-tint-anchor").and_then(|n| ref_element(&n));
        if anchor.is_some_and(|a| a.contains(Some(&target))) {
            return;
        }
        if let Some(h) = el.get_attribute("data-tint-dismiss") {
            run_click(&shared, &h);
        }
    }) as Box<dyn FnMut(_)>);
    let opts = web_sys::AddEventListenerOptions::new();
    opts.set_capture(true);
    let _ = document.add_event_listener_with_callback_and_add_event_listener_options("pointerdown", down.as_ref().unchecked_ref(), &opts);
    down.forget();
    let key = Closure::wrap(Box::new(move |e: web_sys::KeyboardEvent| {
        let top = OVERLAYS.with(|o| o.borrow().last().map(|x| (x.el.clone(), x.shared.clone())));
        let Some((el, shared)) = top else { return };
        match e.key().as_str() {
            "Escape" => {
                if let Some(h) = OVERLAYS.with(|o| {
                    o.borrow().iter().rev().find_map(|x| x.el.get_attribute("data-tint-dismiss"))
                }) {
                    e.prevent_default();
                    run_click(&shared, &h);
                }
            }
            "Tab" if el.has_attribute("data-tint-trap") => {
                let items = focusables(&el);
                let (Some(first), Some(last)) = (items.first(), items.last()) else {
                    e.prevent_default();
                    return;
                };
                let active = web_sys::window().and_then(|w| w.document()).and_then(|d| d.active_element());
                let inside = active.as_ref().is_some_and(|a| el.contains(Some(a)));
                let at = |x: &web_sys::HtmlElement| active.as_ref().is_some_and(|a| a == x.as_ref() as &Element);
                if !inside || (e.shift_key() && at(first)) {
                    e.prevent_default();
                    let _ = if inside { last.focus() } else { first.focus() };
                } else if !e.shift_key() && at(last) {
                    e.prevent_default();
                    let _ = first.focus();
                }
            }
            _ => {}
        }
    }) as Box<dyn FnMut(_)>);
    let _ = document.add_event_listener_with_callback("keydown", key.as_ref().unchecked_ref());
    key.forget();
    let again = Closure::wrap(Box::new(move |_e: web_sys::Event| overlay_place_all()) as Box<dyn FnMut(_)>);
    let _ = window.add_event_listener_with_callback("resize", again.as_ref().unchecked_ref());
    let opts = web_sys::AddEventListenerOptions::new();
    opts.set_capture(true);
    let _ = window.add_event_listener_with_callback_and_add_event_listener_options("scroll", again.as_ref().unchecked_ref(), &opts);
    again.forget();
}

fn overlay_place_all() {
    let els: Vec<Element> = OVERLAYS.with(|o| o.borrow().iter().map(|x| x.el.clone()).collect());
    for el in els {
        overlay_place(&el);
    }
}

/// Puts `el` next to its `anchor||"ref"` node as `place||"bottom-start"` says
/// (`top`/`bottom`/`left`/`right`, then `-start`/`-end`; no suffix centres it).
fn overlay_place(el: &Element) {
    let Some(anchor) = el.get_attribute("data-tint-anchor").and_then(|n| ref_element(&n)) else { return };
    let Some(node) = el.dyn_ref::<web_sys::HtmlElement>() else { return };
    let Some(window) = web_sys::window() else { return };
    let (vw, vh) = (
        window.inner_width().ok().and_then(|v| v.as_f64()).unwrap_or(0.0),
        window.inner_height().ok().and_then(|v| v.as_f64()).unwrap_or(0.0),
    );
    let a = anchor.get_bounding_client_rect();
    let (w, h) = (node.offset_width() as f64, node.offset_height() as f64);
    let place = el.get_attribute("data-tint-place").unwrap_or_else(|| "bottom-start".into());
    let (side, align) = place.split_once('-').unwrap_or((place.as_str(), ""));
    let gap = 8.0;
    let mut side = side;
    match side {
        "bottom" if a.bottom() + gap + h > vh && a.top() - gap - h >= 0.0 => side = "top",
        "top" if a.top() - gap - h < 0.0 && a.bottom() + gap + h <= vh => side = "bottom",
        "right" if a.right() + gap + w > vw && a.left() - gap - w >= 0.0 => side = "left",
        "left" if a.left() - gap - w < 0.0 && a.right() + gap + w <= vw => side = "right",
        _ => {}
    }
    let across = |start: f64, end: f64, size: f64| match align {
        "start" => start,
        "end" => end - size,
        _ => start + (end - start - size) / 2.0,
    };
    let (mut left, mut top) = match side {
        "top" => (across(a.left(), a.right(), w), a.top() - gap - h),
        "right" => (a.right() + gap, across(a.top(), a.bottom(), h)),
        "left" => (a.left() - gap - w, across(a.top(), a.bottom(), h)),
        _ => (across(a.left(), a.right(), w), a.bottom() + gap),
    };
    left = left.min(vw - w - 8.0).max(8.0);
    top = top.min(vh - h - 8.0).max(8.0);
    let style = node.style();
    let _ = style.set_property("position", "fixed");
    let _ = style.set_property("margin", "0");
    let _ = style.set_property("left", &format!("{left:.0}px"));
    let _ = style.set_property("top", &format!("{top:.0}px"));
}
