// Shared runtime fixture. The source is split by feature so each `.tn` file
// contains complete, related declarations rather than numbered fragments.

use std::panic::{catch_unwind, AssertUnwindSafe};
use tint_lexer::{collect_tokens, Lexer};
use tint_parser::error::ParserError;
use tint_parser::Parser;
use tint_runtime::vm::TintVM;

pub const SOURCE: &str = concat!(
    include_str!("source/ui.tn"),
    include_str!("source/gpu-and-logic.tn"),
    include_str!("source/generics-and-data.tn"),
    include_str!("source/control-flow.tn"),
    include_str!("source/logic-and-helpers.tn"),
    include_str!("source/assignments-and-data.tn"),
    include_str!("source/all-features.tn"),
    include_str!("source/patterns-and-maps.tn")
);

/// Lexes, parses, and loads `SOURCE` into a fresh `TintVM`, panicking with
/// the same diagnostics the original single giant test printed (source
/// span for a parser error; a plain message if the VM panics while
/// loading). Every split-out test file calls this once to get its own VM
/// -- `TintVM` isn't `Clone`, and there's no cross-test state to share by
/// reusing one instance, so re-loading per test (cheap: this is a small
/// in-memory source string, not I/O) is simpler than threading a shared
/// instance through `OnceLock`/`Mutex` for no real benefit.
pub fn load_vm() -> TintVM {
    let tokens = collect_tokens(&mut Lexer::new(SOURCE));

    let mut parser = Parser::new(tokens);

    let program = match parser.parse_program() {
        Ok(p) => p,
        Err(e) => {
            if let Some(span) = match &e {
                ParserError::Message { span, .. } => Some(span),
                ParserError::Unexpected { span, .. } => Some(span),
            } {
                panic!(
                    "parser failed at {}..{} -> `{}`: {:?}",
                    span.start.offset,
                    span.end.offset,
                    &SOURCE[span.start.offset..span.end.offset],
                    e
                );
            }
            panic!("parser failed: {:?}", e);
        }
    };

    let mut vm = TintVM::new();

    let load = catch_unwind(AssertUnwindSafe(|| {
        vm.run_program(&program);
    }));

    if load.is_err() {
        panic!("VM panicked while loading the shared fixture source");
    }

    vm
}
