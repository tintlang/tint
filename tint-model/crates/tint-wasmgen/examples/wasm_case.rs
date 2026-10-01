//! `cargo run --release -p tint-wasmgen --example wasm_case -- file.tn entry out.wasm`
//! Compiles `entry` to the app module `out.wasm` (run it with the runtime module
//! `target/wasm32-unknown-unknown/wasm/tint_wasmrt.wasm`).
use tint_ir::typed::lower_program;
use tint_lexer::{collect_tokens, Lexer};
use tint_parser::Parser;
use tint_semantics::prelude::CheckerContext;
use tint_semantics::SemanticChecker;

fn main() {
    let mut args = std::env::args().skip(1);
    let (path, entry, out) = (args.next().unwrap(), args.next().unwrap(), args.next().unwrap());
    let source = std::fs::read_to_string(path).unwrap();
    let tokens = collect_tokens(&mut Lexer::new(&source));
    let program = Parser::new(tokens).parse_program().expect("parse");
    let ctx = CheckerContext { strict: true, ..Default::default() };
    let (errors, model) = SemanticChecker::new(ctx).check_with_model(&program);
    assert!(errors.is_empty(), "{errors:?}");
    let lowered = lower_program(&program, &model);
    assert!(lowered.errors.is_empty(), "{:?}", lowered.errors);
    let c = tint_wasmgen::compile(&lowered.module, &[&entry]).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(&out, &c.wasm).unwrap();
    eprintln!("{} bytes", c.wasm.len());
}
