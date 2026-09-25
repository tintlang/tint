// rune-wasm/src/lib.rs
//
// Step B of the sandbox plan: a thin wasm-bindgen bridge exposing the
// SAME lexer -> parser -> RuneVM pipeline rune-cli drives from the
// command line, but callable from JS in a browser. This crate holds no
// language logic of its own -- it only translates between JS and the
// existing rune-* crates, mirroring rune-cli/src/main.rs's cmd_check/
// cmd_run.
//
// Two entry points:
//   check(source)       -- lex + parse only, report ok/error + span.
//   run(source, entry)  -- parse, compile to IR, run `entry` (no args),
//                           report the result value, any error, and
//                           whatever the program printed via print()/dbg()
//                           (captured through rune_evaluator::output,
//                           since there is no real stdout in a browser).
//
// Both return a JS object (via serde-wasm-bindgen) rather than throwing,
// so the sandbox UI can always render *something* -- a value, an error
// with a span to highlight, or both.

use serde::Serialize;
use wasm_bindgen::prelude::*;

use rune_ast::{Item, Program, Span};
use rune_evaluator::EvalHost;
use rune_lexer::{collect_tokens, Lexer};
use rune_parser::error::ParserError;
use rune_parser::Parser;
use rune_runtime::vm::RuneVM;

#[derive(Serialize)]
pub struct SpanInfo {
    pub start_line: usize,
    pub start_col: usize,
    pub end_line: usize,
    pub end_col: usize,
}

#[derive(Serialize, Default)]
pub struct RunResult {
    pub ok: bool,
    pub item_count: usize,
    pub value: Option<String>,
    pub output: String,
    pub error: Option<String>,
    pub error_span: Option<SpanInfo>,
}

fn parse_source(code: &str) -> Result<Program, ParserError> {
    let tokens = collect_tokens(&mut Lexer::new(code));
    let mut parser = Parser::new(tokens);
    parser.parse_program()
}

fn span_of(e: &ParserError) -> Span {
    match e {
        ParserError::Message { span, .. } => *span,
        ParserError::Unexpected { span, .. } => *span,
    }
}

fn to_span_info(span: Span) -> SpanInfo {
    SpanInfo {
        start_line: span.start.line,
        start_col: span.start.column,
        end_line: span.end.line,
        end_col: span.end.column,
    }
}

fn has_fn(program: &Program, name: &str) -> bool {
    program
        .items
        .iter()
        .any(|item| matches!(item, Item::Fn(f) if f.name == name))
}

/// Lex + parse `source` only. Mirrors `rune check`.
#[wasm_bindgen]
pub fn check(source: &str) -> JsValue {
    let mut result = RunResult::default();
    match parse_source(source) {
        Ok(program) => {
            result.ok = true;
            result.item_count = program.items.len();
        }
        Err(e) => {
            result.error_span = Some(to_span_info(span_of(&e)));
            result.error = Some(format!("{:?}", e));
        }
    }
    serde_wasm_bindgen::to_value(&result).unwrap_or(JsValue::NULL)
}

/// Parse, compile to IR, and call `entry` with no arguments. Mirrors
/// `rune run <file> [entry]`. Any output from print()/dbg() inside the
/// program is captured and returned in `output` rather than lost, since
/// there is no real stdout to write to here.
#[wasm_bindgen]
pub fn run(source: &str, entry: &str) -> JsValue {
    let mut result = RunResult::default();

    let program = match parse_source(source) {
        Ok(p) => p,
        Err(e) => {
            result.error_span = Some(to_span_info(span_of(&e)));
            result.error = Some(format!("{:?}", e));
            return serde_wasm_bindgen::to_value(&result).unwrap_or(JsValue::NULL);
        }
    };
    result.item_count = program.items.len();

    let mut vm = RuneVM::new();
    vm.run_program(&program);

    if !has_fn(&program, entry) {
        // Compiled fine, nothing to call -- not an error (e.g. a file
        // that only declares structs/enums, or a typo'd entry name the
        // sandbox should surface as "nothing ran", not a crash).
        result.ok = true;
        result.output = rune_evaluator::output::take_output();
        return serde_wasm_bindgen::to_value(&result).unwrap_or(JsValue::NULL);
    }

    match vm.call_fn(entry, &[], Span::dummy()) {
        Ok(v) => {
            result.ok = true;
            result.value = Some(format!("{}", v));
        }
        Err(e) => {
            result.error = Some(format!("{:?}", e));
        }
    }
    result.output = rune_evaluator::output::take_output();
    serde_wasm_bindgen::to_value(&result).unwrap_or(JsValue::NULL)
}

