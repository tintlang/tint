// TintVM v3: Dual Runtime (IR VM + EvalHost)

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::{scope::RuntimeScopeStack, ui::UiRuntime, value::RuntimeValue};

use tint_evaluator::eval_pattern::bind_pattern;
use tint_ir::{ir::Value as IrValue, optimize, IrVM, ProgramIR, SsaCompiler};

use tint_ast::{Block, Expr, FnBody, FnDecl, Item, Program, Span, UiFnDecl};

use tint_evaluator::errors::EvalResult;
use tint_evaluator::eval_host::Flow;
use tint_evaluator::{call::call_builtin, eval_stmt, EvalHost, Value as EvalValue};

pub struct TintVM {
    // Runtime
    pub scopes: RuntimeScopeStack,
    pub logic_functions: HashMap<String, FnDecl>,
    pub ui_functions: HashMap<String, UiFnDecl>,
    pub impl_methods: HashMap<(String, String), FnDecl>,

    // Real Rust functions registered from outside the language (see
    // `register_native`) -- direct Rust interop: no Rust parsing, no
    // borrow-checking of Tint code, just an ordinary Rust closure called
    // by name from `.tn` source.
    //
    // `Rc<RefCell<..>>`, not a plain `HashMap`, so that `call_fn`'s IR
    // branch (below) can hand a fresh `IrVM` a native-fn-lookup closure
    // that owns a `'static` handle back into this same table (`tint-ir`
    // has no dependency on this crate, so `IrVM` can't just borrow
    // `&self` -- see `IrVM::set_native_call`'s doc comment) without that
    // closure's lifetime getting tangled up in `self`'s.
    pub native_fns:
        Rc<RefCell<HashMap<String, Box<dyn Fn(&[EvalValue]) -> EvalResult<EvalValue>>>>>,

    /// Host-backed application storage. The default implementation is an
    /// in-memory store; browser hosts can hydrate/snapshot it through the
    /// UiSession API and persist it in localStorage.
    pub storage: Rc<RefCell<HashMap<String, String>>>,

    /// HTTP requests started by Tint. Fetch itself belongs to the host, so
    /// the VM queues a request and returns its numeric id synchronously.
    pub http_requests: Rc<RefCell<Vec<HttpRequest>>>,

    pub ui: UiRuntime,

    // UI handlers and helpers called by them execute through the tree-walker
    // and must share the persistent state scope.  Top-level logic calls can
    // still use the isolated IR VM.
    pub(crate) treewalk_call_depth: usize,

    // IR
    pub ir_program: ProgramIR,
    pub last_result: EvalValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    pub id: u64,
    pub method: String,
    pub url: String,
}

mod classification;
mod conversion;
mod core;
mod host;
mod ui;
