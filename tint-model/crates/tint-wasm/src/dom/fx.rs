// DOM side of `enter` / `exit` / `view` / `layout` and drag `bounds` (the language side
// is tint-runtime's style/fx.rs: it leaves `--tint-fx-*` custom properties on the node).
//
//   enter  a new element starts from the `enter` style and animates to its own.
//   exit   a removed element is kept (a ghost) while it animates to the `exit` style, then goes.
//   layout an element whose box moved or resized in a re-render glides from where it was (FLIP).
//   view   the `view` style is on while the element is in the viewport.
//
// The animation is the node's own `transition` / `spring` (200 ms ease when it has none),
// layout takes its duration and easing from it as well (300 ms when it has none).

thread_local! {
    /// Counts renders: elements entering in the same one are staggered together.
    static RENDER_SEQ: Cell<u32> = const { Cell::new(0) };
    static HAS_LAYOUT: Cell<bool> = const { Cell::new(false) };
    /// Ghosts still animating out (see `start_exit`).
    static EXITING: Cell<usize> = const { Cell::new(0) };
    static VIEW_OBSERVER: RefCell<Option<web_sys::IntersectionObserver>> = const { RefCell::new(None) };
}

const FX_DEFAULT_MS: f64 = 200.0;

fn fx_prop<'a>(node: &'a UiRenderNode, name: &str) -> Option<&'a str> {
    node.style.iter().rev().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
}

fn has_fx(node: &UiRenderNode) -> bool {
    node.style.iter().any(|(k, _)| k.starts_with("--tint-fx-"))
}

fn fx_decode(text: &str) -> Vec<(String, String)> {
    text.split('|')
        .filter_map(|p| p.split_once(':'))
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

fn exit_style(node: &UiRenderNode) -> Option<Vec<(String, String)>> {
    fx_prop(node, "--tint-fx-exit").map(fx_decode)
}

/// Runs `f` after `frames` animation frames.
fn after_frames(frames: u32, f: Box<dyn FnOnce()>) {
    let Some(window) = web_sys::window() else {
        f();
        return;
    };
    let cb = Closure::once_into_js(move || {
        if frames <= 1 {
            f();
        } else {
            after_frames(frames - 1, f);
        }
    });
    let _ = window.request_animation_frame(cb.unchecked_ref());
}

fn after_ms(ms: f64, f: Box<dyn FnOnce()>) {
    let Some(window) = web_sys::window() else {
        f();
        return;
    };
    let cb = Closure::once_into_js(move || f());
    let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(cb.unchecked_ref(), ms.max(0.0) as i32);
}

fn parse_css_time(text: &str) -> f64 {
    let t = text.trim();
    if let Some(ms) = t.strip_suffix("ms") {
        ms.trim().parse().unwrap_or(0.0)
    } else if let Some(s) = t.strip_suffix('s') {
        s.trim().parse::<f64>().unwrap_or(0.0) * 1000.0
    } else {
        0.0
    }
}

/// First item of a comma separated CSS list, keeping a `linear(...)` / `cubic-bezier(...)` whole.
fn first_css_item(list: &str) -> &str {
    let mut depth = 0;
    for (i, c) in list.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => return list[..i].trim(),
            _ => {}
        }
    }
    list.trim()
}

fn computed(el: &Element, property: &str) -> String {
    web_sys::window()
        .and_then(|w| w.get_computed_style(el).ok().flatten())
        .and_then(|s| s.get_property_value(property).ok())
        .unwrap_or_default()
}

/// Longest `transition-delay + transition-duration` of the element, in ms.
fn transition_ms(el: &Element) -> f64 {
    let longest = |property: &str| {
        computed(el, property)
            .split(',')
            .map(parse_css_time)
            .fold(0.0f64, f64::max)
    };
    longest("transition-duration") + longest("transition-delay")
}

