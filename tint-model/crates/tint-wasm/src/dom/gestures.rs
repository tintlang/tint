// `pan||h`, `swipe||h`, `long_press||h`: pointer gestures on any node. The handlers sit on the node as
// `data-tint-pan` / `-swipe` / `-long_press`; one set of listeners on the container serves them all.
//
//   pan         h(dx, dy)  on every move of a press that started on the node (px from where it started)
//   swipe       h(dir)     on release after a quick, long enough move: "left" "right" "up" "down"
//   long_press  h()        after the pointer rested on the node for 500 ms
//
// A node that pans or swipes on touch screens needs `touch-action::none` (or `pan-y` / `pan-x`).

const GESTURE_SELECTOR: &str = "[data-tint-pan],[data-tint-swipe],[data-tint-long_press]";
const LONG_PRESS_MS: i32 = 500;
const SWIPE_MIN_PX: f64 = 40.0;
const SWIPE_MAX_MS: f64 = 600.0;
const MOVE_SLOP_PX: f64 = 8.0;

struct Press {
    el: Element,
    x: f64,
    y: f64,
    t: f64,
    timer: Option<i32>,
    moved: bool,
}

fn bind_gestures(container: &Element, shared: &Rc<Shared>) {
    if shared.gesture_bound.replace(true) {
        return;
    }
    bind_extras(container, shared);
    let press: Rc<RefCell<Option<Press>>> = Rc::new(RefCell::new(None));
    let root = container.clone();

    let (p, s, r) = (press.clone(), shared.clone(), root.clone());
    let down = Closure::wrap(Box::new(move |e: web_sys::PointerEvent| {
        let Some(target) = e.target().and_then(|t| t.dyn_into::<Element>().ok()) else { return };
        let Ok(Some(el)) = target.closest(GESTURE_SELECTOR) else { return };
        if !r.contains(Some(&el)) {
            return;
        }
        let mut timer = None;
        if let (Some(h), Some(window)) = (el.get_attribute("data-tint-long_press"), web_sys::window()) {
            let (p2, s2) = (p.clone(), s.clone());
            let cb = Closure::once_into_js(move || {
                let fire = match p2.borrow_mut().as_mut() {
                    Some(press) if !press.moved => {
                        press.timer = None;
                        true
                    }
                    _ => false,
                };
                if fire {
                    run_click(&s2, &h);
                }
            });
            timer = window.set_timeout_with_callback_and_timeout_and_arguments_0(cb.unchecked_ref(), LONG_PRESS_MS).ok();
        }
        *p.borrow_mut() = Some(Press { el, x: f64::from(e.client_x()), y: f64::from(e.client_y()), t: now(), timer, moved: false });
    }) as Box<dyn FnMut(_)>);
    let _ = container.add_event_listener_with_callback("pointerdown", down.as_ref().unchecked_ref());
    down.forget();

    let (p, s) = (press.clone(), shared.clone());
    let moved = Closure::wrap(Box::new(move |e: web_sys::PointerEvent| {
        let (handler, dx, dy) = {
            let mut guard = p.borrow_mut();
            let Some(press) = guard.as_mut() else { return };
            let (dx, dy) = (f64::from(e.client_x()) - press.x, f64::from(e.client_y()) - press.y);
            if dx.abs() > MOVE_SLOP_PX || dy.abs() > MOVE_SLOP_PX {
                press.moved = true;
                if let (Some(id), Some(window)) = (press.timer.take(), web_sys::window()) {
                    window.clear_timeout_with_handle(id);
                }
            }
            (press.el.get_attribute("data-tint-pan"), dx, dy)
        };
        if let Some(h) = handler {
            dispatch_pointer(&s, &h, dx, dy);
        }
    }) as Box<dyn FnMut(_)>);
    if let Some(window) = web_sys::window() {
        let _ = window.add_event_listener_with_callback("pointermove", moved.as_ref().unchecked_ref());
    }
    moved.forget();

    let (p, s) = (press, shared.clone());
    let up = Closure::wrap(Box::new(move |e: web_sys::PointerEvent| {
        let Some(press) = p.borrow_mut().take() else { return };
        if let (Some(id), Some(window)) = (press.timer, web_sys::window()) {
            window.clear_timeout_with_handle(id);
        }
        let Some(h) = press.el.get_attribute("data-tint-swipe") else { return };
        let (dx, dy) = (f64::from(e.client_x()) - press.x, f64::from(e.client_y()) - press.y);
        if now() - press.t > SWIPE_MAX_MS || dx.abs().max(dy.abs()) < SWIPE_MIN_PX {
            return;
        }
        let dir = if dx.abs() >= dy.abs() {
            if dx > 0.0 { "right" } else { "left" }
        } else if dy > 0.0 {
            "down"
        } else {
            "up"
        };
        dispatch_text(&s, &h, dir.to_string());
    }) as Box<dyn FnMut(_)>);
    if let Some(window) = web_sys::window() {
        let _ = window.add_event_listener_with_callback("pointerup", up.as_ref().unchecked_ref());
        let _ = window.add_event_listener_with_callback("pointercancel", up.as_ref().unchecked_ref());
    }
    up.forget();
}

