// `spring::{ stiffness::170, damping::26, mass::1, props::"transform, opacity" }`
//
// A damped spring as a CSS transition. The spring equation is integrated here
// and sampled into a `linear(...)` easing, so the browser runs it with no
// JavaScript per frame: `transition: transform 640ms linear(0, .03, .11, ...)`.
// Overshoot (values above 1) is what makes it feel like a spring.

fn spring_samples(stiffness: f64, damping: f64, mass: f64) -> (Vec<f64>, f64) {
    const STEP: f64 = 1.0 / 480.0;
    const EVERY: usize = 8; // sample at 60 Hz
    const MAX_SECONDS: f64 = 4.0;
    let (mut x, mut v) = (0.0f64, 0.0f64);
    let accel = |x: f64, v: f64| (stiffness * (1.0 - x) - damping * v) / mass;
    let mut samples = vec![0.0];
    let mut settled_at: Option<usize> = None;
    let steps = (MAX_SECONDS / STEP) as usize;
    for i in 1..=steps {
        // RK4
        let k1x = v;
        let k1v = accel(x, v);
        let k2x = v + 0.5 * STEP * k1v;
        let k2v = accel(x + 0.5 * STEP * k1x, v + 0.5 * STEP * k1v);
        let k3x = v + 0.5 * STEP * k2v;
        let k3v = accel(x + 0.5 * STEP * k2x, v + 0.5 * STEP * k2v);
        let k4x = v + STEP * k3v;
        let k4v = accel(x + STEP * k3x, v + STEP * k3v);
        x += STEP / 6.0 * (k1x + 2.0 * k2x + 2.0 * k3x + k4x);
        v += STEP / 6.0 * (k1v + 2.0 * k2v + 2.0 * k3v + k4v);
        if i % EVERY == 0 {
            samples.push(x);
            if (x - 1.0).abs() < 0.002 && v.abs() < 0.01 {
                settled_at = Some(samples.len() - 1);
                break;
            }
        }
    }
    let last = settled_at.unwrap_or(samples.len() - 1);
    samples.truncate(last + 1);
    // Land exactly on 1 so the end state is the real target.
    if let Some(end) = samples.last_mut() {
        *end = 1.0;
    }
    let duration_ms = last as f64 * EVERY as f64 * STEP * 1000.0;
    (samples, duration_ms)
}

fn apply_spring(value: &UiModifierValue, out: &mut StyleList) {
    let UiModifierValue::Tuple(items) = value else { return };
    let (mut stiffness, mut damping, mut mass) = (170.0, 26.0, 1.0);
    let mut props = "all".to_string();
    for item in items {
        if let UiModifierValue::MiniMod { key, value } = item {
            match (key.first().map(String::as_str), value.as_ref()) {
                (Some("stiffness"), UiModifierValue::Number(n)) if *n > 0.0 => stiffness = *n,
                (Some("damping"), UiModifierValue::Number(n)) if *n >= 0.0 => damping = *n,
                (Some("mass"), UiModifierValue::Number(n)) if *n > 0.0 => mass = *n,
                (Some("props"), UiModifierValue::String(s) | UiModifierValue::Ident(s)) => props = s.clone(),
                _ => {}
            }
        }
    }
    let (samples, duration) = spring_samples(stiffness, damping, mass);
    let points: Vec<String> = samples
        .iter()
        .map(|s| {
            let text = format!("{:.3}", s);
            let text = text.trim_end_matches('0').trim_end_matches('.');
            if text.is_empty() || text == "-0" { "0".to_string() } else { text.to_string() }
        })
        .collect();
    let transition = props
        .split(',')
        .map(|p| format!("{} {}ms linear({})", p.trim(), duration.round() as i64, points.join(", ")))
        .collect::<Vec<_>>()
        .join(", ");
    out.push(("transition".to_string(), transition));
}

// `drag::both`, `drag::x`, `drag::{ y, back }`: the node follows
// the pointer (visual only, no Tint state per move). With `back` it
// goes back to its place on release -- animated if the node has a `spring`
// (or any transition) on `translate`. Resolved into custom properties the DOM
// host reads when it binds the gesture; combine with `pointer_up||` to react.
fn apply_drag(value: &UiModifierValue, out: &mut StyleList) {
    let mut axis = "both".to_string();
    let mut back = false;
    match value {
        UiModifierValue::Ident(s) | UiModifierValue::String(s) => axis = s.clone(),
        UiModifierValue::Tuple(items) => {
            for item in items {
                if let UiModifierValue::MiniMod { key, value } = item {
                    match (key.first().map(String::as_str), value.as_ref()) {
                        (Some("axis"), UiModifierValue::Ident(s) | UiModifierValue::String(s)) => axis = s.clone(),
                        _ => {}
                    }
                } else if let UiModifierValue::Ident(word) = item {
                    match word.as_str() {
                        "back" => back = true,
                        "x" | "y" | "both" => axis = word.clone(),
                        _ => {}
                    }
                }
            }
        }
        _ => return,
    }
    if !matches!(axis.as_str(), "x" | "y" | "both") {
        return;
    }
    out.push(("--tint-drag".to_string(), axis));
    if back {
        out.push(("--tint-drag-return".to_string(), "1".to_string()));
    }
    out.push(("touch-action".to_string(), "none".to_string()));
    out.push(("user-select".to_string(), "none".to_string()));
    out.push(("cursor".to_string(), "grab".to_string()));
}