/// Called when an element is built or its style changed.
fn apply_fx(el: &Element, node: &UiRenderNode) {
    if !has_fx(node) {
        if el.has_attribute("data-tint-fx") {
            let _ = el.remove_attribute("data-tint-fx");
            let _ = el.remove_attribute("data-tint-layout");
            let _ = el.remove_attribute("data-tint-layout-id");
            let _ = el.remove_attribute("data-tint-scroll");
            let _ = el.remove_attribute("data-tint-stagger");
            unobserve_view(el);
        }
        return;
    }
    let _ = el.set_attribute("data-tint-fx", "1");
    match fx_prop(node, "--tint-fx-stagger") {
        Some(ms) => {
            let _ = el.set_attribute("data-tint-stagger", ms);
        }
        None => {
            let _ = el.remove_attribute("data-tint-stagger");
        }
    }
    match fx_prop(node, "--tint-fx-layout-id") {
        Some(id) => {
            let _ = el.set_attribute("data-tint-layout-id", id);
            HAS_LAYOUT.with(|f| f.set(true));
        }
        None => {
            let _ = el.remove_attribute("data-tint-layout-id");
        }
    }
    match fx_prop(node, "--tint-fx-layout").or(fx_prop(node, "--tint-fx-layout-id").map(|_| "all")) {
        Some(mode) => {
            let _ = el.set_attribute("data-tint-layout", mode);
            HAS_LAYOUT.with(|f| f.set(true));
        }
        None => {
            let _ = el.remove_attribute("data-tint-layout");
        }
    }
    match (fx_prop(node, "--tint-fx-scroll-from"), fx_prop(node, "--tint-fx-scroll-to")) {
        (Some(from), Some(to)) => {
            let _ = el.set_attribute("data-tint-scroll-from", from);
            let _ = el.set_attribute("data-tint-scroll-to", to);
            let _ = el.set_attribute("data-tint-scroll", fx_prop(node, "--tint-fx-scroll-mode").unwrap_or("element"));
            scroll_bind();
            scroll_update_one(el);
        }
        _ => {
            let _ = el.remove_attribute("data-tint-scroll");
        }
    }
    if fx_prop(node, "--tint-fx-view").is_some() {
        observe_view(el);
    } else {
        unobserve_view(el);
    }
}

/// The user asked the system for less motion (`prefers-reduced-motion`): enter and exit keep their
/// fades but lose their movement (`translate`, `scale`, `rotate`), and layout glides are skipped.
fn calm(props: Vec<(String, String)>) -> Vec<(String, String)> {
    if !reduced_motion() {
        return props;
    }
    props
        .into_iter()
        .filter(|(k, _)| !(k.starts_with("translate") || k.starts_with("scale") || k.starts_with("rotate") || k == "transform"))
        .collect()
}

fn reduced_motion() -> bool {
    web_sys::window()
        .and_then(|w| w.match_media("(prefers-reduced-motion: reduce)").ok().flatten())
        .is_some_and(|m| m.matches())
}

// ---- enter ----

/// The style the element starts from (`base` plus the `enter` style) is written now;
/// two frames later, once it is in the DOM and has been drawn, the real one replaces it
/// and the transition runs.
fn start_enter(el: &Element, node: &UiRenderNode, base: &[(String, String)]) {
    let Some(text) = fx_prop(node, "--tint-fx-enter") else { return };
    let mut from = base.to_vec();
    from.extend(calm(fx_decode(text)));
    let from_css = style_to_css_text(&from);
    let has_transition = base.iter().any(|(k, _)| k == "transition");
    let mut to = base.to_vec();
    if !has_transition {
        to.push(("transition".to_string(), format!("all {}ms ease", FX_DEFAULT_MS)));
    }
    let to_css = style_to_css_text(&to);
    let base_css = style_to_css_text(base);
    let _ = el.set_attribute("style", &from_css);
    let batch = RENDER_SEQ.with(|s| s.get()).to_string();
    let _ = el.set_attribute("data-tint-entering", &batch);
    let el = el.clone();
    after_frames(
        2,
        Box::new(move || {
            // Only if nothing re-rendered the node in between.
            if el.get_attribute("style").as_deref() != Some(from_css.as_str()) {
                return;
            }
            let delay = stagger_delay(&el, &batch);
            let _ = el.remove_attribute("data-tint-entering");
            let to_css = if delay > 0.0 { format!("{to_css} transition-delay: {delay}ms;") } else { to_css };
            let _ = el.set_attribute("style", &to_css);
            if !has_transition {
                after_ms(
                    FX_DEFAULT_MS + 50.0,
                    Box::new(move || {
                        if el.get_attribute("style").as_deref() == Some(to_css.as_str()) {
                            let _ = el.set_attribute("style", &base_css);
                        }
                    }),
                );
            }
        }),
    );
}

