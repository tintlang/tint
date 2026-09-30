//! Stateful UI sessions keep one VM alive so `state` values persist across
//! renders and event dispatches. Runtime errors are converted to `Result`s so
//! an authoring mistake does not bring down the host application.

use std::collections::HashMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::rc::Rc;

use tint_ast::{Item, Program, Span};
use tint_evaluator::errors::EvalResult;
use tint_evaluator::{EvalHost, Value as EvalValue};
use tint_lexer::{collect_tokens, Lexer};
use tint_parser::Parser;
#[cfg(feature = "semantic-check")]
use tint_semantics::prelude::CheckerContext;
#[cfg(feature = "semantic-check")]
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

/// Name of the variable holding the current URL path (`"/"`, `"/pong"`, ...).
/// The host keeps it current with `set_route_path` on load and on every
/// navigation, so `if{route_path == "/pong"}` picks the page to render.
pub const ROUTE_PATH_VAR: &str = "route_path";

pub struct UiSession {
    vm: TintVM,
    ui_fn_name: String,
    /// Reuse unchanged subtrees between renders (default). Turning it off
    /// rebuilds everything every time; results are identical either way.
    reuse: bool,
    app_meta: tint_ast::AppMeta,
}

impl UiSession {
    /// Parses `source`, runs it (registering every `fn`/`ui fn`), and
    /// evaluates `ui_fn_name`'s `state` declarations once into this
    /// session's persistent scope. Fails if the source doesn't parse or
    /// no `ui fn` named `ui_fn_name` exists in it.
    pub fn new(source: &str, ui_fn_name: &str) -> Result<Self, String> {
        Self::new_with_storage(source, ui_fn_name, HashMap::new())
    }

    /// Like `new`, with host storage loaded before any `state` initializer
    /// runs, so `state code = storage_get_or("code", "")` sees saved values.
    pub fn new_with_storage(
        source: &str,
        ui_fn_name: &str,
        storage: HashMap<String, String>,
    ) -> Result<Self, String> {
        let tokens = collect_tokens(&mut Lexer::new(source));
        let mut parser = Parser::new(tokens);
        let program = parser.parse_program().map_err(|e| format!("{:?}", e))?;

        Self::from_program_with_storage(program, ui_fn_name, storage)
    }

    /// Builds a session from a serialized Tint program. This uses the same
    /// VM and state initialization path as source mode; only lexing/parsing
    /// is skipped.
    #[cfg(feature = "bytecode")]
    pub fn from_bytecode(bytes: &[u8], ui_fn_name: &str) -> Result<Self, String> {
        let program = crate::bytecode::decode(bytes)?;
        Self::from_program(program, ui_fn_name)
    }

    /// Builds a session from an already parsed program. Kept public so native
    /// build tools and the WASM bytecode host share exactly one initialization
    /// path.
    pub fn from_program(program: Program, ui_fn_name: &str) -> Result<Self, String> {
        Self::from_program_with_storage(program, ui_fn_name, HashMap::new())
    }

