//! Stateful UI sessions keep one VM alive so `state` values persist across
//! renders and event dispatches. Runtime errors are converted to `Result`s so
//! an authoring mistake does not bring down the host application.

use std::collections::HashMap;
use std::panic::{catch_unwind, AssertUnwindSafe};

use tint_ast::{Item, Program, Span};
use tint_evaluator::errors::EvalResult;
use tint_evaluator::{EvalHost, Value as EvalValue};
use tint_lexer::{collect_tokens, Lexer};
use tint_parser::Parser;
use tint_semantics::prelude::CheckerContext;
use tint_semantics::SemanticChecker;

use crate::ui::render::UiRenderNode;
use crate::vm::{HttpRequest, TintVM};

/// Name of the read-only variable `UiSession` keeps in scope for a
/// `ui fn` body to branch on structurally (an `if{}` directive, not
/// just a style modifier -- see ui/style.rs's mobile::{}/tablet::{}/
/// laptop::{}/desktop::{} for the style-only equivalent). Set once at
/// construction with a placeholder value and kept current by whatever
/// host embeds this session (see tint-wasm/src/dom.rs's
/// `sync_viewport_width`, which calls `set_viewport_width` right
/// before every render so a resize alone -- no click/hover needed --
/// can flip an `if{ viewport_width < 640 } { ... }` branch.
pub const VIEWPORT_WIDTH_VAR: &str = "viewport_width";

pub struct UiSession {
    vm: TintVM,
    ui_fn_name: String,
}

impl UiSession {
    /// Parses `source`, runs it (registering every `fn`/`ui fn`), and
    /// evaluates `ui_fn_name`'s `state` declarations once into this
    /// session's persistent scope. Fails if the source doesn't parse or
    /// no `ui fn` named `ui_fn_name` exists in it.
    pub fn new(source: &str, ui_fn_name: &str) -> Result<Self, String> {
        let tokens = collect_tokens(&mut Lexer::new(source));
        let mut parser = Parser::new(tokens);
        let program = parser.parse_program().map_err(|e| format!("{:?}", e))?;

        Self::from_program(program, ui_fn_name)
    }

    /// Builds a session from a serialized Tint program. This uses the same
    /// VM and state initialization path as source mode; only lexing/parsing
    /// is skipped.
    pub fn from_bytecode(bytes: &[u8], ui_fn_name: &str) -> Result<Self, String> {
        let program = crate::bytecode::decode(bytes)?;
        Self::from_program(program, ui_fn_name)
    }

    /// Builds a session from an already parsed program. Kept public so native
    /// build tools and the WASM bytecode host share exactly one initialization
    /// path.
    pub fn from_program(program: Program, ui_fn_name: &str) -> Result<Self, String> {
        let semantic_errors = SemanticChecker::new(CheckerContext::default()).check(&program);
        if !semantic_errors.is_empty() {
            return Err(format!("semantic errors: {:?}", semantic_errors));
        }

        let has_ui_fn = program
            .items
            .iter()
            .any(|item| matches!(item, Item::UiFn(f) if f.name == ui_fn_name));
        if !has_ui_fn {
            return Err(format!("Unknown ui fn '{}'", ui_fn_name));
        }

        let outcome = catch_unwind(AssertUnwindSafe(|| {
            let mut vm = TintVM::new();
            vm.run_program(&program);

            // `run_program` may already have auto-mounted this same
            // `ui fn` (if it's literally named App/Main) before any state
            // existed -- harmless, that throwaway tree is discarded, and
            // every real render happens through `render()`/`dispatch()`
            // below, after state is bound.
            let state_decls = vm
                .ui_functions
                .get(ui_fn_name)
                .map(|f| f.state.clone())
                .unwrap_or_default();

            for decl in &state_decls {
                let v = vm.eval_expr(&decl.init);
                vm.define_var(&decl.name, v);
            }

            vm.define_var(VIEWPORT_WIDTH_VAR, EvalValue::Number(1440.0));
            // `theme::dark { ... }` / `theme::light { ... }` selects from
            // this ordinary state value. Applications can change it from a
            // click handler just like any other state variable.
            if matches!(vm.load_var("theme", Span::dummy()), EvalValue::Unit) {
                vm.define_var("theme", EvalValue::String("dark".to_string()));
            }

            vm
        }));

        match outcome {
            Ok(vm) => Ok(Self {
                vm,
                ui_fn_name: ui_fn_name.to_string(),
            }),
            Err(panic) => Err(panic_message(panic)),
        }
    }

