//! `cargo run --release -p tint-codegen --example jit_run -- file.tn`
use std::time::Instant;
use tint_ir::typed::lower_program;
use tint_lexer::{collect_tokens, Lexer};
use tint_parser::Parser;
use tint_semantics::prelude::CheckerContext;
use tint_semantics::SemanticChecker;

fn main() {
    let path = std::env::args().nth(1).expect("usage: jit_run file.tn");
    let source = std::fs::read_to_string(&path).unwrap();
    let tokens = collect_tokens(&mut Lexer::new(&source));
    let program = Parser::new(tokens).parse_program().expect("parse");
    let ctx = CheckerContext { strict: true, ..Default::default() };
    let (errors, model) = SemanticChecker::new(ctx).check_with_model(&program);
    if !errors.is_empty() {
        eprintln!("type errors: {errors:?}");
        std::process::exit(2);
    }
    let lowered = lower_program(&program, &model);
    if !lowered.errors.is_empty() {
        eprintln!("lowering errors: {:?}", lowered.errors);
        std::process::exit(2);
    }
    if std::env::var_os("TINT_DUMP").is_some() {
        for f in &lowered.module.funcs {
            eprintln!("{}", tint_ir::typed::display::show_func(&lowered.module, f));
        }
    }
    let t = Instant::now();
    let jit = match tint_codegen::compile(lowered.module) {
        Ok(j) => j,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(3);
        }
    };
    let compile = t.elapsed();
    let t = Instant::now();
    let res = jit.run_main();
    if std::env::var("TINT_TIME").is_ok() {
        eprintln!("compile {compile:?}, run {:?}, obj copies {}", t.elapsed(), tint_codegen::rt::COPIES.load(std::sync::atomic::Ordering::Relaxed));
    }
    if let Err(e) = res {
        eprintln!("{e}");
        std::process::exit(3);
    }
}