    pub fn from_program_with_storage(
        program: Program,
        ui_fn_name: &str,
        storage: HashMap<String, String>,
    ) -> Result<Self, String> {
        #[cfg(feature = "semantic-check")]
        {
            let semantic_errors = SemanticChecker::new(CheckerContext::default()).check(&program);
            if !semantic_errors.is_empty() {
                return Err(format!("semantic errors: {:?}", semantic_errors));
            }
        }

        let has_ui_fn = program
            .items
            .iter()
            .any(|item| matches!(item, Item::UiFn(f) if f.name == ui_fn_name));
        if !has_ui_fn {
            return Err(format!("Unknown ui fn '{}'", ui_fn_name));
        }

        let app_meta = program.app_meta();
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            let mut vm = TintVM::new();
            vm.hydrate_storage(storage);
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
            if matches!(vm.load_var(ROUTE_PATH_VAR, Span::dummy()), EvalValue::Unit) {
                vm.define_var(ROUTE_PATH_VAR, EvalValue::String("/".to_string()));
            }
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
                reuse: true,
                app_meta,
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

    /// Updates `route_path` (see `ROUTE_PATH_VAR`); takes effect on the next
    /// `render()`/`dispatch()`.
    pub fn set_route_path(&mut self, path: &str) {
        self.vm
            .define_var(ROUTE_PATH_VAR, EvalValue::String(path.to_string()));
    }

    /// Page metadata from the program's `app { ... }` declaration.
    pub fn app_meta(&self) -> &tint_ast::AppMeta {
        &self.app_meta
    }

    /// CSS for the host `<body>` from `app { page::{ ... } }`.
    pub fn page_style(&self) -> Vec<(String, String)> {
        crate::ui::style::resolve_page_style(&self.app_meta.page)
    }

    /// The route declared for `path` (`app { route.Ui::"/path" }`), if any.
    pub fn route_for_path(&self, path: &str) -> Option<&tint_ast::RouteDecl> {
        self.app_meta.routes.iter().find(|route| route.path == path)
    }

    pub fn ui_fn_name(&self) -> &str {
        &self.ui_fn_name
    }

    /// Makes `name` the rendered root `ui fn`, initializing its `state`.
    /// A `theme` already set by the previous page is kept, so switching pages
    /// doesn't flip dark/light.
    pub fn switch_ui_fn(&mut self, name: &str) -> Result<(), String> {
        if name == self.ui_fn_name {
            return Ok(());
        }
        let vm = &mut self.vm;
        let Some(function) = vm.ui_functions.get(name) else {
            return Err(format!("Unknown ui fn '{}'", name));
        };
        let decls = function.state.clone();
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            for decl in &decls {
                if decl.name == "theme" {
                    continue;
                }
                let value = vm.eval_expr(&decl.init);
                vm.define_var(&decl.name, value);
            }
        }));
        outcome.map_err(panic_message)?;
        self.ui_fn_name = name.to_string();
        Ok(())
    }

    /// `@font-face`/`@keyframes` rules declared in `app { }`.
    pub fn app_css(&self) -> String {
        crate::ui::style::app_at_rules(&self.app_meta)
    }

    /// Enables or disables subtree reuse between renders.
    pub fn set_reuse(&mut self, reuse: bool) {
        self.reuse = reuse;
    }

    /// Builds and returns the session's current tree.
    pub fn render(&mut self) -> Result<Vec<Rc<UiRenderNode>>, String> {
        let vm = &mut self.vm;
        let name = &self.ui_fn_name;
        let reuse = self.reuse;
        let outcome = catch_unwind(AssertUnwindSafe(|| render(vm, name, reuse)));
        unwrap_outcome(outcome)
    }

    /// Runs `handler` (a plain `fn`, no args) against this session's
    /// persistent state, then rebuilds and returns the tree -- see this
    /// module's doc comment for why that needs `call_user_fn` and a
    /// long-lived `TintVM`, not a fresh one per call.
    pub fn dispatch(&mut self, handler: &str) -> Result<Vec<Rc<UiRenderNode>>, String> {
        self.dispatch_with_args(handler, &[])
    }

    /// Dispatches a host event with runtime values, used by `frame||handler`
    /// to pass elapsed seconds without making the browser part of Tint logic.
    pub fn dispatch_with_args(
        &mut self,
        handler: &str,
        args: &[EvalValue],
    ) -> Result<Vec<Rc<UiRenderNode>>, String> {
        let vm = &mut self.vm;
        let name = &self.ui_fn_name;
        let reuse = self.reuse;
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            vm.call_user_fn(handler, args, Span::dummy())?;
            render(vm, name, reuse)
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

fn render(vm: &mut TintVM, name: &str, reuse: bool) -> EvalResult<Vec<Rc<UiRenderNode>>> {
    if reuse {
        vm.render_ui_fn_reusing(name, &[])
    } else {
        vm.render_ui_fn_shared_fresh(name, &[])
    }
}

fn unwrap_outcome(
    outcome: std::thread::Result<EvalResult<Vec<Rc<UiRenderNode>>>>,
) -> Result<Vec<Rc<UiRenderNode>>, String> {
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
