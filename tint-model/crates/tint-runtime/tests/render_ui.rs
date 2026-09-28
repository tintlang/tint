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
use tint_runtime::ui_session::UiSession;
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
    assert!(card
        .style
        .contains(&("padding".to_string(), "24px".to_string())));
    assert!(card
        .style
        .contains(&("background-color".to_string(), "#6c5ce7".to_string())));
    assert!(card
        .hover_style
        .contains(&("background-color".to_string(), "#8a7ff0".to_string())));
    assert!(card
        .hover_style
        .contains(&("transform".to_string(), "scale(1.04)".to_string())));

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

#[test]
fn active_theme_tokens_resolve_inside_ui_modifier_tuples() {
    let code = r#"
ui fn Widget() {
    state theme = "dark"
    theme::dark {
        tokens {
            text-color::#ffffff
            button-size::18
            outline-text::#ffffff
            outline-border::#ffffff
        }
    }
    theme::light {
        tokens {
            text-color::#111111
            button-size::16
            outline-text::#111111
            outline-border::#111111
        }
    }
    Button {
        text::{@button-size, @text-color}
        layout::{ padding.x::@button-size }
        "Open"
    }
    Outline {
        paint::{ color::@outline-text, border::{1, @outline-border} }
        "GitHub"
    }
}

"#;

    let mut session = UiSession::new(code, "Widget").expect("session creation failed");
    let nodes = session.render().expect("render failed");
    let button = &nodes[0];

    assert!(button
        .style
        .contains(&("padding-left".to_string(), "18px".to_string())));
    assert!(button
        .style
        .contains(&("padding-right".to_string(), "18px".to_string())));
    assert!(button
        .style
        .contains(&("font-size".to_string(), "18px".to_string())));
    assert!(button
        .style
        .contains(&("color".to_string(), "#ffffff".to_string())));
    let outline = &nodes[1];
    assert!(outline
        .style
        .contains(&("color".to_string(), "#ffffff".to_string())));
    assert!(outline
        .style
        .contains(&("border".to_string(), "1px solid #ffffff".to_string())));
}