// ---- hotkeys ----
//
// `hotkey||h keys||"mod+k"`: while the node is on the page, the shortcut runs `h`. `keys` is a list
// separated by commas (`"mod+k, /"`); a shortcut is modifiers and a key joined by `+`: `mod` (Ctrl, or
// Cmd on a Mac), `ctrl`, `meta`, `alt`, `shift`, then a key name (`k`, `/`, `enter`, `esc`, `space`, `up`...).
// A shortcut without a modifier does not fire while a text field has the focus.

fn key_matches(spec: &str, e: &web_sys::KeyboardEvent) -> bool {
    let parts: Vec<String> = spec.split('+').map(|p| p.trim().to_lowercase()).collect();
    let Some(key) = parts.last().cloned() else { return false };
    let mods = &parts[..parts.len() - 1];
    let has = |m: &str| mods.iter().any(|x| x == m);
    let (ctrl_or_meta, ctrl, meta) = (e.ctrl_key() || e.meta_key(), e.ctrl_key(), e.meta_key());
    let wants_mod = has("mod");
    if wants_mod != (ctrl_or_meta && !has("ctrl") && !has("meta")) && !(has("ctrl") || has("meta")) {
        return false;
    }
    if has("ctrl") != ctrl && !wants_mod || has("meta") != meta && !wants_mod {
        return false;
    }
    if has("alt") != e.alt_key() || has("shift") != e.shift_key() && key.chars().count() > 1 {
        return false;
    }
    let pressed = e.key().to_lowercase();
    let want = match key.as_str() {
        "esc" => "escape",
        "space" => " ",
        "up" => "arrowup",
        "down" => "arrowdown",
        "left" => "arrowleft",
        "right" => "arrowright",
        other => other,
    };
    pressed == want
}

fn bind_hotkeys(container: &Element, shared: &Rc<Shared>) {
    if shared.hotkey_bound.replace(true) {
        return;
    }
    let (root, shared) = (container.clone(), shared.clone());
    let cb = Closure::wrap(Box::new(move |e: web_sys::KeyboardEvent| {
        let Ok(list) = root.query_selector_all("[data-tint-hotkey]") else { return };
        let typing = e
            .target()
            .and_then(|t| t.dyn_into::<Element>().ok())
            .is_some_and(|t| matches!(t.tag_name().as_str(), "INPUT" | "TEXTAREA" | "SELECT") || t.has_attribute("contenteditable"));
        for i in 0..list.length() {
            let Some(el) = list.item(i).and_then(|n| n.dyn_into::<Element>().ok()) else { continue };
            let (Some(h), Some(keys)) = (el.get_attribute("data-tint-hotkey"), el.get_attribute("data-tint-keys")) else { continue };
            for spec in keys.split(',') {
                let plain = !spec.contains("mod") && !spec.contains("ctrl") && !spec.contains("meta") && !spec.contains("alt");
                if typing && plain && !spec.trim().eq_ignore_ascii_case("esc") {
                    continue;
                }
                if key_matches(spec, &e) {
                    e.prevent_default();
                    run_click(&shared, &h);
                    return;
                }
            }
        }
    }) as Box<dyn FnMut(_)>);
    if let Some(document) = web_sys::window().and_then(|w| w.document()) {
        let _ = document.add_event_listener_with_callback("keydown", cb.as_ref().unchecked_ref());
    }
    cb.forget();
}

