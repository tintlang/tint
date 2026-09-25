// rune-runtime/src/repl.rs
//
// rune-cli's cmd_repl (crates/rune-cli/src/main.rs) documents this exact
// gap: "Each entry compiles and runs in a brand-new RuneVM ... A
// persistent REPL would need to keep accumulating source across entries
// and re-run the growing program each time, since there's no
// incremental-compile API yet -- doable, but a bigger step than this
// first pass". This module is that bigger step, factored into
// rune-runtime (rather than duplicated per-frontend) so both a future
// CLI REPL and rune-wasm's browser terminal can share it.
//
// Design: re-parse-and-rerun-everything, not true incrementality. Each
// `eval()` call re-parses+re-runs the ACCUMULATED source of every
// previously-COMMITTED item-level entry (fn/struct/enum/ui fn/...), plus
// the new entry:
//
//   - An item-level entry (starts with `fn`/`struct`/`enum`/`ui fn`/
//     `space`/`impl`) is validated against the accumulated source first;
//     only on success is it appended to `accumulated`, so a bad
//     definition can't corrupt the session. It is not auto-executed --
//     `defined: true` comes back with no value. Deliberately NOT checked
//     for undefined calls at this point (see find_undefined_call below) --
//     that would break forward/mutual references (`fn a() { b() }` then
//     later `fn b() { a() }`), which are fine once both are committed.
//   - Anything else is treated as a statement/expression body, wrapped in
//     a throwaway `fn __repl_line__()` appended AFTER the accumulated
//     source (so it can see every earlier definition), evaluated once,
//     and NOT committed -- `2 + 2` doesn't pollute the session, but a
//     `fn`/`struct` two lines later still sees anything committed so
//     far.
//
// Fine for a REPL: sessions are short and programs are small, and
// re-parsing the whole accumulated buffer on every entry is cheap
// relative to human typing speed.
//
// Panic hardening: the IR VM (rune-ir/src/ir_vm.rs) panics rather than
// returning a Result for most runtime errors (unknown function, index out
// of bounds, no match arm, ...) -- a systemic design choice across that
// whole file, not something this module can or should fix wholesale.
// That's mostly invisible in `rune-cli run`/`rune-wasm run()` today
// because each click gets a fresh, immediately-discarded VM. A REPL is
// different: it's interactive, so hitting one of these is the NORMAL
// case (a typo calling an undefined function is the single most common
// REPL mistake there is), not a rare edge case, and a REPL session is
// long-lived state the user keeps building on.
//
// `catch_unwind` (below) handles this on NATIVE targets. It does
// nothing on wasm32-unknown-unknown: that target compiles with
// panic="abort" by default (confirmed empirically -- see this module's
// test and crates/rune-wasm's Node-based check), so a Rust panic there is
// an unrecoverable `unreachable` trap that aborts the whole wasm
// instance, not something `catch_unwind` can intercept. So for the one
// panic source a REPL user will hit constantly -- calling a function that
// doesn't exist -- `find_undefined_call` statically validates the WHOLE
// compiled program (every function's every instruction, including
// match-arm sub-blocks) against the real "Call to unknown function" panic
// site in `IrVM::call_function`, and `eval()` refuses to execute anything
// until that comes back clean. This is intentionally scoped to that one
// failure mode, which observably covers the common case; the other panic
// sites listed above (index out of bounds, non-exhaustive match, ...)
// are NOT statically checked and will still hard-crash the wasm module
// if hit (see sandbox/README.md's known limitations) -- fully closing
// that gap means making ir_vm.rs's whole instruction-execution path
// Result-returning, a much larger change than this REPL feature needs.

use std::collections::HashSet;
use std::panic::{self, AssertUnwindSafe};

use rune_ast::{Item, Program, Span};
use rune_evaluator::EvalHost;
use rune_ir::ir::{CallTarget, Instr};
use rune_lexer::{collect_tokens, Lexer};
use rune_parser::error::ParserError;
use rune_parser::Parser;

use crate::vm::RuneVM;

/// Result of one `ReplSession::eval` call.
pub struct ReplOutcome {
    /// True if this entry was an item-level definition that got committed
    /// to the session (no value to show, just "defined").
    pub defined: bool,
    pub value: Option<String>,
    pub output: String,
    pub error: Option<String>,
}

#[derive(Default)]
pub struct ReplSession {
    accumulated: String,
}

const ENTRY_NAME: &str = "__repl_line__";

// Mirrors IrVM::call_function's builtin list exactly (rune-ir/src/ir_vm.rs)
// -- these are called by name from compiled IR but have no corresponding
// FunctionIR entry, since they aren't user `fn` declarations.
const BUILTINS: &[&str] = &["print", "dbg", "sqrt"];

fn parse_source(code: &str) -> Result<Program, ParserError> {
    let tokens = collect_tokens(&mut Lexer::new(code));
    let mut parser = Parser::new(tokens);
    parser.parse_program()
}

fn has_fn(program: &Program, name: &str) -> bool {
    program
        .items
        .iter()
        .any(|item| matches!(item, Item::Fn(f) if f.name == name))
}

// Mirrors rune-cli's cmd_repl `looks_like_item` check exactly.
fn looks_like_item(src: &str) -> bool {
    ["fn ", "struct ", "enum ", "ui fn", "space ", "impl "]
        .iter()
        .any(|kw| src.trim_start().starts_with(kw))
}

fn err_outcome(e: &ParserError) -> ReplOutcome {
    ReplOutcome {
        defined: false,
        value: None,
        output: String::new(),
        error: Some(format!("{:?}", e)),
    }
}

fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic (no message)".to_string()
    }
}

/// Run `f` (some operation on `vm`) catching any panic instead of letting
/// it unwind out of this module. Meaningful on native targets only -- see
/// this module's doc comment for why it's a no-op safety net on wasm32.
/// Safe here specifically because callers always construct a fresh,
/// immediately-discarded `RuneVM` per `eval()` call -- so even if a caught
/// panic leaves that VM's own internal state inconsistent, nothing reuses
/// it afterward.
fn catch_vm_panic<T>(f: impl FnOnce() -> T) -> Result<T, String> {
    panic::catch_unwind(AssertUnwindSafe(f)).map_err(|payload| panic_message(payload.as_ref()))
}

fn collect_calls_in_block(block: &rune_ir::ir::Block, out: &mut Vec<String>) {
    for instr in &block.instrs {
        collect_calls_in_instr(instr, out);
    }
}

fn collect_calls_in_instr(instr: &Instr, out: &mut Vec<String>) {
    match instr {
        Instr::Call { func: CallTarget::Named(name), .. } => out.push(name.clone()),
        Instr::Match { arms, .. } => {
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    collect_calls_in_block(guard, out);
                }
                collect_calls_in_block(&arm.body, out);
            }
        }
        _ => {}
    }
}

/// Statically finds the first call to a function that doesn't exist
/// anywhere in the compiled program (and isn't a builtin) -- exactly the
/// condition `IrVM::call_function` would otherwise panic on. Scans every
/// instruction of every function, including nested match-arm sub-blocks
/// (those aren't part of `FunctionIR.blocks` -- see ir_vm.rs's comment on
/// why -- so they need their own recursion).
fn find_undefined_call(program: &rune_ir::ProgramIR) -> Option<String> {
    let defined: HashSet<&str> = program.functions.iter().map(|f| f.name.as_str()).collect();

    let mut called = Vec::new();
    for func in &program.functions {
        for block in &func.blocks {
            collect_calls_in_block(block, &mut called);
        }
    }

    called
        .into_iter()
        .find(|name| !defined.contains(name.as_str()) && !BUILTINS.contains(&name.as_str()))
}

impl ReplSession {
    pub fn new() -> Self {
        Self::default()
    }

    /// Forget everything defined in this session so far (equivalent to
    /// starting a brand-new terminal).
    pub fn reset(&mut self) {
        self.accumulated.clear();
    }

    pub fn eval(&mut self, line: &str) -> ReplOutcome {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return ReplOutcome {
                defined: false,
                value: None,
                output: String::new(),
                error: None,
            };
        }

        if looks_like_item(trimmed) {
            let candidate = format!("{}\n{}\n", self.accumulated, line);
            let program = match parse_source(&candidate) {
                Ok(p) => p,
                Err(e) => return err_outcome(&e),
            };

            let mut vm = RuneVM::new();
            let ran = catch_vm_panic(AssertUnwindSafe(|| vm.run_program(&program)));
            let _ = rune_evaluator::output::take_output(); // defensive: drain, definitions shouldn't print

            match ran {
                Ok(()) => {
                    self.accumulated = candidate;
                    ReplOutcome {
                        defined: true,
                        value: None,
                        output: String::new(),
                        error: None,
                    }
                }
                Err(msg) => ReplOutcome {
                    defined: false,
                    value: None,
                    output: String::new(),
                    error: Some(format!("runtime panic while defining: {}", msg)),
                },
            }
        } else {
            let source = format!("{}\nfn {}() {{\n{}\n}}\n", self.accumulated, ENTRY_NAME, line);
            let program = match parse_source(&source) {
                Ok(p) => p,
                Err(e) => return err_outcome(&e),
            };

            let mut vm = RuneVM::new();
            let ran = catch_vm_panic(AssertUnwindSafe(|| vm.run_program(&program)));
            if let Err(msg) = ran {
                let output = rune_evaluator::output::take_output();
                return ReplOutcome {
                    defined: false,
                    value: None,
                    output,
                    error: Some(format!("runtime panic: {}", msg)),
                };
            }

            if !has_fn(&program, ENTRY_NAME) {
                let output = rune_evaluator::output::take_output();
                return ReplOutcome {
                    defined: false,
                    value: None,
                    output,
                    error: Some("internal: repl entry not found after parsing".to_string()),
                };
            }

            // The one panic source a REPL hits constantly (see module doc
            // comment) -- checked BEFORE ever calling into the VM, since
            // on wasm32 there is no catching it once it happens.
            if let Some(name) = find_undefined_call(&vm.ir_program) {
                let output = rune_evaluator::output::take_output();
                return ReplOutcome {
                    defined: false,
                    value: None,
                    output,
                    error: Some(format!("undefined function `{}`", name)),
                };
            }

            let call_result = catch_vm_panic(AssertUnwindSafe(|| {
                vm.call_fn(ENTRY_NAME, &[], Span::dummy())
            }));
            let output = rune_evaluator::output::take_output();

            match call_result {
                Ok(Ok(v)) => ReplOutcome {
                    defined: false,
                    value: Some(format!("{}", v)),
                    output,
                    error: None,
                },
                Ok(Err(e)) => ReplOutcome {
                    defined: false,
                    value: None,
                    output,
                    error: Some(format!("{:?}", e)),
                },
                Err(msg) => ReplOutcome {
                    defined: false,
                    value: None,
                    output,
                    error: Some(format!("runtime panic: {}", msg)),
                },
            }
        }
    }
}
