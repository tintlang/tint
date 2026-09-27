#[test]
fn hover_and_style_modifiers_resolve_to_css() {
    let code = r#"
ui fn App() {
    Column {
        padding::20
        radius::12
        background::#12141a

        Card {
            padding::24
            radius::20
            background::#6c5ce7
            border::{1, #8c5ce7}
            hover::{ background::#8a7ff0, scale::1.02, shadow::"0 16px 40px rgba(0,0,0,.35)" }

            Text { text::{34, bold, white} "Hi" }
        }
    }
}
"#;

    let ui_fn = parse_ui_fn(code, "App");

    let mut builder = UiBuilder::new();
    let mut vm = TintVM::new();
    builder.build_root(&ui_fn.body, &mut vm);
    let tree = builder.finish();

    let card = tree
        .nodes
        .iter()
        .find(|n| n.tag == "Card")
        .expect("Card node should exist");

    assert!(card
        .style
        .contains(&("padding".to_string(), "24px".to_string())));
    assert!(card
        .style
        .contains(&("border-radius".to_string(), "20px".to_string())));
    assert!(card
        .style
        .contains(&("background-color".to_string(), "#6c5ce7".to_string())));
    assert!(card
        .style
        .contains(&("border".to_string(), "1px solid #8c5ce7".to_string())));

    // Hover-only properties must not leak into the base style...
    assert!(!card.style.iter().any(|(k, _)| k == "transform"));

    // ...and must show up, resolved, in hover_style.
    assert!(card
        .hover_style
        .contains(&("background-color".to_string(), "#8a7ff0".to_string())));
    assert!(card
        .hover_style
        .contains(&("transform".to_string(), "scale(1.02)".to_string())));
    assert!(card.hover_style.contains(&(
        "box-shadow".to_string(),
        "0 16px 40px rgba(0,0,0,.35)".to_string()
    )));

    let text = tree
        .nodes
        .iter()
        .find(|n| n.tag == "Text")
        .expect("Text node should exist");

    assert!(text
        .style
        .contains(&("font-size".to_string(), "34px".to_string())));
    assert!(text
        .style
        .contains(&("font-weight".to_string(), "bold".to_string())));
    assert!(text
        .style
        .contains(&("color".to_string(), "white".to_string())));

    // A node with no hover::{} modifier gets no hover style at all -- a
    // renderer must not invent one just because of the node's tag name.
    let column = tree
        .nodes
        .iter()
        .find(|n| n.tag == "Column")
        .expect("Column node should exist");
    assert!(column.hover_style.is_empty());
}

#[test]
fn semantic_style_groups_resolve_without_whitespace_rules() {
    let code = r#"
ui fn App() {
    Card {
        layout::{ padding::24, direction::column, gap::12 }
        paint::{ gradient::{ angle::135, from::#6c5ce7, to::#8b7cf0 }, radius::20, border::{1, #8b7cf0} }
        motion::{ transition::"transform .2s ease", hover::{ scale::1.03, shadow::"0 16px 40px rgba(0,0,0,.35)" } }
        "Card"
    }
}
"#;

    let ui_fn = parse_ui_fn(code, "App");
    let mut builder = UiBuilder::new();
    let mut vm = TintVM::new();
    builder.build_root(&ui_fn.body, &mut vm);
    let tree = builder.finish();
    let card = tree.nodes.iter().find(|n| n.tag == "Card").unwrap();

    assert!(card
        .style
        .contains(&("padding".to_string(), "24px".to_string())));
    assert!(card
        .style
        .contains(&("display".to_string(), "flex".to_string())));
    assert!(card
        .style
        .contains(&("flex-direction".to_string(), "column".to_string())));
    assert!(card.style.contains(&(
        "background-image".to_string(),
        "linear-gradient(135deg, #6c5ce7, #8b7cf0)".to_string()
    )));
    assert!(card
        .style
        .contains(&("border-radius".to_string(), "20px".to_string())));
    assert!(card
        .style
        .contains(&("transition".to_string(), "transform .2s ease".to_string())));
    assert!(card
        .hover_style
        .contains(&("transform".to_string(), "scale(1.03)".to_string())));
    assert!(card.hover_style.contains(&(
        "box-shadow".to_string(),
        "0 16px 40px rgba(0,0,0,.35)".to_string()
    )));
}

#[test]
fn row_and_column_are_flex_containers_by_default() {
    let code = r#"
ui fn App() {
    Column {
        gap::16
        Row {
            gap::8
            "A"
            "B"
        }
    }
}
"#;

    let ui_fn = parse_ui_fn(code, "App");
    let mut builder = UiBuilder::new();
    let mut vm = TintVM::new();
    builder.build_root(&ui_fn.body, &mut vm);
    let tree = builder.finish();

    let column = tree.nodes.iter().find(|node| node.tag == "Column").unwrap();
    assert!(column
        .style
        .contains(&("display".to_string(), "flex".to_string())));
    assert!(column
        .style
        .contains(&("flex-direction".to_string(), "column".to_string())));
    assert!(column.style.contains(&("gap".to_string(), "16px".to_string())));

    let row = tree.nodes.iter().find(|node| node.tag == "Row").unwrap();
    assert!(row
        .style
        .contains(&("display".to_string(), "flex".to_string())));
    assert!(row
        .style
        .contains(&("flex-direction".to_string(), "row".to_string())));
    assert!(row.style.contains(&("gap".to_string(), "8px".to_string())));
}

#[test]
fn justify_resolves_to_main_axis_distribution() {
    let code = r#"
ui fn App() {
    Row {
        direction::row
        justify::space-between
        "Cards"
    }
}
"#;

    let ui_fn = parse_ui_fn(code, "App");

    let mut builder = UiBuilder::new();
    let mut vm = TintVM::new();
    builder.build_root(&ui_fn.body, &mut vm);
    let tree = builder.finish();

    let row = tree.nodes.iter().find(|n| n.tag == "Row").unwrap();
    assert!(row
        .style
        .contains(&("justify-content".to_string(), "space-between".to_string())));
}
