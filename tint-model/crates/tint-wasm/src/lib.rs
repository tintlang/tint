// tint-wasm/src/lib.rs
//
// Step B of the sandbox plan: a thin wasm-bindgen bridge exposing the
// SAME lexer -> parser -> TintVM pipeline tint-cli drives from the
// command line, but callable from JS in a browser. This crate holds no
// language logic of its own -- it only translates between JS and the
// existing tint-* crates, mirroring tint-cli/src/main.rs's cmd_check/
// cmd_run.
//
// Two entry points:
//   check(source)       -- lex + parse only, report ok/error + span.
//   run(source, entry)  -- parse, compile to IR, run `entry` (no args),
//                           report the result value, any error, and
//                           whatever the program printed via print()/dbg()
//                           (captured through tint_evaluator::output,
//                           since there is no real stdout in a browser).
//
// Both return a JS object (via serde-wasm-bindgen) rather than throwing,
// so the sandbox UI can always render *something* -- a value, an error
// with a span to highlight, or both.

use serde::Serialize;
use wasm_bindgen::prelude::*;

mod dom;
pub use dom::DomSession;

use tint_ast::{Item, Program, Span};
use tint_evaluator::EvalHost;
use tint_lexer::{collect_tokens, Lexer};
use tint_parser::error::ParserError;
use tint_parser::Parser;
use tint_runtime::vm::TintVM;

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

/// Lex + parse `source` only. Mirrors `tint check`.
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
/// `tint run <file> [entry]`. Any output from print()/dbg() inside the
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

    let mut vm = TintVM::new();
    vm.run_program(&program);

    if !has_fn(&program, entry) {
        // Compiled fine, nothing to call -- not an error (e.g. a file
        // that only declares structs/enums, or a typo'd entry name the
        // sandbox should surface as "nothing ran", not a crash).
        result.ok = true;
        result.output = tint_evaluator::output::take_output();
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
    result.output = tint_evaluator::output::take_output();
    serde_wasm_bindgen::to_value(&result).unwrap_or(JsValue::NULL)
}

#[derive(Serialize, Default)]
pub struct UiRunResult {
    pub ok: bool,
    pub item_count: usize,
    pub tree: Vec<tint_runtime::ui::render::UiRenderNode>,
    pub error: Option<String>,
    pub error_span: Option<SpanInfo>,
}

/// Parses `source`, runs it, and calls `ui_fn_name` (a `ui fn`, not a
/// plain `fn`) with NO arguments -- same no-arg-only limitation as
/// `run()`/`tint-cli`'s `run` command, for the same reason (no CLI/JS-side
/// argument marshalling yet). Returns the evaluated render tree: tags
/// with resolved text, if{}/for{} directives actually applied, and a
/// best-effort CSS `style` list translated from Tint's modifiers (see
/// tint_runtime::ui::render's module doc comment for exactly what's
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

    let mut vm = TintVM::new();
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
pub fn tint_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// One entry's worth of `TintRepl::eval()` output.
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
/// throwaway `TintVM` (see this crate's other functions' doc comments and
/// sandbox/README.md's "Known limitations"). Backed by
/// `tint_runtime::repl::ReplSession` -- see that module's doc comment for
/// exactly how persistence works (re-parse-and-rerun-everything, not true
/// incrementality) and why every call is panic-hardened (an interactive
/// REPL hits the IR VM's panic-on-runtime-error paths, e.g. calling an
/// undefined function, as a normal/frequent case, not a rare one).
#[wasm_bindgen]
pub struct TintRepl {
    session: tint_runtime::repl::ReplSession,
}

#[wasm_bindgen]
impl TintRepl {
    #[wasm_bindgen(constructor)]
    pub fn new() -> TintRepl {
        TintRepl {
            session: tint_runtime::repl::ReplSession::new(),
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

impl Default for TintRepl {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Serialize, Default)]
pub struct UiSessionResult {
    pub ok: bool,
    pub tree: Vec<tint_runtime::ui::render::UiRenderNode>,
    pub error: Option<String>,
}

/// A stateful UI session for the sandbox's preview pane, backing real
/// `click||`/`hover_in||`/`hover_out||` interactivity -- unlike
/// `render_ui()` above, which builds a fresh, throwaway `TintVM` on every
/// call (fine for "re-render on every keystroke", useless for "remember
/// that the popover is open"), this wraps a single
/// `tint_runtime::ui_session::UiSession` that stays alive for the
/// session's lifetime. See that module's doc comment for exactly how
/// `state` persistence and handler dispatch work under here.
///
/// Construction can fail (bad source, unknown `ui_fn_name`) without a JS
/// exception -- the failure is stored and reported from `tree()`/
/// `dispatch()` instead, so `new UiSession(...)` itself never throws.
#[wasm_bindgen]
pub struct UiSession {
    inner: Option<tint_runtime::ui_session::UiSession>,
    init_error: Option<String>,
}

#[wasm_bindgen]
impl UiSession {
    #[wasm_bindgen(constructor)]
    pub fn new(source: &str, ui_fn_name: &str) -> UiSession {
        match tint_runtime::ui_session::UiSession::new(source, ui_fn_name) {
            Ok(session) => UiSession {
                inner: Some(session),
                init_error: None,
            },
            Err(e) => UiSession {
                inner: None,
                init_error: Some(e),
            },
        }
    }

    /// The session's current tree (a fresh build, not cached).
    pub fn tree(&mut self) -> JsValue {
        let mut result = UiSessionResult::default();
        match &mut self.inner {
            None => result.error = self.init_error.clone(),
            Some(session) => match session.render() {
                Ok(tree) => {
                    result.ok = true;
                    result.tree = tree;
                }
                Err(e) => result.error = Some(e),
            },
        }
        serde_wasm_bindgen::to_value(&result).unwrap_or(JsValue::NULL)
    }

    /// Points this SAME `UiSession` at different source -- re-parsing
    /// `source` as `ui_fn_name` into a brand-new inner session (this
    /// resets `state`, exactly like constructing a new session would)
    /// and returning the freshly-rendered tree in the same shape as
    /// `tree()`/`dispatch()`. Mirrors `DomSession::reload` for this
    /// JS-tree-consuming path: the sandbox's live editor calls this on
    /// every debounced edit instead of throwing away and reconstructing
    /// the whole `UiSession` (and losing the ability to report a
    /// bad-source error the same way `tree()` does) per keystroke.
    pub fn reload(&mut self, source: &str, ui_fn_name: &str) -> JsValue {
        match tint_runtime::ui_session::UiSession::new(source, ui_fn_name) {
            Ok(session) => {
                self.inner = Some(session);
                self.init_error = None;
            }
            Err(e) => {
                self.inner = None;
                self.init_error = Some(e);
            }
        }
        self.tree()
    }

    /// Runs `handler` (a plain `fn`, no args) against this session's
    /// persistent state, then returns the rebuilt tree -- see
    /// `tint_runtime::ui_session`'s doc comment for why this needs a
    /// real, persistent session rather than a fresh `render_ui()` call.
    pub fn dispatch(&mut self, handler: &str) -> JsValue {
        let mut result = UiSessionResult::default();
        match &mut self.inner {
            None => result.error = self.init_error.clone(),
            Some(session) => match session.dispatch(handler) {
                Ok(tree) => {
                    result.ok = true;
                    result.tree = tree;
                }
                Err(e) => result.error = Some(e),
            },
        }
        serde_wasm_bindgen::to_value(&result).unwrap_or(JsValue::NULL)
    }
}
