fn build(code: &str) -> tint_runtime::ui::tree::UiTree {
    let ui_fn = parse_ui_fn(code, "App");
    let mut builder = UiBuilder::new();
    let mut vm = TintVM::new();
    builder.build_root(&ui_fn.body, &mut vm);
    builder.finish()
}

fn node<'a>(tree: &'a tint_runtime::ui::tree::UiTree, tag: &str) -> &'a tint_runtime::ui::tree::UiElement {
    tree.nodes.iter().find(|n| n.tag == tag).expect("node")
}

#[test]
fn new_css_properties_resolve() {
    let tree = build(
        r#"
ui fn App() {
    Box {
        layout::{ flex-basis::200, grid-area::"a", scroll-snap-type::"x mandatory" }
        paint::{ object-fit::cover, clip-path::"circle(40%)", mix-blend-mode::multiply, text-overflow::ellipsis }
        line-clamp::3
        rotate::15
        skew.x::10
        translate.y::4
        scale::1.1
        ring::{2, #6c5ce7}
        shadow::"0 1px 2px #000"
    }
}
"#,
    );
    let b = node(&tree, "Box");
    let has = |k: &str, v: &str| b.style.contains(&(k.to_string(), v.to_string()));
    assert!(has("flex-basis", "200px"));
    assert!(has("grid-area", "a"));
    assert!(has("object-fit", "cover"));
    assert!(has("clip-path", "circle(40%)"));
    assert!(has("mix-blend-mode", "multiply"));
    assert!(has("-webkit-line-clamp", "3"));
    // typed transforms combine instead of overriding each other
    assert!(has("transform", "rotate(15deg) skewX(10deg) translateY(4px) scale(1.1)"), "{:?}", b.style);
    // ring adds to an existing shadow
    assert!(has("box-shadow", "0 1px 2px #000") || b.style.iter().any(|(k, v)| k == "box-shadow" && v.contains("0 0 0 2px #6c5ce7")));
}

#[test]
fn state_blocks_become_selector_styles() {
    let tree = build(
        r#"
ui fn App() {
    Input {
        placeholder||"Name"
        disabled||{false}
        focus::{ ring::{2, #6c5ce7}, background::#fff }
        disabled::{ opacity::0.5 }
        placeholder::{ color::#999 }
        before::{ content::"*", color::red }
        tap::{ scale::0.95 }
    }
}
"#,
    );
    let n = node(&tree, "Input");
    let names: Vec<_> = n.breakpoints.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(names, [":focus", ":disabled", "::placeholder", "::before", ":active"]);
    let focus = &n.breakpoints[0].1;
    assert!(focus.contains(&("box-shadow".to_string(), "0 0 0 2px #6c5ce7".to_string())));
    let before = &n.breakpoints[3].1;
    assert!(before.contains(&("content".to_string(), "\"*\"".to_string())));
    assert!(n.style.iter().all(|(k, _)| k != "opacity"));
    // `disabled||{false}` is left out, `placeholder` kept
    assert_eq!(n.attrs, vec![("placeholder".to_string(), "Name".to_string())]);
}

#[test]
fn spring_becomes_a_linear_transition_and_drag_flags() {
    let tree = build(
        r#"
ui fn App() {
    Box {
        spring::{ stiffness::300, damping::12, props::"transform, opacity" }
        drag::{ x, back }
    }
}
"#,
    );
    let b = node(&tree, "Box");
    let transition = &b.style.iter().find(|(k, _)| k == "transition").expect("transition").1;
    assert!(transition.starts_with("transform ") && transition.contains(", opacity ") && transition.contains("linear(0,"), "{transition}");
    // an under-damped spring overshoots 1 before settling at 1
    let first = transition.split(", opacity").next().unwrap();
    let points: Vec<f64> = first
        .split("linear(")
        .nth(1)
        .unwrap()
        .trim_end_matches(')')
        .split(", ")
        .map(|p| p.parse().unwrap())
        .collect();
    assert!(points.iter().any(|p| *p > 1.0));
    assert_eq!(*points.last().unwrap(), 1.0);
    assert!(b.style.contains(&("--tint-drag".to_string(), "x".to_string())));
    assert!(b.style.contains(&("--tint-drag-return".to_string(), "1".to_string())));
}

#[test]
fn fluid_font_size_ranges_and_css_math() {
    let tree = build(
        r#"
ui fn App() {
    Text { text::{16..44, bold} "a" }
    Box { size::"clamp(1rem, 3vw, 2rem)" }
    Row { size::18..30 }
    Column { text::{20, white} }
}
"#,
    );
    let size = |tag: &str| {
        node(&tree, tag).style.iter().find(|(k, _)| k == "font-size").map(|(_, v)| v.clone())
    };
    // 16px at 360 wide -> 44px at 1280: slope 28/920 = 3.043vw, intercept 16 - 360*28/920 = 5.043px
    assert_eq!(size("Text").unwrap(), "clamp(16px, calc(5.043px + 3.043vw), 44px)");
    assert_eq!(size("Box").unwrap(), "clamp(1rem, 3vw, 2rem)");
    assert!(size("Row").unwrap().starts_with("clamp(18px,"));
    assert_eq!(size("Column").unwrap(), "20px");
    assert!(node(&tree, "Text").style.contains(&("font-weight".to_string(), "bold".to_string())));
}

#[test]
fn fluid_lengths_in_spacing_and_sizes() {
    let tree = build(
        r#"
ui fn App() {
    Column { layout::{ padding::{16..48}, gap::8..24, margin::{x::0..32, top::12} } paint::{ radius::8..20 } }
    Row { layout::{ padding::{x::10..30, y::6}, width::100..200, margin::"calc(1px + 1vw)" } }
}
"#,
    );
    let get = |tag: &str, k: &str| node(&tree, tag).style.iter().find(|(p, _)| p == k).map(|(_, v)| v.clone());
    assert_eq!(get("Column", "padding").unwrap(), "clamp(16px, calc(3.478px + 3.478vw), 48px)");
    assert!(get("Column", "gap").unwrap().starts_with("clamp(8px,"));
    assert!(get("Column", "margin-left").unwrap().starts_with("clamp(0px,"));
    assert_eq!(get("Column", "margin-top").unwrap(), "12px");
    assert!(get("Column", "border-radius").unwrap().starts_with("clamp(8px,"));
    assert!(get("Row", "padding-left").unwrap().starts_with("clamp(10px,"));
    assert_eq!(get("Row", "padding-top").unwrap(), "6px");
    assert!(get("Row", "width").unwrap().starts_with("clamp(100px,"));
    assert_eq!(get("Row", "margin").unwrap(), "calc(1px + 1vw)");
}

#[test]
fn enter_exit_view_layout_and_drag_bounds_resolve_to_host_flags() {
    let tree = build(
        r#"
ui fn App() {
    Box {
        motion::{
            layout::all,
            enter::{ opacity::0, scale::0.8 },
            exit::{ opacity::0 },
            view::{ once, opacity::1 }
        }
        drag::{ x, bounds::{ left::-40, right::80 }, elastic::0.2 }
    }
}
"#,
    );
    let b = node(&tree, "Box");
    let get = |k: &str| b.style.iter().find(|(key, _)| key == k).map(|(_, v)| v.as_str());
    assert_eq!(get("--tint-fx-layout"), Some("all"));
    assert_eq!(get("--tint-fx-enter"), Some("opacity:0|transform:scale(0.8)"));
    assert_eq!(get("--tint-fx-exit"), Some("opacity:0"));
    assert_eq!(get("--tint-fx-view"), Some("opacity:1"));
    assert_eq!(get("--tint-fx-view-once"), Some("1"));
    assert_eq!(get("--tint-drag-bounds"), Some("-40,x,80,x"));
    assert_eq!(get("--tint-drag-elastic"), Some("0.2"));
}
