//! `tint-native-case file.tn entry`: compiles the file natively and runs the
//! zero-argument function `entry`. Prints one line on stdout:
//!
//! - `OK <rendered result>`
//! - `ERR <message>` for parse, type and lowering errors
//! - `UNSUPPORTED <why>` when the native backend cannot compile the program
//!
//! A run-time trap prints `trap: <message>` on stderr and exits with 1.
//! Used by the differential conformance test, which needs a process per case
//! because a trap ends the process.

use tint_ir::typed::lower_program;
use tint_lexer::{collect_tokens, Lexer};
use tint_parser::Parser;
use tint_semantics::prelude::CheckerContext;
use tint_semantics::SemanticChecker;

fn main() {
    let mut args = std::env::args().skip(1);
    let (path, entry) = (args.next().expect("file"), args.next().expect("entry"));
    let source = std::fs::read_to_string(&path).expect("read");
    let tokens = collect_tokens(&mut Lexer::new(&source));
    let program = match Parser::new(tokens).parse_program() {
        Ok(p) => p,
        Err(e) => return println!("ERR parse error: {e:?}"),
    };
    let ctx = CheckerContext { strict: true, ..Default::default() };
    let (errors, model) = SemanticChecker::new(ctx).check_with_model(&program);
    if !errors.is_empty() {
        return println!("ERR type error: {errors:?}");
    }
    let lowered = lower_program(&program, &model);
    if !lowered.errors.is_empty() {
        let messages: Vec<String> = lowered.errors.iter().map(|e| e.to_string()).collect();
        return println!("ERR lowering error: {}", messages.join("; "));
    }
    let jit = match tint_codegen::compile_for(lowered.module, &[entry.as_str()]) {
        Ok(jit) => jit,
        Err(e) => return println!("UNSUPPORTED {e}"),
    };
    match jit.run_fn(&entry) {
        Ok(text) => println!("OK {text}"),
        Err(e) => println!("ERR {e}"),
    }
}
