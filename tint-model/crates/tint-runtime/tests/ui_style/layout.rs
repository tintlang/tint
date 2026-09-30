#[test]
fn align_position_and_z_resolve_for_a_floating_popover() {
    let code = r#"
ui fn App() {
    Menu {
        position::relative

        Anchor {
            "Send / Request"
        }

        Panel {
            position::absolute
            z::50
            top::56
            left::0
            "Send money"
        }
    }
}
"#;

    let ui_fn = parse_ui_fn(code, "App");

    let mut builder = UiBuilder::new();
    let mut vm = TintVM::new();
    builder.build_root(&ui_fn.body, &mut vm);
    let tree = builder.finish();

    let menu = tree
        .nodes
        .iter()
        .find(|n| n.tag == "Menu")
        .expect("Menu node should exist");
    assert!(menu
        .style
        .contains(&("position".to_string(), "relative".to_string())));

    let panel = tree
        .nodes
        .iter()
        .find(|n| n.tag == "Panel")
        .expect("Panel node should exist");
    assert!(panel
        .style
        .contains(&("position".to_string(), "absolute".to_string())));
    assert!(panel
        .style
        .contains(&("z-index".to_string(), "50".to_string())));
    assert!(panel
        .style
        .contains(&("top".to_string(), "56px".to_string())));
    assert!(panel
        .style
        .contains(&("left".to_string(), "0px".to_string())));
}

#[test]
fn align_resolves_to_cross_axis_alignment() {
    let code = r#"
ui fn App() {
    Row {
        direction::row
        align::center
        gap::8
        "Recent activity"
    }
}
"#;

    let ui_fn = parse_ui_fn(code, "App");

    let mut builder = UiBuilder::new();
    let mut vm = TintVM::new();
    builder.build_root(&ui_fn.body, &mut vm);
    let tree = builder.finish();

    let row = tree
        .nodes
        .iter()
        .find(|n| n.tag == "Row")
        .expect("Row node should exist");
    assert!(row
        .style
        .contains(&("align-items".to_string(), "center".to_string())));
}

#[test]
fn dotted_radius_and_padding_sides_resolve_per_corner() {
    let code = r#"
ui fn App() {
    Button {
        padding::{left::12, right::12}
        radius.top::10
        "Open"
    }
}
"#;

    let ui_fn = parse_ui_fn(code, "App");

    let mut builder = UiBuilder::new();
    let mut vm = TintVM::new();
    builder.build_root(&ui_fn.body, &mut vm);
    let tree = builder.finish();

    let button = tree
        .nodes
        .iter()
        .find(|n| n.tag == "Button")
        .expect("Button node should exist");

    assert!(button
        .style
        .contains(&("padding-left".to_string(), "12px".to_string())));
    assert!(button
        .style
        .contains(&("padding-right".to_string(), "12px".to_string())));
    assert!(button
        .style
        .contains(&("border-top-left-radius".to_string(), "10px".to_string())));
    assert!(button
        .style
        .contains(&("border-top-right-radius".to_string(), "10px".to_string())));
}

#[test]
fn padding_xy_expands_to_the_four_css_sides() {
    let code = r#"
ui fn App() {
    Button { padding.x::18 padding.y::10 "Open" }
}
"#;
    let ui_fn = parse_ui_fn(code, "App");
    let mut builder = UiBuilder::new();
    let mut vm = TintVM::new();
    builder.build_root(&ui_fn.body, &mut vm);
    let tree = builder.finish();
    let button = tree.nodes.iter().find(|n| n.tag == "Button").unwrap();

    for (side, value) in [
        ("padding-left", "18px"),
        ("padding-right", "18px"),
        ("padding-top", "10px"),
        ("padding-bottom", "10px"),
    ] {
        assert!(button
            .style
            .contains(&(side.to_string(), value.to_string())));
    }
}

#[test]
fn css_keyword_and_length_properties_resolve_without_a_stylesheet() {
    let code = r#"
ui fn App() {
    Panel {
        layout::{
            min-height::"100dvh",
            max-width::480,
            inset::0,
            flex::1,
            aspect-ratio::"16 / 9",
            align-self::stretch,
            flex-wrap::wrap,
            box-sizing::content-box,
            cursor::pointer,
            pointer-events::none,
            user-select::none,
            outline::none,
            touch-action::none,
            text-align::center
        }
    }
}
"#;

    let ui_fn = parse_ui_fn(code, "App");
    let mut builder = UiBuilder::new();
    let mut vm = TintVM::new();
    builder.build_root(&ui_fn.body, &mut vm);
    let tree = builder.finish();
    let panel = tree.nodes.iter().find(|n| n.tag == "Panel").expect("Panel node");

    for (key, value) in [
        ("min-height", "100dvh"),
        ("max-width", "480px"),
        ("inset", "0px"),
        ("flex", "1"),
        ("aspect-ratio", "16 / 9"),
        ("align-self", "stretch"),
        ("flex-wrap", "wrap"),
        ("box-sizing", "content-box"),
        ("cursor", "pointer"),
        ("pointer-events", "none"),
        ("user-select", "none"),
        ("outline", "none"),
        ("touch-action", "none"),
        ("text-align", "center"),
    ] {
        assert!(
            panel.style.contains(&(key.to_string(), value.to_string())),
            "missing {key}: {value}, got {:?}",
            panel.style
        );
    }
}

#[test]
fn width_threshold_breakpoints_resolve_in_source_order_with_negative_values() {
    let code = r#"
ui fn App() {
    Panel {
        layout::{ gap::20 }
        max-560::{ gap::12, margin.b::-72 }
        min-800::{ gap::30 }
        mobile::{ gap::8 }
    }
}
"#;

    let ui_fn = parse_ui_fn(code, "App");
    let mut builder = UiBuilder::new();
    let mut vm = TintVM::new();
    builder.build_root(&ui_fn.body, &mut vm);
    let tree = builder.finish();
    let panel = tree.nodes.iter().find(|n| n.tag == "Panel").expect("Panel node");

    let names: Vec<&str> = panel.breakpoints.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, ["max-560", "min-800", "mobile"]);
    assert!(panel.breakpoints[0]
        .1
        .contains(&("margin-bottom".to_string(), "-72px".to_string())));
    assert!(panel.breakpoints[1]
        .1
        .contains(&("gap".to_string(), "30px".to_string())));
}