// ---- exit ----

/// Keeps a removed element in the DOM while it animates to `props`, then removes it.
/// Nothing in the tree refers to it any more; it cannot be clicked meanwhile.
fn start_exit(el: &Element, props: Vec<(String, String)>) {
    let props = calm(props);
    let Some(html) = el.dyn_ref::<web_sys::HtmlElement>() else {
        el.remove();
        return;
    };
    let mut ms = transition_ms(el);
    let style = html.style();
    if ms <= 0.0 {
        ms = FX_DEFAULT_MS;
        let _ = style.set_property("transition", &format!("all {}ms ease", ms));
    }
    let _ = el.set_attribute("data-tint-exiting", "1");
    let _ = el.set_attribute("aria-hidden", "true");
    let _ = style.set_property("pointer-events", "none");
    for (k, v) in &props {
        let _ = style.set_property(k, v);
    }
    EXITING.with(|n| n.set(n.get() + 1));
    let el = el.clone();
    after_ms(
        ms + 50.0,
        Box::new(move || {
            // Siblings take the freed room with a glide if they have `layout`.
            let root = document().ok().and_then(|d| d.body());
            let before = root.as_ref().map(|r| flip_measure(r)).unwrap_or_default();
            el.remove();
            EXITING.with(|n| n.set(n.get().saturating_sub(1)));
            flip_play(before);
        }),
    );
}

/// Ghosts that are direct children of `container`.
fn exiting_children(container: &Element) -> usize {
    if EXITING.with(|n| n.get()) == 0 {
        return 0;
    }
    container
        .query_selector_all(":scope > [data-tint-exiting]")
        .map(|l| l.length() as usize)
        .unwrap_or(0)
}

// ---- layout (FLIP) ----

struct FlipBox {
    el: Element,
    mode: String,
    id: Option<String>,
    cx: f64,
    cy: f64,
    w: f64,
    h: f64,
}

fn js_get(target: &JsValue, key: &str) -> JsValue {
    js_sys::Reflect::get(target, &JsValue::from_str(key)).unwrap_or(JsValue::UNDEFINED)
}

fn js_set(target: &JsValue, key: &str, value: &JsValue) {
    let _ = js_sys::Reflect::set(target, &JsValue::from_str(key), value);
}

/// Boxes of every `layout` element under `root`, before a re-render. A glide still
/// running is cancelled first so the real position is measured.
fn flip_measure(root: &Element) -> Vec<FlipBox> {
    if !HAS_LAYOUT.with(|f| f.get()) || reduced_motion() {
        return Vec::new();
    }
    let Ok(list) = root.query_selector_all("[data-tint-layout],[data-tint-layout-id]") else { return Vec::new() };
    let mut els = Vec::with_capacity(list.length() as usize);
    for i in 0..list.length() {
        if let Some(el) = list.item(i).and_then(|n| n.dyn_into::<Element>().ok()) {
            let running = js_get(el.as_ref(), "__tintFlip");
            if let Ok(cancel) = js_get(&running, "cancel").dyn_into::<js_sys::Function>() {
                let _ = cancel.call0(&running);
            }
            js_set(el.as_ref(), "__tintFlip", &JsValue::UNDEFINED);
            els.push(el);
        }
    }
    els.into_iter()
        .map(|el| {
            let r = el.get_bounding_client_rect();
            FlipBox {
                mode: el.get_attribute("data-tint-layout").unwrap_or_else(|| "all".to_string()),
                id: el.get_attribute("data-tint-layout-id"),
                cx: r.left() + r.width() / 2.0,
                cy: r.top() + r.height() / 2.0,
                w: r.width(),
                h: r.height(),
                el,
            }
        })
        .collect()
}

/// After the re-render: each element that moved or resized glides from its old box to
/// the new one (the `translate` and `scale` properties are animated, so its own `transform` stays).
fn flip_play(before: Vec<FlipBox>) {
    let live = |b: &FlipBox| b.el.is_connected() && !b.el.has_attribute("data-tint-exiting");
    let kept: Vec<Element> = before.iter().filter(|b| live(b)).map(|b| b.el.clone()).collect();
    for b in &before {
        if live(b) {
            flip_animate(&b.el, b);
            continue;
        }
        // The element is gone (or leaving): another one with the same `layout-id` takes over its box.
        let Some(id) = &b.id else { continue };
        let selector = format!("[data-tint-layout-id=\"{}\"]:not([data-tint-exiting])", id.replace('\\', "\\\\").replace('"', "\\\""));
        let Some(next) = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.query_selector(&selector).ok().flatten())
        else {
            continue;
        };
        if kept.contains(&next) {
            continue;
        }
        flip_animate(&next, b);
    }
}