#[test]
fn named_styles_are_composed_before_runtime_style_resolution() {
    let code = r#"
ui fn Widget() {
    style ButtonBase {
        layout::{ padding.x::12 }
        motion::{ transition::"transform .2s ease", hover::{ scale::1.04 } }
    }
    Button {
        use::ButtonBase
        paint::{ background::#ffffff }
        "Open"
    }
}

"#;

    let mut session = UiSession::new(code, "Widget").expect("session creation failed");
    let nodes = session.render().expect("render failed");
    let button = &nodes[0];

    assert!(button
        .style
        .contains(&("padding-left".to_string(), "12px".to_string())));
    assert!(button
        .style
        .contains(&("padding-right".to_string(), "12px".to_string())));
    assert!(button
        .hover_style
        .contains(&("transform".to_string(), "scale(1.04)".to_string())));
}

#[test]
fn components_expand_and_route_default_and_named_slots() {
    let code = r#"
ui fn Widget() {
    component Card {
        layout::{ direction::column }
        variant::raised { paint::{ background::#ffffff } }
        slot::header
        slot::content
    }
    Card {
        variant::raised
        slot header { Text { "Title" } }
        Text { "Body" }
    }
}
"#;

    let mut session = UiSession::new(code, "Widget").expect("session creation failed");
    let nodes = session.render().expect("render failed");
    let card = &nodes[0];

    assert_eq!(card.tag, "Card");
    assert!(card
        .style
        .contains(&("background-color".to_string(), "#ffffff".to_string())));
    assert_eq!(card.children.len(), 2);
    assert_eq!(card.children[0].children[0].text, Some("Title".to_string()));
    assert_eq!(card.children[1].children[0].text, Some("Body".to_string()));
}

// `for{var in iterable}` used to be parsed and then completely ignored --
// the modifier's loop variable name was even discarded by the parser (see
// tint-parser/src/ui/block.rs) and UiBuilder had no evaluator to run
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
        vec![
            Some("A".to_string()),
            Some("B".to_string()),
            Some("C".to_string())
        ],
        "each repetition's \"{{tx}}\" interpolation resolved to that iteration's real value"
    );
}

// Standalone `for { var in iterable } { ...body... }`: unlike the `for{}`
// modifier above (which repeats a single node's own children, with that
// node itself as the stable wrapper), this is a child in its own right --
// it splices a whole run of sibling nodes per iteration straight into the
// parent, so it can sit between static siblings with no synthetic wrapper
// node around the loop body.
#[test]
fn block_for_node_splices_multiple_children_per_iteration_among_siblings() {
    let code = r#"
ui fn Widget() {
    Block {
        direction::column
        "Header"
        for { item in ["A", "B"] } {
            Text { "{item}" }
            Divider {}
        }
        "Footer"
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
    let tags: Vec<&str> = block.children.iter().map(|c| c.tag.as_str()).collect();
    assert_eq!(
        tags,
        vec!["Text", "Text", "Divider", "Text", "Divider", "Text"],
        "Header, then (Text, Divider) per item spliced flat into Block, then Footer -- \
         no wrapper node around the loop body"
    );

    let texts: Vec<Option<String>> = block
        .children
        .iter()
        .map(|c| {
            c.text
                .clone()
                .or_else(|| c.children.first().and_then(|t| t.text.clone()))
        })
        .collect();
    assert_eq!(
        texts,
        vec![
            Some("Header".to_string()),
            Some("A".to_string()),
            None,
            Some("B".to_string()),
            None,
            Some("Footer".to_string()),
        ],
        "each repetition's \"{{item}}\" interpolation resolved to that iteration's own value"
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

// `match{scrutinee}` + `case label { ... }` children used to parse into
// completely inert AST -- worse, back when this was XML-only syntax
// (`<case label>...</case>`, since removed -- see
// tint-parser/src/ui/block.rs's `parse_block_case_node`, its block-mode
// replacement), the parser silently discarded each case's own label token
// (`ready`/`error`/...), so two different cases parsed into byte-identical
// nodes with no way to tell them apart even if something HAD tried to
// evaluate them. Proves the label survives parsing and only the matching
// arm's children are built -- the `case { ... }` arm itself never shows
// up in the render tree.
#[test]
fn match_modifier_builds_only_the_matching_case_arm() {
    let code = r#"
ui fn TestMatch(status) {
    Block {
        match{status}
        case ready { Text { "OK" } }
        case error { Text { "ERR" } }
        case loading { Text { "LOAD" } }
    }
}
"#;

    let tokens = collect_tokens(&mut Lexer::new(code));
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().expect("parse failed");

    let mut vm = TintVM::new();
    vm.run_program(&program);

    let nodes = vm
        .render_ui_fn(
            "TestMatch",
            &[tint_evaluator::value::Value::String("error".into())],
        )
        .expect("render_ui_fn failed");

    assert_eq!(nodes.len(), 1, "one top-level node (Block)");
    let block = &nodes[0];
    assert_eq!(
        block.children.len(),
        1,
        "only the matching case error arm's children were built, not all three cases' worth"
    );
    assert_eq!(block.children[0].tag, "Text");
    assert_eq!(block.children[0].children[0].text, Some("ERR".to_string()));
}

#[test]
fn match_modifier_falls_back_to_the_wildcard_case() {
    let code = r#"
ui fn TestMatch(status) {
    Block {
        match{status}
        case ready { Text { "OK" } }
        case _ { Text { "UNKNOWN" } }
    }
}
"#;

    let tokens = collect_tokens(&mut Lexer::new(code));
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().expect("parse failed");

    let mut vm = TintVM::new();
    vm.run_program(&program);

    let nodes = vm
        .render_ui_fn(
            "TestMatch",
            &[tint_evaluator::value::Value::String(
                "something_else".into(),
            )],
        )
        .expect("render_ui_fn failed");

    let block = &nodes[0];
    assert_eq!(block.children.len(), 1, "the wildcard `_` case matched");
    assert_eq!(
        block.children[0].children[0].text,
        Some("UNKNOWN".to_string())
    );
}

#[test]
fn match_modifier_with_no_matching_case_and_no_wildcard_renders_nothing() {
    let code = r#"
ui fn TestMatch(status) {
    Block {
        match{status}
        case ready { Text { "OK" } }
        case error { Text { "ERR" } }
    }
}
"#;

    let tokens = collect_tokens(&mut Lexer::new(code));
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().expect("parse failed");

    let mut vm = TintVM::new();
    vm.run_program(&program);

    let nodes = vm
        .render_ui_fn(
            "TestMatch",
            &[tint_evaluator::value::Value::String("loading".into())],
        )
        .expect("render_ui_fn failed");

    let block = &nodes[0];
    assert_eq!(
        block.children.len(),
        0,
        "no case matched and there's no wildcard -- the Block stays empty, not a crash"
    );
}

// `children { ... }` -- a grouping tag for separating a node's own
// modifiers from several structural children, so they don't blur
// together in a long block (the motivating case: a Button with several
// modifiers AND several structural children, not just one bare text
// child). Proves it's transparent: the "children" tag itself never shows
// up as a node in the render tree, only its own children do, spliced
// directly into Button -- same shape as `Theme`/standalone `for {}` (see
// tint-runtime/src/ui/builder/nodes.rs's `build_into`).
#[test]
fn children_tag_splices_grouped_children_without_a_wrapper_node() {
    let code = r#"
ui fn Widget() {
    Button {
        click||restart
        padding::{left::20, right::20}

        children {
            Icon { "refresh" }
            Text { "Restart" }
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
    assert_eq!(nodes.len(), 1, "one top-level node (Button)");

    let button = &nodes[0];
    assert_eq!(button.tag, "Button");
    assert_eq!(
        button.on_click.as_deref(),
        Some("restart"),
        "click|| still resolves normally alongside a children{{}} tag"
    );
    assert!(button
        .style
        .contains(&("padding-left".to_string(), "20px".to_string())));

    let tags: Vec<&str> = button.children.iter().map(|c| c.tag.as_str()).collect();
    assert_eq!(
        tags,
        vec!["Icon", "Text"],
        "Icon and Text were spliced directly into Button -- no \"children\" \
         wrapper node in between"
    );
}
