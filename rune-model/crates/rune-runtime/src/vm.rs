// RuneVM v2.0 — Clean Layered Runtime

// Imports
use std::collections::HashMap;

use crate::{
    scope::RuntimeScopeStack,
    resources::ResourceTable,
    state::StateStore,
    ui::UiRuntime,
    async_rt::Scheduler,
    borrow::BorrowManager,
    value::RuntimeValue,
};

use rune_ast::{Program, Item, FnDecl, UiFnDecl, Expr, Stmt, Span};

use rune_evaluator::{
    EvalHost,
    Value as EvalValue,
    eval_expr,
    eval_stmt,
    eval_block,
    call::call_builtin,
};


/// ============================================================
/// RuneVM — Three-layer runtime
/// ============================================================
pub struct RuneVM {
    // CORE
    pub scopes: RuntimeScopeStack,
    pub functions: HashMap<String, FnDecl>,

    // SYSTEM
    pub resources: ResourceTable,
    pub state: StateStore,
    pub scheduler: Scheduler,
    pub borrow: BorrowManager,

    // UI
    pub ui: UiRuntime,
}

impl RuneVM {
    pub fn new() -> Self {
        Self {
            scopes: RuntimeScopeStack::new(),
            functions: HashMap::new(),

            resources: ResourceTable::new(),
            state: StateStore::new(),
            scheduler: Scheduler::new(),
            borrow: BorrowManager::new(),

            ui: UiRuntime::new(),
        }
    }

    // ------------------------------------------------------------
    // PROGRAM ENTRY
    // ------------------------------------------------------------
    pub fn run_program(&mut self, program: &Program) {
        self.register_functions(program);
        self.mount_entry_ui(program);
        self.main_loop();
    }

    fn register_functions(&mut self, program: &Program) {
        for item in &program.items {
            if let Item::Fn(f) = item {
                self.functions.insert(f.name.clone(), f.clone());
            }
        }
    }

    fn mount_entry_ui(&mut self, program: &Program) {
        for item in &program.items {
            if let Item::UiFn(ui_fn) = item {
                if ui_fn.name == "App" || ui_fn.name == "Main" {
                    self.ui.mount(ui_fn);
                }
            }
        }
    }

    // ------------------------------------------------------------
    // EVENT LOOP
    // ------------------------------------------------------------
    fn main_loop(&mut self) {
        loop {
            let mut progressed = false;

            if self.scheduler.poll_all() { progressed = true; }
            if self.ui.process_events() { progressed = true; }

            if self.ui.needs_redraw() {
                self.ui.render();
                progressed = true;
            }

            if !progressed { break; }
        }
    }

    // ------------------------------------------------------------
    // Eval helpers
    // ------------------------------------------------------------
    pub fn eval_expr(&mut self, expr: &Expr) -> EvalValue {
        eval_expr::eval_expr(self, expr)
    }

    pub fn eval_stmt(&mut self, stmt: &Stmt) -> EvalValue {
        eval_stmt::eval_stmt(self, stmt)
    }

    // ------------------------------------------------------------
    // Value Conversion Layer
    // ------------------------------------------------------------
    fn eval_to_rt(val: EvalValue) -> RuntimeValue {
        match val {
            EvalValue::Number(n) => RuntimeValue::Number(n),
            EvalValue::String(s) => RuntimeValue::String(s),
            EvalValue::Bool(b) => RuntimeValue::Bool(b),
            EvalValue::Unit => RuntimeValue::Unit,
        }
    }

    fn rt_to_eval(val: &RuntimeValue) -> EvalValue {
        match val {
            RuntimeValue::Number(n) => EvalValue::Number(*n),
            RuntimeValue::String(s) => EvalValue::String(s.clone()),
            RuntimeValue::Bool(b) => EvalValue::Bool(*b),
            _ => EvalValue::Unit,
        }
    }
}


// ============================================================
// EvalHost Implementation
// ============================================================
impl EvalHost for RuneVM {

    // VARIABLES
    fn load_var(&mut self, name: &str, _span: Span) -> EvalValue {
        match self.scopes.lookup(name) {
            Some(rt_val) => Self::rt_to_eval(&rt_val),
            None => EvalValue::Unit,
        }
    }

    fn define_var(&mut self, name: &str, value: EvalValue) {
        let rt = Self::eval_to_rt(value);
        self.scopes.define(name, rt);
    }

    fn push_scope(&mut self) { self.scopes.push(); }
    fn pop_scope(&mut self) { self.scopes.pop(); }

    // EVAL
    fn eval_expr(&mut self, expr: &Expr) -> EvalValue {
        eval_expr::eval_expr(self, expr)
    }

    fn eval_block(&mut self, block: &rune_ast::Block) -> EvalValue {
        eval_block::eval_block(self, block)
    }

    // FUNCTIONS
    fn call_fn(&mut self, name: &str, args: &[EvalValue], span: Span) -> EvalValue {
        if let Ok(v) = call_builtin(self, name, args, span) {
            return v;
        }
        self.call_user_fn(name, args, span)
    }

    fn call_user_fn(&mut self, name: &str, args: &[EvalValue], _span: Span) -> EvalValue {
        // ---- 1) Копируем FnDecl, чтобы не держать &func ----
        let func = match self.functions.get(name) {
            Some(f) => f.clone(),   // <-- теперь нет borrow!
            None => {
                println!("Runtime error: no such function `{}`", name);
                return EvalValue::Unit;
            }
        };

        // ---- 2) Теперь можно спокойно работать с self ----
        self.scopes.push();

        // ---- 3) Bind params ----
        for (param, arg) in func.params.iter().zip(args.iter()) {
            self.define_var(&param.name, arg.clone());
        }

        // ---- 4) Execute function body ----
        let res = self.eval_block(&func.body);

        self.scopes.pop();
        res
    }
}