/// `el` glides from the box `b` to where it is now.
fn flip_animate(el: &Element, b: &FlipBox) {
    let r = el.get_bounding_client_rect();
    let (dx, dy) = (b.cx - (r.left() + r.width() / 2.0), b.cy - (r.top() + r.height() / 2.0));
    let (mut sx, mut sy) = (1.0, 1.0);
    if b.mode != "position" && r.width() > 0.0 && r.height() > 0.0 {
        sx = b.w / r.width();
        sy = b.h / r.height();
    }
    if b.mode == "size" {
        // size only: keep the element where it is, grow/shrink around its centre
        if (sx - 1.0).abs() < 0.005 && (sy - 1.0).abs() < 0.005 {
            return;
        }
    } else if dx.abs() < 0.5 && dy.abs() < 0.5 && (sx - 1.0).abs() < 0.005 && (sy - 1.0).abs() < 0.005 {
        return;
    }
    // Property-indexed keyframes: from the old box to "none" (the element's own place).
    let frame = js_sys::Object::new();
    let from_to = |from: String, to: &str| js_sys::Array::of2(&JsValue::from_str(&from), &JsValue::from_str(to));
    if b.mode != "size" {
        js_set(&frame, "translate", &from_to(format!("{}px {}px", dx, dy), "none"));
    }
    if b.mode != "position" {
        js_set(&frame, "scale", &from_to(format!("{} {}", sx, sy), "none"));
    }
    let mut duration = parse_css_time(first_css_item(&computed(el, "transition-duration")));
    let mut easing = first_css_item(&computed(el, "transition-timing-function")).to_string();
    if duration <= 0.0 || easing.is_empty() {
        duration = 300.0;
        easing = "cubic-bezier(0.2, 0.8, 0.2, 1)".to_string();
    }
    let options = js_sys::Object::new();
    js_set(&options, "duration", &JsValue::from_f64(duration));
    js_set(&options, "easing", &JsValue::from_str(&easing));
    if let Ok(animate) = js_get(el.as_ref(), "animate").dyn_into::<js_sys::Function>() {
        if let Ok(animation) = animate.call2(el.as_ref(), &frame, &options) {
            js_set(el.as_ref(), "__tintFlip", &animation);
        }
    }
}

// ---- view ----

fn view_observer() -> Option<web_sys::IntersectionObserver> {
    VIEW_OBSERVER.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.is_none() {
            let cb = Closure::wrap(Box::new(move |entries: js_sys::Array, observer: web_sys::IntersectionObserver| {
                for entry in entries.iter() {
                    if let Ok(entry) = entry.dyn_into::<web_sys::IntersectionObserverEntry>() {
                        // Scrolled past (above the window) or reduced motion: the element counts as seen.
                        let inside = entry.is_intersecting() || { let r = entry.bounding_client_rect(); r.height() > 0.0 && r.bottom() <= 0.0 };
                        set_in_view(&entry.target(), inside, &observer);
                    }
                }
                // An observer reports changes only: nodes a jump in scroll position skipped over
                // (still `out`, now above the window) are caught up here.
                if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
                    if let Ok(list) = doc.query_selector_all("[data-tint-view=out]") {
                        for i in 0..list.length() {
                            if let Some(el) = list.item(i).and_then(|n| n.dyn_into::<Element>().ok()) {
                                let r = el.get_bounding_client_rect();
                                if r.height() > 0.0 && r.bottom() <= 0.0 {
                                    set_in_view(&el, true, &observer);
                                }
                            }
                        }
                    }
                }
            }) as Box<dyn FnMut(js_sys::Array, web_sys::IntersectionObserver)>);
            let init = web_sys::IntersectionObserverInit::new();
            init.set_threshold(&JsValue::from_f64(0.15));
            let observer =
                web_sys::IntersectionObserver::new_with_options(cb.as_ref().unchecked_ref(), &init).ok();
            cb.forget();
            *slot = observer;
        }
        slot.clone()
    })
}