    /// Updates the `viewport_width` variable an `if{}` directive in
    /// the .tint source can read (see `VIEWPORT_WIDTH_VAR`'s doc
    /// comment). Takes effect on the next `render()`/`dispatch()` --
    /// call this first if the host wants that render to reflect a new
    /// width, same convention as setting a `state` var before reading
    /// it back out of the rebuilt tree.
    pub fn set_viewport_width(&mut self, width: f64) {
        self.vm
            .define_var(VIEWPORT_WIDTH_VAR, EvalValue::Number(width));
    }

    /// Builds and returns the session's current tree.
    pub fn render(&mut self) -> Result<Vec<UiRenderNode>, String> {
        let vm = &mut self.vm;
        let name = &self.ui_fn_name;
        let outcome = catch_unwind(AssertUnwindSafe(|| vm.render_ui_fn(name, &[])));
        unwrap_outcome(outcome)
    }

    /// Runs `handler` (a plain `fn`, no args) against this session's
    /// persistent state, then rebuilds and returns the tree -- see this
    /// module's doc comment for why that needs `call_user_fn` and a
    /// long-lived `TintVM`, not a fresh one per call.
    pub fn dispatch(&mut self, handler: &str) -> Result<Vec<UiRenderNode>, String> {
        self.dispatch_with_args(handler, &[])
    }

    /// Dispatches a host event with runtime values, used by `frame||handler`
    /// to pass elapsed seconds without making the browser part of Tint logic.
    pub fn dispatch_with_args(
        &mut self,
        handler: &str,
        args: &[EvalValue],
    ) -> Result<Vec<UiRenderNode>, String> {
        let vm = &mut self.vm;
        let name = &self.ui_fn_name;
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            vm.call_user_fn(handler, args, Span::dummy())?;
            vm.render_ui_fn(name, &[])
        }));
        unwrap_outcome(outcome)
    }

    /// Returns the session's host storage so a browser can persist it in
    /// localStorage (or another host-specific store).
    pub fn storage_snapshot(&self) -> HashMap<String, String> {
        self.vm.storage_snapshot()
    }

    /// Hydrates host storage before the next render/dispatch.
    pub fn hydrate_storage(&mut self, values: HashMap<String, String>) {
        self.vm.hydrate_storage(values);
    }

    /// Takes HTTP work queued by `http_get(url)`. The host owns fetch,
    /// credentials, CORS and cancellation; Tint receives completion through
    /// `dispatch_with_args`.
    pub fn take_http_requests(&mut self) -> Vec<HttpRequest> {
        self.vm.take_http_requests()
    }
}

fn unwrap_outcome(
    outcome: std::thread::Result<EvalResult<Vec<UiRenderNode>>>,
) -> Result<Vec<UiRenderNode>, String> {
    match outcome {
        Ok(Ok(nodes)) => Ok(nodes),
        Ok(Err(e)) => Err(format!("{:?}", e)),
        Err(panic) => Err(panic_message(panic)),
    }
}

fn panic_message(panic: Box<dyn std::any::Any + Send>) -> String {
    let msg = panic
        .downcast_ref::<&str>()
        .map(|s| s.to_string())
        .or_else(|| panic.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "unknown panic".to_string());
    format!("runtime panic: {}", msg)
}