// ---- blur, scroll, sortable ---------------------------------------------------------------------
//
//   blur||h       h()          focus left the node (or something inside it)
//   scroll||h     h(top, h)    the node was scrolled: scrollTop and its visible height in px
//   sortable||h   h(from, to)  its children were dragged into a new order (the app reorders the list)

struct Sort {
    items: Vec<web_sys::HtmlElement>,
    from: usize,
    to: usize,
    centers: Vec<f64>,
    step: f64,
    horizontal: bool,
    x: f64,
    y: f64,
    active: bool,
    handler: String,
}

fn bind_extras(container: &Element, shared: &Rc<Shared>) {
    let Some(document) = web_sys::window().and_then(|w| w.document()) else { return };
    // blur
    let (r, s) = (container.clone(), shared.clone());
    let cb = Closure::wrap(Box::new(move |e: web_sys::Event| {
        let Some(t) = e.target().and_then(|t| t.dyn_into::<Element>().ok()) else { return };
        let Ok(Some(el)) = t.closest("[data-tint-blur]") else { return };
        if !r.contains(Some(&el)) {
            return;
        }
        if let Some(h) = el.get_attribute("data-tint-blur") {
            run_click(&s, &h);
        }
    }) as Box<dyn FnMut(_)>);
    let _ = container.add_event_listener_with_callback("focusout", cb.as_ref().unchecked_ref());
    cb.forget();
    // scroll (it does not bubble: listen while it travels down)
    let (r, s) = (container.clone(), shared.clone());
    let cb = Closure::wrap(Box::new(move |e: web_sys::Event| {
        let Some(el) = e.target().and_then(|t| t.dyn_into::<Element>().ok()) else { return };
        let Some(h) = el.get_attribute("data-tint-scroll") else { return };
        if r.contains(Some(&el)) {
            dispatch_pointer(&s, &h, el.scroll_top() as f64, el.client_height() as f64);
        }
    }) as Box<dyn FnMut(_)>);
    let opts = web_sys::AddEventListenerOptions::new();
    opts.set_capture(true);
    opts.set_passive(true);
    let _ = container.add_event_listener_with_callback_and_add_event_listener_options("scroll", cb.as_ref().unchecked_ref(), &opts);
    cb.forget();
    // sortable
    let sort: Rc<RefCell<Option<Sort>>> = Rc::new(RefCell::new(None));
    let just_dragged = Rc::new(Cell::new(0.0f64));
    let (r, st) = (container.clone(), sort.clone());
    let down = Closure::wrap(Box::new(move |e: web_sys::PointerEvent| {
        if e.button() != 0 {
            return;
        }
        let Some(t) = e.target().and_then(|t| t.dyn_into::<Element>().ok()) else { return };
        let Ok(Some(item)) = t.closest("[data-tint-sortable] > *") else { return };
        let Some(list) = item.parent_element() else { return };
        if !r.contains(Some(&list)) {
            return;
        }
        if let Ok(Some(inner)) = t.closest("button,input,textarea,select,a") {
            if inner != item && item.contains(Some(&inner)) {
                return;
            }
        }
        let Some(handler) = list.get_attribute("data-tint-sortable") else { return };
        let kids = list.children();
        let mut items = Vec::new();
        for i in 0..kids.length() {
            if let Some(k) = kids.item(i).and_then(|k| k.dyn_into::<web_sys::HtmlElement>().ok()) {
                items.push(k);
            }
        }
        let Some(from) = items.iter().position(|k| k.is_same_node(Some(&item))) else { return };
        let horizontal = web_sys::window()
            .and_then(|w| w.get_computed_style(&list).ok().flatten())
            .and_then(|c| c.get_property_value("flex-direction").ok())
            .is_some_and(|d| d.starts_with("row"));
        let pos = |k: &web_sys::HtmlElement| {
            let r = k.get_bounding_client_rect();
            if horizontal { (r.left(), r.width()) } else { (r.top(), r.height()) }
        };
        let centers: Vec<f64> = items.iter().map(|k| { let (a, b) = pos(k); a + b / 2.0 }).collect();
        let step = if items.len() < 2 {
            0.0
        } else if from + 1 < items.len() {
            (centers[from + 1] - centers[from]).abs()
        } else {
            (centers[from] - centers[from - 1]).abs()
        };
        *st.borrow_mut() = Some(Sort { items, from, to: from, centers, step, horizontal, x: e.client_x() as f64, y: e.client_y() as f64, active: false, handler });
    }) as Box<dyn FnMut(_)>);
    let _ = container.add_event_listener_with_callback("pointerdown", down.as_ref().unchecked_ref());
    down.forget();

    let st = sort.clone();
    let mv = Closure::wrap(Box::new(move |e: web_sys::PointerEvent| {
        let mut guard = st.borrow_mut();
        let Some(s) = guard.as_mut() else { return };
        let (dx, dy) = (e.client_x() as f64 - s.x, e.client_y() as f64 - s.y);
        let d = if s.horizontal { dx } else { dy };
        if !s.active {
            if dx.hypot(dy) < 6.0 {
                return;
            }
            s.active = true;
            for (i, k) in s.items.iter().enumerate() {
                let style = k.style();
                if i == s.from {
                    let _ = style.set_property("z-index", "5");
                    let _ = style.set_property("transition", "none");
                    let _ = style.set_property("cursor", "grabbing");
                    let _ = style.set_property("user-select", "none");
                } else {
                    let _ = style.set_property("transition", "transform 160ms ease");
                }
            }
        }
        e.prevent_default();
        let axis = if s.horizontal { "translateX" } else { "translateY" };
        let _ = s.items[s.from].style().set_property("transform", &format!("{axis}({d}px)"));
        let dc = s.centers[s.from] + d;
        let mut to = s.from;
        for i in s.from + 1..s.items.len() {
            if dc > s.centers[i] {
                to = i;
            }
        }
        for i in 0..s.from {
            if dc < s.centers[i] {
                to = i;
                break;
            }
        }
        s.to = to;
        for (i, k) in s.items.iter().enumerate() {
            if i == s.from {
                continue;
            }
            let shift = if i > s.from && i <= to { -s.step } else if i < s.from && i >= to { s.step } else { 0.0 };
            let _ = k.style().set_property("transform", &format!("{axis}({shift}px)"));
        }
    }) as Box<dyn FnMut(_)>);
    let _ = document.add_event_listener_with_callback("pointermove", mv.as_ref().unchecked_ref());
    mv.forget();

    let (st, jd, s2) = (sort.clone(), just_dragged.clone(), shared.clone());
    let up = Closure::wrap(Box::new(move |_e: web_sys::PointerEvent| {
        let Some(s) = st.borrow_mut().take() else { return };
        if !s.active {
            return;
        }
        for k in &s.items {
            let style = k.style();
            for p in ["transform", "transition", "z-index", "cursor", "user-select"] {
                let _ = style.remove_property(p);
            }
        }
        jd.set(now());
        if s.from != s.to {
            dispatch_pointer(&s2, &s.handler, s.from as f64, s.to as f64);
        }
    }) as Box<dyn FnMut(_)>);
    let _ = document.add_event_listener_with_callback("pointerup", up.as_ref().unchecked_ref());
    up.forget();

    // The click that ends a drag is not a click.
    let jd = just_dragged.clone();
    let swallow = Closure::wrap(Box::new(move |e: web_sys::Event| {
        if now() - jd.get() < 60.0 {
            e.stop_propagation();
            e.prevent_default();
        }
    }) as Box<dyn FnMut(_)>);
    let opts = web_sys::AddEventListenerOptions::new();
    opts.set_capture(true);
    let _ = container.add_event_listener_with_callback_and_add_event_listener_options("click", swallow.as_ref().unchecked_ref(), &opts);
    swallow.forget();
}
