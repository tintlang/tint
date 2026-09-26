#[test]
fn theme_tag_builds_only_the_active_fragment() {
    let code = r#"
ui fn App() {
    theme::dark { Dark { "dark" } }
    theme::light { Light { "light" } }
}
"#;
    let ui_fn = parse_ui_fn(code, "App");
    let mut builder = UiBuilder::new();
    let mut vm = TintVM::new();
    vm.define_var("theme", tint_evaluator::Value::String("light".into()));
    builder.build_root(&ui_fn.body, &mut vm);
    let tree = builder.finish();

    assert!(tree.nodes.iter().any(|n| n.tag == "Light"));
    assert!(!tree.nodes.iter().any(|n| n.tag == "Dark"));
}

#[test]
fn direction_resolves_to_real_flex_layout() {
    let code = r#"
ui fn App() {
    Column {
        direction::column
        gap::20

        Row {
            direction::row
            gap::12
            "A"
            "B"
        }

        Text { "no direction, stays a plain block" }
    }
}
"#;

    let ui_fn = parse_ui_fn(code, "App");

    let mut builder = UiBuilder::new();
    let mut vm = TintVM::new();
    builder.build_root(&ui_fn.body, &mut vm);
    let tree = builder.finish();

    let column = tree
        .nodes
        .iter()
        .find(|n| n.tag == "Column")
        .expect("Column node should exist");
    assert!(column
        .style
        .contains(&("display".to_string(), "flex".to_string())));
    assert!(column
        .style
        .contains(&("flex-direction".to_string(), "column".to_string())));
    assert!(column
        .style
        .contains(&("gap".to_string(), "20px".to_string())));

    let row = tree
        .nodes
        .iter()
        .find(|n| n.tag == "Row")
        .expect("Row node should exist");
    assert!(row
        .style
        .contains(&("display".to_string(), "flex".to_string())));
    assert!(row
        .style
        .contains(&("flex-direction".to_string(), "row".to_string())));

    // A node with no direction:: gets neither display nor flex-direction
    // invented for it -- it stays a plain block, same as before this
    // modifier existed.
    let text = tree
        .nodes
        .iter()
        .find(|n| n.tag == "Text")
        .expect("Text node should exist");
    assert!(!text.style.iter().any(|(k, _)| k == "display"));
}
