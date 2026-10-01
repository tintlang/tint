//! A compiled `ui fn` produces the same render trees as the interpreter: same
//! source, same handlers, the JSON of the nodes compared after every step.
mod common;

use common::Session;
use std::rc::Rc;
use tint_ir::typed::{lower_program, Interp, Module};
use tint_lexer::{collect_tokens, Lexer};
use tint_parser::Parser;
use tint_runtime::ui::ir_render::IrRenderer;
use tint_runtime::ui::render::UiRenderNode;

fn lower(source: &str) -> Module {
    let program = Parser::new(collect_tokens(&mut Lexer::new(source))).parse_program().expect("parse");
    let (errors, model) = tint_semantics::SemanticChecker::new(Default::default()).check_with_model(&program);
    assert!(errors.is_empty(), "type errors: {errors:?}");
    let lowered = lower_program(&program, &model);
    assert!(lowered.errors.is_empty(), "lowering errors: {:?}", lowered.errors);
    lowered.module
}

fn json(nodes: &[Rc<UiRenderNode>]) -> String {
    serde_json::to_string_pretty(&nodes.iter().map(|n| (**n).clone()).collect::<Vec<_>>()).unwrap()
}

/// Renders once, then after each handler, on the interpreter and on wasm.
fn assert_parity(source: &str, ui_fn: &str, handlers: &[&str]) -> String {
    let module = lower(source);
    let mut interp = Interp::new(&module);
    interp.init().expect("init");
    let mut renderer = IrRenderer::new();
    let mut entries = vec![ui_fn];
    entries.extend(handlers.iter().copied());
    let mut wasm = Session::new(&module, &entries).unwrap_or_else(|e| panic!("{e}"));

    let mut last = String::new();
    for step in std::iter::once(None).chain(handlers.iter().map(Some)) {
        if let Some(h) = step {
            interp.call(module.functions[*h], Vec::new()).expect("handler");
            wasm.call(h).unwrap_or_else(|e| panic!("{h}: {e}"));
        }
        let events = interp.run_ui(ui_fn, Vec::new()).expect("run ui");
        let expected = json(&renderer.render(&module.ui_templates, &events));
        wasm.call(ui_fn).unwrap_or_else(|e| panic!("{ui_fn}: {e}"));
        let actual = wasm.render_json().unwrap();
        assert_eq!(expected, actual, "render after {step:?} differs");
        last = actual;
    }
    last
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

ui fn App() {
    state count = 2
    state mode = "a"
    state items = ["x", "y", "z"]

    Column {
        gap::count
        Text { "T: {count}" }
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
    let last = assert_parity(source, "App", &["bump", "flip"]);
    assert!(last.contains("T: 3") && last.contains("is b") && last.contains("big"), "{last}");
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
}

#[test]
fn storage_now_and_http() {
    let source = r#"
fn current() {
    storage_get_or("n", "-")
}
fn save() {
    storage_set("n", "{n}")
    n = n + 1
}
fn forget() {
    storage_remove("name")
}
fn fetch() {
    last = http_get("https://example.com/a")
    let t = now_ms()
    stamp = if t > 0 { "t" } else { "?" }
}

ui fn App() {
    state n = 0
    state last = 0
    state stamp = ""
    state name = storage_get_or("name", "anon")
    Column {
        Text { "{name}: {n} {last} {stamp}" }
        Text { "{current()}" }
    }
}
"#;
    let module = lower(source);
    let handlers = ["save", "save", "forget", "fetch"];
    let mut entries = vec!["App"];
    entries.extend(handlers);
    let mut interp = Interp::new(&module);
    interp.clock = Some(1_700_000_000_000.0);
    interp.storage.insert("name".into(), "mark".into());
    interp.init().unwrap();
    let mut wasm = Session::with_storage(&module, &entries, "", r#"{"name":"mark"}"#).unwrap();
    let mut renderer = IrRenderer::new();
    for step in std::iter::once(None).chain(handlers.iter().map(Some)) {
        if let Some(h) = step {
            interp.call(module.functions[*h], Vec::new()).unwrap();
            wasm.call(h).unwrap();
        }
        let events = interp.run_ui("App", Vec::new()).unwrap();
        let expected = json(&renderer.render(&module.ui_templates, &events));
        wasm.call("App").unwrap();
        assert_eq!(expected, wasm.render_json().unwrap(), "after {step:?}");
        let storage: std::collections::BTreeMap<String, String> =
            serde_json::from_str(&wasm.rt_string("storage_snapshot_json")).unwrap();
        assert_eq!(interp.storage, storage, "storage after {step:?}");
    }
    let requests: serde_json::Value = serde_json::from_str(&wasm.rt_string("http_take_json")).unwrap();
    assert_eq!(requests, serde_json::json!([{"id": 1, "method": "GET", "url": "https://example.com/a"}]));
    assert_eq!(interp.http_requests, vec![(1, "https://example.com/a".to_string())]);
}

#[test]
fn host_functions_and_callbacks() {
    use tint_ir::typed::interp::NativeOutcome;
    use serde_json::json;
    let source = r#"
app {
    js::"./host.js"
}

struct Point { x: number, y: number }

fn greet() {
    let s: string = shout("hi")
    msg = s
}
fn total() {
    let t: number = sum_all([1, 2, 3.5])
    msg = "total {t}"
}
fn point() {
    let p: Point = make_point(3, 4)
    msg = "point {p.x},{p.y}"
}
async fn run() {
    msg = "waiting"
    await wait(400)
    msg = "done"
}
async fn run_now() {
    await wait_now(1)
    msg = "ran now"
}
fn plain() {
    wait_now(1, |r| { msg = "plain callback" })
}
async fn broken() {
    msg = "before"
    await boom()
    msg = "after"
}

ui fn App() {
    state msg = "idle"
    Text { "{msg}" }
}
"#;
    let module = lower(source);
    let handlers = ["greet", "total", "point", "run", "run_now", "plain", "broken"];
    let mut entries = vec!["App"];
    entries.extend(handlers);
    let mut interp = Interp::new(&module);
    interp.natives = Some(Box::new(|name, args| match name {
        "shout" => NativeOutcome::Value(json!(format!("{}!", args[0].as_str().unwrap().to_uppercase()))),
        "sum_all" => NativeOutcome::Value(json!(args[0].as_array().unwrap().iter().filter_map(|x| x.as_f64()).sum::<f64>())),
        "make_point" => NativeOutcome::Value(json!({"__tint_struct": "Point", "x": args[0], "y": args[1]})),
        "boom" => NativeOutcome::Error("boom from the host".into()),
        "wait" => NativeOutcome::Pending,
        "wait_now" => NativeOutcome::Value(json!(true)),
        other => NativeOutcome::Error(format!("no host function `{other}`")),
    }));
    interp.init().unwrap();
    let mut wasm = Session::new(&module, &entries).unwrap_or_else(|e| panic!("{e}"));
    let mut renderer = IrRenderer::new();
    let mut check = |interp: &mut Interp, wasm: &mut Session, what: &str| {
        let events = interp.run_ui("App", Vec::new()).unwrap();
        let expected = json(&renderer.render(&module.ui_templates, &events));
        wasm.call("App").unwrap();
        let actual = wasm.render_json().unwrap();
        assert_eq!(expected, actual, "after {what}");
        actual
    };
    check(&mut interp, &mut wasm, "start");
    for h in handlers {
        interp.call(module.functions[h], Vec::new()).unwrap();
        wasm.call(h).unwrap_or_else(|e| panic!("{h}: {e}"));
        let shown = check(&mut interp, &mut wasm, h);
        if h == "run" {
            assert!(shown.contains("waiting"), "{shown}");
            // The host answers later, on both engines.
            interp.resolve_pending(0, Ok(json!(null))).unwrap();
            wasm.answer_callback(0, None).unwrap();
            let shown = check(&mut interp, &mut wasm, "run answered");
            assert!(shown.contains("done"), "{shown}");
        }
    }
    assert!(interp.pending.is_empty());
}

#[test]
fn dynamic_keys_reach_the_render_nodes() {
    let out = assert_parity(
        r#"
struct P { id: number }
fn mk() -> Vec<P> {
    let mut o: Vec<P> = []
    o.push(P { id: 7 })
    o.push(P { id: 9 })
    o
}
ui fn App() {
    state rows = mk()
    state ids = [1, 2]
    Column {
        Block { for{r in rows} Row { key::{r.id} "{r.id}" } }
        Block { for{i in ids} Row { key::{i} "{i}" } }
        Block { for{i in ids} Row { key::{i * 2} "{i}" } }
    }
}
"#,
        "App",
        &[],
    );
    for key in ["\"7\"", "\"9\"", "\"1\"", "\"4\""] {
        assert!(out.contains(&format!("\"key\": {key}")), "missing key {key}");
    }
}
