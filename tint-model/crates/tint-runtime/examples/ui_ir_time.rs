//! `cargo run --release -p tint-runtime --features typed-ir --example ui_ir_time -- app-render.tn create1k ...`
//! Times the typed-IR path: handler call, `ui fn` run, sink replay, render tree.
use std::time::Instant;
use tint_ir::typed::Interp;
use tint_runtime::ui::ir_render::IrRenderer;
use tint_lexer::{collect_tokens, Lexer};
use tint_parser::Parser;

fn main() {
    tint_runtime::ui::render::FOLD_TEXT.store(true, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::args().nth(1).unwrap();
    let src = std::fs::read_to_string(path).unwrap();
    let tokens = collect_tokens(&mut Lexer::new(&src));
    let program = Parser::new(tokens).parse_program().expect("parse");
    let (errors, model) = tint_semantics::SemanticChecker::new(Default::default()).check_with_model(&program);
    assert!(errors.is_empty(), "type errors: {errors:?}");
    let lowered = tint_ir::typed::lower_program(&program, &model);
    assert!(lowered.errors.is_empty(), "lowering errors: {:?}", lowered.errors);
    let module = lowered.module;
    let mut interp = Interp::new(&module);
    let mut renderer = IrRenderer::new();
    interp.init().unwrap();
    for name in std::env::args().skip(2) {
        let id = module.functions[name.as_str()];
        let t0 = Instant::now();
        interp.call(id, Vec::new()).unwrap();
        let t1 = Instant::now();
        let events = interp.run_ui("App", Vec::new()).unwrap();
        let t2 = Instant::now();
        let r = renderer.render(&module.ui_templates, &events);
        let t3 = Instant::now();
        let t4 = t3;
        let shared = r.len();
        let _ = shared;
        println!("{name}: handler {:?} ui-run {:?} ({} events) cons {:?} -- total {:?} roots {}",
            t1 - t0, t2 - t1, events.len(), t3 - t2, t4 - t0, r.len());
    }
}
