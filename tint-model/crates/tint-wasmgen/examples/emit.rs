//! `cargo run --release -p tint-wasmgen --example emit -- prog.tn out.wasm [entry]`
//! Compiles a program's entry function to a wasm app module (for the runtime module `tint_wasmrt.wasm`).
use tint_ir::typed::lower_program;
use tint_lexer::{collect_tokens, Lexer};
use tint_parser::Parser;
use tint_semantics::prelude::CheckerContext;
use tint_semantics::SemanticChecker;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let source = std::fs::read_to_string(&args[1]).unwrap();
    let entry = args.get(3).map(String::as_str).unwrap_or("main");
    let tokens = collect_tokens(&mut Lexer::new(&source));
    let program = Parser::new(tokens).parse_program().expect("parse");
    let (errors, model) = SemanticChecker::new(CheckerContext { strict: true, ..Default::default() }).check_with_model(&program);
    assert!(errors.is_empty(), "{errors:?}");
    let lowered = lower_program(&program, &model);
    assert!(lowered.errors.is_empty(), "{:?}", lowered.errors);
    if std::env::var("DUMP").is_ok() {
        eprintln!("{}", lowered.module);
    }
    let compiled = tint_wasmgen::compile(&lowered.module, &[entry]).expect("compile");
    std::fs::write(&args[2], &compiled.wasm).unwrap();
}
