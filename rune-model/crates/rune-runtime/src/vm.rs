// RuneVM v2. — Clean Layered Runtime

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

// ============================================================================
// RuneVM — layered runtime
// ============================================================================
pub struct RuneVM {
    // ---------------------- CORE ----------------------
    pub scopes: RuntimeScopeStack,
    pub functions: HashMap<String, FnDecl>,

    // ---------------------- SYSTEM --------------------
    pub resources: ResourceTable,
    pub state: StateStore,
    pub scheduler: Scheduler,
    pub borrow: BorrowManager,

    // ---------------------- UI ------------------------
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

    // =========================================================================
    // PROGRAM ENTRY
    // =========================================================================
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
            if let Item::UiFn(ui) = item {
                if ui.name == "App" || ui.name == "Main" {
                    self.ui.mount(ui);
                }
            }
        }
    }

    // EVENT LOOP
    fn main_loop(&mut self) {
        loop {
            let mut progressed = false;

            if self.scheduler.poll_all() { progressed = true; }
            if self.ui.process_events() { progressed = true; }

            if self.ui.needs_redraw() {
                self.ui.render();
                progressed = true;
            }

            if !progressed {
                break;
            }
        }
    }

    // Eval Helpers
    pub fn eval_expr(&mut self, expr: &Expr) -> EvalValue {
        eval_expr::eval_expr(self, expr)
    }

    pub fn eval_stmt(&mut self, stmt: &Stmt) -> EvalValue {
        eval_stmt::eval_stmt(self, stmt)
    }

    // VALUE CONVERSION LAYER
    fn eval_to_rt(v: EvalValue) -> RuntimeValue {
        match v {
            EvalValue::Number(n) => RuntimeValue::Number(n),
            EvalValue::String(s) => RuntimeValue::String(s),
            EvalValue::Bool(b) => RuntimeValue::Bool(b),
            EvalValue::Unit => RuntimeValue::Unit,
        }
    }

    fn rt_to_eval(v: &RuntimeValue) -> EvalValue {
        match v {
            RuntimeValue::Number(n) => EvalValue::Number(*n),
            RuntimeValue::String(s) => EvalValue::String(s.clone()),
            RuntimeValue::Bool(b) => EvalValue::Bool(*b),
            _ => EvalValue::Unit,
        }
    }
}

// EvalHost IMPLEMENTATION

impl EvalHost for RuneVM {


    // VARIABLES
    fn load_var(&mut self, name: &str, _span: Span) -> EvalValue {
        match self.scopes.lookup(name) {
            Some(rt) => Self::rt_to_eval(&rt),
            None => EvalValue::Unit,
        }
    }

    fn define_var(&mut self, name: &str, v: EvalValue) {
        let rt = Self::eval_to_rt(v);
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

    // FUNCTION CALLS
    fn call_fn(&mut self, name: &str, args: &[EvalValue], span: Span) -> EvalValue {
        // 1) Builtins
        if let Ok(v) = call_builtin(self, name, args, span) {
            return v;
        }

        // 2) User functions
        self.call_user_fn(name, args, span)
    }

    fn call_user_fn(&mut self, name: &str, args: &[EvalValue], _span: Span) -> EvalValue {
        let func = match self.functions.get(name).cloned() {
            Some(f) => f,
            None => return EvalValue::Unit,
        };

        self.scopes.push();

        for (param, arg) in func.params.iter().zip(args.iter()) {
            self.define_var(&param.name, arg.clone());
        }

        let out = self.eval_block(&func.body);

        self.scopes.pop();
        out
    }
}