fn observe_view(el: &Element) {
    if el.has_attribute("data-tint-view") {
        return;
    }
    if let Some(observer) = view_observer() {
        let _ = el.set_attribute("data-tint-view", "out");
        observer.observe(el);
    }
}

fn unobserve_view(el: &Element) {
    if !el.has_attribute("data-tint-view") {
        return;
    }
    if let Some(observer) = view_observer() {
        observer.unobserve(el);
    }
    let _ = el.remove_attribute("data-tint-view");
    let _ = el.remove_attribute("data-tint-view-prev");
}

/// Switches the `view` style of `el` on or off. What it overwrites is remembered in
/// `data-tint-view-prev`; a patch while in view keeps the style on (`apply_style_props`).
fn set_in_view(el: &Element, inside: bool, observer: &web_sys::IntersectionObserver) {
    let was_in = el.get_attribute("data-tint-view").as_deref() == Some("in");
    if inside == was_in {
        return;
    }
    let Some(html) = el.dyn_ref::<web_sys::HtmlElement>() else { return };
    let style = html.style();
    if inside {
        let props = fx_decode(&style.get_property_value("--tint-fx-view").unwrap_or_default());
        let prev: Vec<String> = props
            .iter()
            .map(|(k, _)| format!("{}\u{1}{}", k, style.get_property_value(k).unwrap_or_default()))
            .collect();
        let _ = el.set_attribute("data-tint-view-prev", &prev.join("\u{2}"));
        for (k, v) in &props {
            let _ = style.set_property(k, v);
        }
        let _ = el.set_attribute("data-tint-view", "in");
        if style.get_property_value("--tint-fx-view-once").unwrap_or_default() == "1" {
            observer.unobserve(el);
        }
    } else {
        let prev = el.get_attribute("data-tint-view-prev").unwrap_or_default();
        for pair in prev.split('\u{2}').filter(|p| !p.is_empty()) {
            if let Some((k, v)) = pair.split_once('\u{1}') {
                if v.is_empty() {
                    let _ = style.remove_property(k);
                } else {
                    let _ = style.set_property(k, v);
                }
            }
        }
        let _ = el.set_attribute("data-tint-view", "out");
    }
}

// ---- drag bounds ----

/// `left,top,right,bottom` in px from the start position, `x` = free.
fn drag_bounds(el: &Element) -> [Option<f64>; 4] {
    let text = el.get_attribute("data-tint-drag-bounds").unwrap_or_default();
    let mut out = [None; 4];
    for (slot, part) in out.iter_mut().zip(text.split(',')) {
        *slot = part.parse().ok();
    }
    out
}

/// Beyond a bound the offset follows the pointer by `elastic` of the overshoot.
fn clamp_axis(v: f64, lo: Option<f64>, hi: Option<f64>, elastic: f64) -> f64 {
    match (lo, hi) {
        (Some(lo), _) if v < lo => lo + (v - lo) * elastic,
        (_, Some(hi)) if v > hi => hi + (v - hi) * elastic,
        _ => v,
    }
}

// ---- scroll-linked styles ----

thread_local! {
    static SCROLL_BOUND: Cell<bool> = const { Cell::new(false) };
    static SCROLL_PENDING: Cell<bool> = const { Cell::new(false) };
}

/// Splits `translateY(40px) 3 #fff` into its numbers and the text around them.
fn number_split(text: &str) -> (Vec<String>, Vec<f64>) {
    let mut skeleton = vec![String::new()];
    let mut numbers = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let starts = c.is_ascii_digit()
            || (c == '.' && chars.get(i + 1).is_some_and(|d| d.is_ascii_digit()))
            || (c == '-' && chars.get(i + 1).is_some_and(|d| d.is_ascii_digit() || *d == '.'));
        let after_word = i > 0 && (chars[i - 1].is_ascii_alphabetic() || chars[i - 1] == '#') && c != '-';
        if starts && !after_word {
            let mut j = i + 1;
            while j < chars.len() && (chars[j].is_ascii_digit() || chars[j] == '.') {
                j += 1;
            }
            let n: String = chars[i..j].iter().collect();
            numbers.push(n.parse().unwrap_or(0.0));
            skeleton.push(String::new());
            i = j;
        } else {
            skeleton.last_mut().unwrap().push(c);
            i += 1;
        }
    }
    (skeleton, numbers)
}

