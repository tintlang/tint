use tint_lexer::{collect_tokens, Lexer};
use tint_parser::Parser;
use tint_runtime::bytecode;
use tint_runtime::ui_session::UiSession;

const SOURCE: &str = r#"
ui fn App() {
    Text { "Hello from bytecode" }
}
"#;

#[test]
fn bytecode_round_trip_uses_the_same_ui_runtime() {
    let program = Parser::new(collect_tokens(&mut Lexer::new(SOURCE)))
        .parse_program()
        .expect("source must parse");
    let bytes = bytecode::encode(&program).expect("program must encode");
    let decoded = bytecode::decode(&bytes).expect("encoded program must decode");
    assert_eq!(decoded.items.len(), program.items.len());

    let mut session =
        UiSession::from_bytecode(&bytes, "App").expect("bytecode session must use the existing VM");
    let tree = session.render().expect("bytecode UI must render");
    fn contains_text(nodes: &[std::rc::Rc<tint_runtime::ui::render::UiRenderNode>]) -> bool {
        nodes.iter().any(|node| {
            (node.tag == "Text" && node.text.as_deref() == Some("Hello from bytecode"))
                || contains_text(&node.children)
        })
    }
    assert!(contains_text(&tree));
}

#[test]
fn bytecode_rejects_unknown_versions_and_headers() {
    assert!(bytecode::decode(b"not-tint-bytecode").is_err());
}

#[test]
fn component_click_handler_survives_bytecode_loading() {
    let source = r#"
fn set_light() { theme = "light" }

ui fn App() {
    component ThemeButton { slot::content }
    ThemeButton { click||set_light "Light" }
    theme::dark { Text { "dark" } }
    theme::light { Text { "light" } }
}
"#;
    let program = Parser::new(collect_tokens(&mut Lexer::new(source)))
        .parse_program()
        .expect("source must parse");
    let bytes = bytecode::encode(&program).expect("program must encode");
    let mut session = UiSession::from_bytecode(&bytes, "App").expect("session must load");
    let before = session.render().expect("initial render must work");
    assert_eq!(before[0].on_click.as_deref(), Some("set_light"));
    session
        .dispatch("set_light")
        .expect("handler must dispatch");
    let after = session.render().expect("updated render must work");
    assert!(after.iter().any(|node| {
        node.text.as_deref() == Some("light")
            || node
                .children
                .iter()
                .any(|child| child.text.as_deref() == Some("light"))
    }));
}