#[derive(Serialize, Default)]
pub struct UiRunResult {
    pub ok: bool,
    pub item_count: usize,
    pub tree: Vec<rune_runtime::ui::render::UiRenderNode>,
    pub error: Option<String>,
    pub error_span: Option<SpanInfo>,
}

/// Parses `source`, runs it, and calls `ui_fn_name` (a `ui fn`, not a
/// plain `fn`) with NO arguments -- same no-arg-only limitation as
/// `run()`/`rune-cli`'s `run` command, for the same reason (no CLI/JS-side
/// argument marshalling yet). Returns the evaluated render tree: tags
/// with resolved text, if{}/for{} directives actually applied, and a
/// best-effort CSS `style` list translated from Rune's modifiers (see
/// rune_runtime::ui::render's module doc comment for exactly what's
/// covered). The sandbox turns this into real DOM elements.
#[wasm_bindgen]
pub fn render_ui(source: &str, ui_fn_name: &str) -> JsValue {
    let mut result = UiRunResult::default();

    let program = match parse_source(source) {
        Ok(p) => p,
        Err(e) => {
            result.error_span = Some(to_span_info(span_of(&e)));
            result.error = Some(format!("{:?}", e));
            return serde_wasm_bindgen::to_value(&result).unwrap_or(JsValue::NULL);
        }
    };
    result.item_count = program.items.len();

    let mut vm = RuneVM::new();
    vm.run_program(&program);

    match vm.render_ui_fn(ui_fn_name, &[]) {
        Ok(tree) => {
            result.ok = true;
            result.tree = tree;
        }
        Err(e) => {
            result.error = Some(format!("{:?}", e));
        }
    }
    serde_wasm_bindgen::to_value(&result).unwrap_or(JsValue::NULL)
}

#[wasm_bindgen]
pub fn rune_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// One entry's worth of `RuneRepl::eval()` output.
#[derive(Serialize, Default)]
pub struct ReplLineResult {
    /// True if `line` was a definition (fn/struct/enum/...) that got
    /// committed to the session -- no `value`, just persisted for later
    /// entries to use.
    pub defined: bool,
    pub value: Option<String>,
    pub output: String,
    pub error: Option<String>,
}

/// A stateful REPL session for the sandbox's terminal panel, unlike
/// `check()`/`run()`/`render_ui()` above which each construct a fresh,
/// throwaway `RuneVM` (see this crate's other functions' doc comments and
/// sandbox/README.md's "Known limitations"). Backed by
/// `rune_runtime::repl::ReplSession` -- see that module's doc comment for
/// exactly how persistence works (re-parse-and-rerun-everything, not true
/// incrementality) and why every call is panic-hardened (an interactive
/// REPL hits the IR VM's panic-on-runtime-error paths, e.g. calling an
/// undefined function, as a normal/frequent case, not a rare one).
#[wasm_bindgen]
pub struct RuneRepl {
    session: rune_runtime::repl::ReplSession,
}

#[wasm_bindgen]
impl RuneRepl {
    #[wasm_bindgen(constructor)]
    pub fn new() -> RuneRepl {
        RuneRepl {
            session: rune_runtime::repl::ReplSession::new(),
        }
    }

    /// Evaluate one line/entry against everything defined earlier in this
    /// session.
    pub fn eval(&mut self, line: &str) -> JsValue {
        let outcome = self.session.eval(line);
        let result = ReplLineResult {
            defined: outcome.defined,
            value: outcome.value,
            output: outcome.output,
            error: outcome.error,
        };
        serde_wasm_bindgen::to_value(&result).unwrap_or(JsValue::NULL)
    }

    /// Forget every definition made in this session so far.
    pub fn reset(&mut self) {
        self.session.reset();
    }
}

impl Default for RuneRepl {
    fn default() -> Self {
        Self::new()
    }
}
