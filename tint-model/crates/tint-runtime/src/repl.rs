// repl.rs
//
// A stateful REPL session: unlike the throwaway VMs `check()`/`run()`/
// `render_ui()` (see tint-wasm/src/lib.rs) each build for one-off calls,
// every `eval()` here sees every earlier entry's definitions in the same
// session. There's no incremental-compile API on the parser/IR side --
// no way to hand the pipeline "just this one new fn" and have it merge
// into an already-running program -- so persistence works by re-parsing
// and re-running the WHOLE growing source from scratch on every call,
// rather than true incremental evaluation. Simple and correct; not fast
// for a very long session, which is an acceptable trade for a sandbox
// terminal panel.
//
// Every call is hardened with `catch_unwind`: an interactive REPL hits
// the IR VM's panic-on-runtime-error paths (undefined variable, wrong
// argument count, calling something that isn't a function, ...) as a
// normal, frequent event -- typos and half-finished expressions -- not a
// rare crash, so a panic here becomes an ordinary `error` result instead
// of poisoning or ending the whole session.

use std::panic::{catch_unwind, AssertUnwindSafe};

use tint_ast::{Item, Span};
use tint_lexer::{collect_tokens, Lexer};
use tint_parser::Parser;

use crate::vm::TintVM;
use tint_evaluator::EvalHost;

/// The result of evaluating one REPL entry.
pub struct ReplOutcome {
    /// True when `line` was a definition (fn/struct/enum/ui fn/...) that
    /// got committed to the session -- there's no `value` for these,
    /// just the fact that later entries can now use it.
    pub defined: bool,
    pub value: Option<String>,
    pub output: String,
    pub error: Option<String>,
}

pub struct ReplSession {
    /// Every earlier entry that was itself a definition, concatenated in
    /// the order they were accepted. A definition that failed to parse
    /// or run is never appended, so this always re-parses cleanly.
    definitions: String,
}

impl ReplSession {
    pub fn new() -> Self {
        Self {
            definitions: String::new(),
        }
    }

    /// Forgets every definition accepted so far.
    pub fn reset(&mut self) {
        self.definitions.clear();
    }

    /// Evaluates one entry against everything defined earlier in this
    /// session (see this module's doc comment for how "against" works).
    pub fn eval(&mut self, line: &str) -> ReplOutcome {
        let looks_like_item = ["fn ", "struct ", "enum ", "ui fn", "space ", "impl ", "use "]
            .iter()
            .any(|kw| line.trim_start().starts_with(kw));

        if looks_like_item {
            // A definition: try it appended to the session; only keep it
            // if the resulting program still parses and loads cleanly.
            let candidate = format!("{}\n{}\n", self.definitions, line);

            match run_source(&candidate, None) {
                Ok((_, output)) => {
                    self.definitions = candidate;
                    ReplOutcome {
                        defined: true,
                        value: None,
                        output,
                        error: None,
                    }
                }
                Err(error) => ReplOutcome {
                    defined: false,
                    value: None,
                    output: String::new(),
                    error: Some(error),
                },
            }
        } else {
            // A bare statement/expression: wrap it in a throwaway
            // function (so it doesn't need its own `fn` boilerplate) and
            // run it against the session's accumulated definitions,
            // without committing anything new to the session.
            let candidate = format!("{}\nfn __repl__() {{\n{}\n}}\n", self.definitions, line);

            match run_source(&candidate, Some("__repl__")) {
                Ok((value, output)) => ReplOutcome {
                    defined: false,
                    value,
                    output,
                    error: None,
                },
                Err(error) => ReplOutcome {
                    defined: false,
                    value: None,
                    output: String::new(),
                    error: Some(error),
                },
            }
        }
    }
}

impl Default for ReplSession {
    fn default() -> Self {
        Self::new()
    }
}

/// Parses and runs `source` in a fresh `TintVM`, calling `entry` with no
/// arguments if given and present. Returns the entry's result (as
/// `Display` text, if there was an entry to call) plus whatever
/// print()/dbg() captured (see `tint_evaluator::output`). Both parse
/// errors and IR-VM panics come back as `Err` instead of propagating.
fn run_source(source: &str, entry: Option<&str>) -> Result<(Option<String>, String), String> {
    let tokens = collect_tokens(&mut Lexer::new(source));
    let mut parser = Parser::new(tokens);
    let program = match parser.parse_program() {
        Ok(p) => p,
        Err(e) => return Err(format!("{:?}", e)),
    };

    let has_entry = entry.is_some_and(|name| {
        program
            .items
            .iter()
            .any(|item| matches!(item, Item::Fn(f) if f.name == name))
    });

    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let mut vm = TintVM::new();
        vm.run_program(&program);

        if has_entry {
            let entry = entry.expect("has_entry implies entry is Some");
            match vm.call_fn(entry, &[], Span::dummy()) {
                Ok(v) => Ok(Some(format!("{}", v))),
                Err(e) => Err(format!("{:?}", e)),
            }
        } else {
            Ok(None)
        }
    }));

    let value = match outcome {
        Ok(Ok(v)) => v,
        Ok(Err(e)) => return Err(e),
        Err(panic) => {
            let msg = panic
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| panic.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "unknown panic".to_string());
            return Err(format!("runtime panic: {}", msg));
        }
    };

    Ok((value, tint_evaluator::output::take_output()))
}
