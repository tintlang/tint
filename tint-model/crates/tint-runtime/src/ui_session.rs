// ui_session.rs
//
// A stateful UI session: the sandbox's click/hover round-trip needs a
// `ui fn`'s `state <ident> = <expr>` declarations (see UiStateDecl in
// tint-ast) to actually persist across calls -- build the tree once, a
// click handler mutates one, the tree is rebuilt reflecting the new
// value. Unlike `ReplSession` (see repl.rs), which is deliberately
// stateless-by-reconstruction (its "persistence" is re-parsing and
// re-running a growing block of SOURCE TEXT from scratch every call), a
// `UiSession` keeps one real `TintVM` alive for its whole lifetime,
// because UI state genuinely needs to live as runtime state, not as more
// source text -- there's nothing to re-parse, only a value to remember.
//
// `state` declarations are bound once, at construction, into the VM's
// outermost variable scope (`RuntimeScopeStack` starts with exactly one
// frame -- see scope.rs). Every later `render()`/`dispatch()` call reads
// and writes that same frame through the ordinary `EvalHost` scope
// machinery (`load_var`/`set_var`), so no separate state store was
// needed -- the existing scope stack already does the right thing here,
// as long as the same `TintVM` instance stays alive, which is this
// struct's whole job.
//
// `dispatch()` calls the handler through `EvalHost::call_user_fn`, NOT
// `EvalHost::call_fn`'s IR-VM path: `call_fn` routes every plain `fn`
// through a *fresh, separate* `IrVM` with its own locals (see vm.rs's
// `call_fn`/the "Bug 3" fix in the project's compilation-status doc), so
// a handler that assigns to a `state` variable would be mutating a local
// that vanishes the moment the call returns -- never reaching the
// `TintVM.scopes` frame the state was actually bound into. `call_user_fn`
// is the scope-based tree-walking evaluator, which does share that
// frame.
//
// Every call is `catch_unwind`-hardened for the same reason as
// `ReplSession`: a handler referencing an undefined function, or hitting
// one of the IR-VM's panic-on-runtime-error paths some other way, is a
// normal authoring mistake here (this is driving a live sandbox), not a
// rare crash that should take the whole wasm module down with it.

use std::panic::{catch_unwind, AssertUnwindSafe};

use tint_ast::{Item, Span};
use tint_evaluator::errors::EvalResult;
use tint_evaluator::{EvalHost, Value as EvalValue};
use tint_lexer::{collect_tokens, Lexer};
use tint_parser::Parser;

use crate::ui::render::UiRenderNode;
use crate::vm::TintVM;

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
        self.vm.define_var(VIEWPORT_WIDTH_VAR, EvalValue::Number(width));
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
        let vm = &mut self.vm;
        let name = &self.ui_fn_name;
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            vm.call_user_fn(handler, &[], Span::dummy())?;
            vm.render_ui_fn(name, &[])
        }));
        unwrap_outcome(outcome)
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