/// `from` and `to` at `t` (0..1): every number is interpolated when the two have the same shape.
fn lerp_css(from: &str, to: &str, t: f64) -> String {
    let (sa, na) = number_split(from);
    let (sb, nb) = number_split(to);
    if sa != sb || na.len() != nb.len() || na.is_empty() {
        return if t < 0.5 { from.to_string() } else { to.to_string() };
    }
    let mut out = String::new();
    for (i, text) in sa.iter().enumerate() {
        out.push_str(text);
        if i < na.len() {
            let v = na[i] + (nb[i] - na[i]) * t;
            out.push_str(&format!("{}", (v * 1000.0).round() / 1000.0));
        }
    }
    out
}

fn scroll_progress(el: &Element, mode: &str) -> f64 {
    let Some(window) = web_sys::window() else { return 0.0 };
    let vh = window.inner_height().ok().and_then(|v| v.as_f64()).unwrap_or(0.0);
    if mode == "page" {
        let Some(root) = window.document().and_then(|d| d.document_element()) else { return 0.0 };
        let max = (root.scroll_height() as f64 - vh).max(1.0);
        let y = window.scroll_y().unwrap_or(0.0);
        return (y / max).clamp(0.0, 1.0);
    }
    let r = el.get_bounding_client_rect();
    ((vh - r.top()) / (vh + r.height()).max(1.0)).clamp(0.0, 1.0)
}

fn scroll_update_one(el: &Element) {
    let (Some(from), Some(to), Some(html)) = (
        el.get_attribute("data-tint-scroll-from"),
        el.get_attribute("data-tint-scroll-to"),
        el.dyn_ref::<web_sys::HtmlElement>(),
    ) else {
        return;
    };
    let t = scroll_progress(el, &el.get_attribute("data-tint-scroll").unwrap_or_default());
    let to = fx_decode(&to);
    for (prop, a) in fx_decode(&from) {
        if let Some((_, b)) = to.iter().find(|(k, _)| *k == prop) {
            let _ = html.style().set_property(&prop, &lerp_css(&a, b, t));
        }
    }
}

fn scroll_update_all() {
    let Some(document) = web_sys::window().and_then(|w| w.document()) else { return };
    let Ok(list) = document.query_selector_all("[data-tint-scroll]") else { return };
    for i in 0..list.length() {
        if let Some(el) = list.item(i).and_then(|n| n.dyn_into::<Element>().ok()) {
            scroll_update_one(&el);
        }
    }
}

/// One scroll/resize listener for every scroll-linked node (once per frame).
fn scroll_bind() {
    if SCROLL_BOUND.with(|b| b.replace(true)) {
        return;
    }
    let Some(window) = web_sys::window() else { return };
    let cb = Closure::wrap(Box::new(move |_e: web_sys::Event| {
        if SCROLL_PENDING.with(|p| p.replace(true)) {
            return;
        }
        after_frames(1, Box::new(|| {
            SCROLL_PENDING.with(|p| p.set(false));
            scroll_update_all();
        }));
    }) as Box<dyn FnMut(_)>);
    let opts = web_sys::AddEventListenerOptions::new();
    opts.set_capture(true);
    opts.set_passive(true);
    let _ = window.add_event_listener_with_callback_and_add_event_listener_options("scroll", cb.as_ref().unchecked_ref(), &opts);
    let _ = window.add_event_listener_with_callback("resize", cb.as_ref().unchecked_ref());
    cb.forget();
}

/// How long an entering node waits: its position among the siblings entering in the same render
/// times the parent's `stagger` (ms).
fn stagger_delay(el: &Element, batch: &str) -> f64 {
    let Some(parent) = el.parent_element() else { return 0.0 };
    let Some(step) = parent.get_attribute("data-tint-stagger").and_then(|v| v.parse::<f64>().ok()) else { return 0.0 };
    let mut index = 0.0;
    let mut cur = el.previous_element_sibling();
    while let Some(sibling) = cur {
        if sibling.get_attribute("data-tint-entering").as_deref() == Some(batch) {
            index += 1.0;
        }
        cur = sibling.previous_element_sibling();
    }
    index * step
}
