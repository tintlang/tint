// Exercises the path tint-wasm's `render_ui()` calls (see
// crates/tint-wasm/src/lib.rs): lex -> parse -> `TintVM::run_program` ->
// `TintVM::render_ui_fn`, ending in the serializable `UiRenderNode` tree
// that crosses the wasm boundary to the sandbox. `ui_style.rs` already
// covers style/hover resolution on the raw `UiTree`; this proves the
// same data survives the `UiRenderNode` conversion (ui/render.rs), that
// a bare string child's text is actually captured (previously silently
// dropped -- see builder.rs's `render_ui_text`), and that `render_ui_fn`
// doesn't depend on the ui fn being named App/Main (unlike
// `run_program`'s own auto-mount).

use tint_lexer::{collect_tokens, Lexer};
use tint_parser::Parser;
use tint_runtime::vm::TintVM;

#[test]
fn render_ui_fn_produces_styled_text_bearing_tree() {
    let code = r#"
ui fn Widget() {
    Card {
        padding::24
        background::#6c5ce7
        hover::{ background::#8a7ff0, scale::1.04 }

        "Hello"
    }
}
"#;

    let tokens = collect_tokens(&mut Lexer::new(code));
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().expect("parse failed");

    let mut vm = TintVM::new();
    vm.run_program(&program);

    // "Widget" is neither App nor Main, so run_program's own auto-mount
    // never touches it -- render_ui_fn must mount it itself.
    let nodes = vm.render_ui_fn("Widget", &[]).expect("render_ui_fn failed");
    assert_eq!(nodes.len(), 1, "one top-level node (Card)");

    let card = &nodes[0];
    assert_eq!(card.tag, "Card");
    assert!(card.style.contains(&("padding".to_string(), "24px".to_string())));
    assert!(card.style.contains(&("background-color".to_string(), "#6c5ce7".to_string())));
    assert!(card.hover_style.contains(&("background-color".to_string(), "#8a7ff0".to_string())));
    assert!(card.hover_style.contains(&("transform".to_string(), "scale(1.04)".to_string())));

    assert_eq!(card.children.len(), 1, "one child (the text node)");
    let text_node = &card.children[0];
    assert_eq!(text_node.text, Some("Hello".to_string()));
}

#[test]
fn render_ui_fn_reports_unknown_ui_fn() {
    let mut vm = TintVM::new();
    let err = vm.render_ui_fn("DoesNotExist", &[]);
    assert!(err.is_err());
}

// `for{var in iterable}` used to be parsed and then completely ignored --
// the modifier's loop variable name was even discarded by the parser (see
// tint-parser/src/ui/{block,xml}.rs) and UiBuilder had no evaluator to run
// the iterable expression with anyway. Proves it now actually repeats the
// node's children once per item, with the loop variable bound to the real
// per-iteration value inside each repetition's text interpolations.
#[test]
fn for_loop_repeats_children_with_bound_variable() {
    let code = r#"
ui fn Widget() {
    Block {
        direction::column
        gap::10
        for{tx in ["A", "B", "C"]}

        ListItem {
            "{tx}"
        }
    }
}
"#;

    let tokens = collect_tokens(&mut Lexer::new(code));
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().expect("parse failed");

    let mut vm = TintVM::new();
    vm.run_program(&program);

    let nodes = vm.render_ui_fn("Widget", &[]).expect("render_ui_fn failed");
    assert_eq!(nodes.len(), 1, "one top-level node (Block)");

    let block = &nodes[0];
    assert_eq!(
        block.children.len(),
        3,
        "for{{}} repeated the ListItem child once per array item, not once total"
    );

    // Each ListItem's own child is the leaf text node ("{tx}" is a bare
    // string child, same shape as render_ui_fn_produces_styled_text_bearing_tree's "Hello").
    let texts: Vec<Option<String>> = block
        .children
        .iter()
        .map(|item| item.children.first().and_then(|t| t.text.clone()))
        .collect();
    assert_eq!(
        texts,
        vec![Some("A".to_string()), Some("B".to_string()), Some("C".to_string())],
        "each repetition's \"{{tx}}\" interpolation resolved to that iteration's real value"
    );
}

// `if{cond}` was parsed but never evaluated -- a node carrying it rendered
// unconditionally either way. Proves a falsy condition now removes the
// node (and its children) entirely, while a truthy one still renders.
#[test]
fn if_modifier_hides_node_when_false_keeps_when_true() {
    // Block mode puts control-flow modifiers inside the braces, alongside
    // any other modifier (same position `for{}` uses above) -- not as
    // `Tag if{cond} { ... }` before the opening brace.
    let code = r#"
ui fn Widget() {
    Block {
        if{false}
        "Hidden"
    }
    Block {
        if{true}
        "Shown"
    }
}
"#;

    let tokens = collect_tokens(&mut Lexer::new(code));
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().expect("parse failed");

    let mut vm = TintVM::new();
    vm.run_program(&program);

    let nodes = vm.render_ui_fn("Widget", &[]).expect("render_ui_fn failed");
    assert_eq!(
        nodes.len(),
        1,
        "the if{{false}} Block is skipped entirely, only if{{true}} survives"
    );
    assert_eq!(nodes[0].children[0].text, Some("Shown".to_string()));
}
