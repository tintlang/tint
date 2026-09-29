//! The typed-IR engine builds the same UI trees as the tree-walking builder:
//! same source, same `state`, same events, compared node for node.

use std::rc::Rc;
use tint_ir::typed::{replay, Interp, Val};
use tint_lexer::{collect_tokens, Lexer};
use tint_parser::Parser;
use tint_runtime::ui::ir_sink::IrUiSink;
use tint_runtime::ui::render::{to_render_tree, UiRenderNode};
use tint_runtime::ui_session::UiSession;

struct IrSession {
    module: tint_ir::typed::Module,
    ui_fn: String,
}

fn lower(source: &str, ui_fn: &str) -> IrSession {
    let tokens = collect_tokens(&mut Lexer::new(source));
    let program = Parser::new(tokens).parse_program().expect("parse");
    let (errors, model) = tint_semantics::SemanticChecker::new(Default::default()).check_with_model(&program);
    assert!(errors.is_empty(), "type errors: {errors:?}");
    let lowered = tint_ir::typed::lower_program(&program, &model);
    assert!(lowered.errors.is_empty(), "lowering errors: {:?}", lowered.errors);
    IrSession { module: lowered.module, ui_fn: ui_fn.to_string() }
}

fn render_ir(interp: &mut Interp, module: &tint_ir::typed::Module, ui_fn: &str) -> Vec<Rc<UiRenderNode>> {
    let events = interp.run_ui(ui_fn, Vec::new()).expect("run ui");
    let mut sink = IrUiSink::new();
    replay(&module.ui_templates, &events, &mut sink);
    let root = sink.root();
    let tree = sink.finish();
    to_render_tree(&tree, root).children
}

fn json(nodes: &[Rc<UiRenderNode>]) -> String {
    serde_json::to_string_pretty(&nodes.iter().map(|n| (**n).clone()).collect::<Vec<_>>()).unwrap()
}

fn json_plain(nodes: &[Rc<UiRenderNode>]) -> String {
    json(nodes)
}

/// Renders once, then after each handler, on both engines.
fn assert_parity(source: &str, ui_fn: &str, handlers: &[&str]) -> String {
    let ir = lower(source, ui_fn);
    let mut interp = Interp::new(&ir.module);
    interp.init().expect("init");
    let mut walker = UiSession::new(source, ui_fn).expect("session");

    let expected = json(&walker.render().expect("walker render"));
    let mut actual = json_plain(&render_ir(&mut interp, &ir.module, &ir.ui_fn));
    assert_eq!(expected, actual, "initial render differs");

    for handler in handlers {
        let expected = json(&walker.dispatch(handler).expect("walker dispatch"));
        let id = ir.module.functions[*handler];
        interp.call(id, Vec::new()).expect("handler");
        actual = json_plain(&render_ir(&mut interp, &ir.module, &ir.ui_fn));
        assert_eq!(expected, actual, "render after `{handler}` differs");
    }
    actual
}

#[test]
fn wallet_app_matches_with_events() {
    let source = include_str!("../../../examples/ui_app.tn");
    assert_parity(source, "App", &["toggle_menu", "show_tip", "hide_tip", "toggle_menu"]);
}

#[test]
fn grouped_style_syntax_matches() {
    assert_parity(include_str!("../../../examples/grouped_ui.tn"), "App", &[]);
}

#[test]
fn control_flow_and_interpolation() {
    let source = r#"
fn bump() {
    count = count + 1
}
fn flip() {
    mode = if mode == "a" { "b" } else { "a" }
}

ui fn App(title: string) {
    state count = 2
    state mode = "a"
    state items = ["x", "y", "z"]

    Column {
        gap::count
        Text { "{title}: {count}" }
        for { item in items } {
            Row { key::item Text { "{item}!" } }
        }
        Block {
            match{mode}
            case a { Text { "is a" } }
            case b { Text { "is b" } }
            case _ { Text { "other" } }
        }
        Block { if{count > 2} Text { "big" } }
    }
}
"#;
    let ir = lower(source, "App");
    let mut interp = Interp::new(&ir.module);
    interp.init().unwrap();
    let render = |interp: &mut Interp| {
        let events = interp.run_ui("App", vec![Val::str("T")]).unwrap();
        let mut sink = IrUiSink::new();
        replay(&ir.module.ui_templates, &events, &mut sink);
        let root = sink.root();
        let tree = sink.finish();
        to_render_tree(&tree, root).children
    };
    let texts = |nodes: &[Rc<UiRenderNode>]| {
        fn walk(n: &UiRenderNode, out: &mut Vec<String>) {
            if let Some(t) = &n.text {
                out.push(t.clone());
            }
            for c in &n.children {
                walk(c, out);
            }
        }
        let mut out = Vec::new();
        nodes.iter().for_each(|n| walk(n, &mut out));
        out
    };
    let first = texts(&render(&mut interp));
    assert_eq!(first, ["T: 2", "x!", "y!", "z!", "is a"]);
    for name in ["bump", "flip"] {
        interp.call(ir.module.functions[name], Vec::new()).unwrap();
    }
    let second = texts(&render(&mut interp));
    assert_eq!(second, ["T: 3", "x!", "y!", "z!", "is b", "big"]);
}

#[test]
fn components_variants_and_slots() {
    let source = r#"
ui fn App() {
    style Base { padding::8 radius::4 }
    component Button {
        use::Base
        background::#111
        variant::outline { background::#fff }
        Text { "btn" }
        slot::content
    }
    Column {
        Button { variant::outline "one" }
        Button { "two" }
    }
}
"#;
    assert_parity(source, "App", &[]);
}

#[test]
fn themes_tokens_and_viewport() {
    let source = r#"
fn go_light() {
    theme = "light"
}

ui fn App() {
    theme::dark {
        tokens { bg::#111111 fg::#eeeeee }
        Panel { background::@bg color::@fg "dark" }
    }
    theme::light {
        tokens { bg::#ffffff fg::#222222 }
        Panel { background::@bg color::@fg "light" }
    }
    Footer { if{viewport_width > 1000} "wide" }
}
"#;
    let last = assert_parity(source, "App", &["go_light"]);
    assert!(last.contains("light") && last.contains("#ffffff") && !last.contains("#111111"), "{last}");
}

#[test]
fn nested_for_match_and_component_slots() {
    let source = r#"
fn pick() {
    kind = "card"
}

ui fn App() {
    state kind = "row"
    state rows = [1, 2, 3]

    component Frame {
        padding::4
        Header { padding::1 }
        slot::title
        slot::content
    }

    Frame {
        slot title { Text { "T" } }
        Text { "body" }
        Column {
            for{r in rows}
            Cell { "{r}" }
        }
        Holder {
            match{kind}
            case row { Text { "as row" } }
            case card { Text { "as card" } }
        }
    }
}
"#;
    let last = assert_parity(source, "App", &["pick"]);
    assert!(last.contains("as card") && !last.contains("as row"), "{last}");
    assert!(last.contains("\"T\"") && last.contains("body"), "{last}");
}
