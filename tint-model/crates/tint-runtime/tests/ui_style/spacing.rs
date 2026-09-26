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
    let mut vm = TintVM::new();
    builder.build_root(&ui_fn.body, &mut vm);
    let tree = builder.finish();

    let card_a = tree.nodes.iter().find(|n| n.tag == "CardA").unwrap();
    assert!(card_a
        .style
        .contains(&("margin-right".to_string(), "10px".to_string())));

    let card_b = tree.nodes.iter().find(|n| n.tag == "CardB").unwrap();
    assert!(card_b
        .style
        .contains(&("margin-right".to_string(), "28px".to_string())));

    // Different siblings, different gaps -- not a uniform container `gap`.
    assert_ne!(
        card_a.style.iter().find(|(k, _)| k == "margin-right"),
        card_b.style.iter().find(|(k, _)| k == "margin-right")
    );

    let logo = tree.nodes.iter().find(|n| n.tag == "Logo").unwrap();
    assert!(logo.svg.as_deref().unwrap().contains("<path d='M12 2"));
    assert!(logo
        .style
        .contains(&("transition".to_string(), "transform .25s ease".to_string())));
    assert!(logo.hover_style.contains(&(
        "transform".to_string(),
        "scaleX(1.15) scaleY(0.85)".to_string()
    )));
}
