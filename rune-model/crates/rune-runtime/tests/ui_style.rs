// Proves that a UI node's `key::value` modifiers -- including a nested
// `hover::{ ... }` -- actually resolve into CSS style data on the built
// UiTree, instead of hover being something only the sandbox's own
// hardcoded, tag-name-based CSS fakes (see sandbox/src/components/
// UiPreviewNode.svelte's TAG_DEFAULTS comment for what that used to look
// like). This exercises rune-runtime/src/ui/style.rs end to end, through
// the real lexer -> parser -> UiBuilder pipeline.

use rune_ast::Item;
use rune_lexer::{collect_tokens, Lexer};
use rune_parser::Parser;
use rune_runtime::ui::builder::UiBuilder;
use rune_runtime::vm::RuneVM;

fn parse_ui_fn(code: &str, name: &str) -> rune_ast::UiFnDecl {
    let tokens = collect_tokens(&mut Lexer::new(code));
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().expect("parse failed");

    for item in program.items {
        if let Item::UiFn(f) = item {
            if f.name == name {
                return f;
            }
        }
    }

    panic!("ui fn `{}` not found", name);
}

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
    let mut vm = RuneVM::new();
    builder.build_root(&ui_fn.body, &mut vm);
    let tree = builder.finish();

    let card = tree
        .nodes
        .iter()
        .find(|n| n.tag == "Card")
        .expect("Card node should exist");

    assert!(card.style.contains(&("padding".to_string(), "24px".to_string())));
    assert!(card.style.contains(&("border-radius".to_string(), "20px".to_string())));
    assert!(card.style.contains(&("background-color".to_string(), "#6c5ce7".to_string())));
    assert!(card.style.contains(&("border".to_string(), "1px solid #8c5ce7".to_string())));

    // Hover-only properties must not leak into the base style...
    assert!(!card.style.iter().any(|(k, _)| k == "transform"));

    // ...and must show up, resolved, in hover_style.
    assert!(card.hover_style.contains(&("background-color".to_string(), "#8a7ff0".to_string())));
    assert!(card.hover_style.contains(&("transform".to_string(), "scale(1.02)".to_string())));
    assert!(card
        .hover_style
        .contains(&("box-shadow".to_string(), "0 16px 40px rgba(0,0,0,.35)".to_string())));

    let text = tree
        .nodes
        .iter()
        .find(|n| n.tag == "Text")
        .expect("Text node should exist");

    assert!(text.style.contains(&("font-size".to_string(), "34px".to_string())));
    assert!(text.style.contains(&("font-weight".to_string(), "bold".to_string())));
    assert!(text.style.contains(&("color".to_string(), "white".to_string())));

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
    let mut vm = RuneVM::new();
    builder.build_root(&ui_fn.body, &mut vm);
    let tree = builder.finish();

    let row = tree.nodes.iter().find(|n| n.tag == "Row").unwrap();
    assert!(row.style.contains(&("justify-content".to_string(), "space-between".to_string())));
}

#[test]
fn margin_transition_and_svg_are_available_to_a_node() {
    let code = r#"
ui fn App() {
    Row {
        direction::row

        CardA {
            margin::{right::10}
            "A"
        }

        CardB {
            margin::{right::28}
            "B"
        }

        CardC {
            "C"
        }

        Logo {
            svg||"<svg viewBox='0 0 24 24'><path d='M12 2 L14 9 L21 12' fill='#6c5ce7'/></svg>"
            transition::"transform .25s ease"
            hover::{ transform::"scaleX(1.15) scaleY(0.85)" }
        }
    }
}
"#;

    let ui_fn = parse_ui_fn(code, "App");

    let mut builder = UiBuilder::new();
    let mut vm = RuneVM::new();
    builder.build_root(&ui_fn.body, &mut vm);
    let tree = builder.finish();

    let card_a = tree.nodes.iter().find(|n| n.tag == "CardA").unwrap();
    assert!(card_a.style.contains(&("margin-right".to_string(), "10px".to_string())));

    let card_b = tree.nodes.iter().find(|n| n.tag == "CardB").unwrap();
    assert!(card_b.style.contains(&("margin-right".to_string(), "28px".to_string())));

    // Different siblings, different gaps -- not a uniform container `gap`.
    assert_ne!(
        card_a.style.iter().find(|(k, _)| k == "margin-right"),
        card_b.style.iter().find(|(k, _)| k == "margin-right")
    );

    let logo = tree.nodes.iter().find(|n| n.tag == "Logo").unwrap();
    assert!(logo.svg.as_deref().unwrap().contains("<path d='M12 2"));
    assert!(logo.style.contains(&("transition".to_string(), "transform .25s ease".to_string())));
    assert!(logo.hover_style.contains(&("transform".to_string(), "scaleX(1.15) scaleY(0.85)".to_string())));
}

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
    let mut vm = RuneVM::new();
    builder.build_root(&ui_fn.body, &mut vm);
    let tree = builder.finish();

    let menu = tree
        .nodes
        .iter()
        .find(|n| n.tag == "Menu")
        .expect("Menu node should exist");
    assert!(menu.style.contains(&("position".to_string(), "relative".to_string())));

    let panel = tree
        .nodes
        .iter()
        .find(|n| n.tag == "Panel")
        .expect("Panel node should exist");
    assert!(panel.style.contains(&("position".to_string(), "absolute".to_string())));
    assert!(panel.style.contains(&("z-index".to_string(), "50".to_string())));
    assert!(panel.style.contains(&("top".to_string(), "56px".to_string())));
    assert!(panel.style.contains(&("left".to_string(), "0px".to_string())));
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
    let mut vm = RuneVM::new();
    builder.build_root(&ui_fn.body, &mut vm);
    let tree = builder.finish();

    let row = tree
        .nodes
        .iter()
        .find(|n| n.tag == "Row")
        .expect("Row node should exist");
    assert!(row.style.contains(&("align-items".to_string(), "center".to_string())));
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
    let mut vm = RuneVM::new();
    builder.build_root(&ui_fn.body, &mut vm);
    let tree = builder.finish();

    let button = tree
        .nodes
        .iter()
        .find(|n| n.tag == "Button")
        .expect("Button node should exist");

    assert!(button.style.contains(&("padding-left".to_string(), "12px".to_string())));
    assert!(button.style.contains(&("padding-right".to_string(), "12px".to_string())));
    assert!(button.style.contains(&("border-top-left-radius".to_string(), "10px".to_string())));
    assert!(button.style.contains(&("border-top-right-radius".to_string(), "10px".to_string())));
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
    let mut vm = RuneVM::new();
    builder.build_root(&ui_fn.body, &mut vm);
    let tree = builder.finish();

    let column = tree
        .nodes
        .iter()
        .find(|n| n.tag == "Column")
        .expect("Column node should exist");
    assert!(column.style.contains(&("display".to_string(), "flex".to_string())));
    assert!(column.style.contains(&("flex-direction".to_string(), "column".to_string())));
    assert!(column.style.contains(&("gap".to_string(), "20px".to_string())));

    let row = tree
        .nodes
        .iter()
        .find(|n| n.tag == "Row")
        .expect("Row node should exist");
    assert!(row.style.contains(&("display".to_string(), "flex".to_string())));
    assert!(row.style.contains(&("flex-direction".to_string(), "row".to_string())));

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
