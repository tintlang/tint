// Shared fixture for the split-out `AllFeatures`/pattern-matching test
// suite (see `tests/main.rs`, which just declares `mod` for the files in
// this directory). This one giant Tint source string is the SAME text
// that used to live inline inside the single `test_tint_capabilities`
// function in `tests/main.rs` before the split -- copied verbatim, not
// re-derived, so nothing here changes what source is actually exercised.
//
// It intentionally covers roughly eighty different, mostly-unrelated
// language constructs in one blob: UI parsing, GPU kernels, borrow
// blocks, generics, control flow, pattern matching, map literals, and
// more. Most of that is PARSER/LOADER-acceptance coverage only -- a lot
// of it (GPU kernels, borrow blocks, several of the generic-fn shapes)
// isn't wired up to real runtime semantics yet, so there's nothing
// meaningful to assert about what running those functions actually
// computes. `parses_and_loads.rs` covers that half honestly, as a smoke
// test: the whole source lexes, parses, and loads into a VM without
// panicking. `pattern_matching.rs` is the other half -- the handful of
// functions in here (`AllFeatures`, `TestMatchNumbers`, `TestMatchTuple`,
// `TestMatchStruct`, `TestGuard`) that DO compute something checkable,
// upgraded from a `println!` of whatever came out to a real `assert_eq!`
// against a hand-verified expected value.

use std::panic::{catch_unwind, AssertUnwindSafe};
use tint_lexer::{collect_tokens, Lexer};
use tint_parser::error::ParserError;
use tint_parser::Parser;
use tint_runtime::vm::TintVM;

pub const SOURCE: &str = concat!(
    include_str!("source/part1.tn"),
    include_str!("source/part2.tn"),
    include_str!("source/part3.tn"),
    include_str!("source/part4.tn"),
    include_str!("source/part5.tn"),
    include_str!("source/part6.tn")
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
