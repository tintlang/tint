use crate::{
    scope::ScopeStack,
    resources::ResourceTable,
    state::StateTable,
    ui::UiRuntime,
    async_rt::Scheduler,
    borrow::BorrowManager,
};

use rune_evaluator::{EvalHost, Value};
use rune_evaluator::eval_expr::eval_expr;
use rune_evaluator::eval_stmt::eval_stmt;
use rune_evaluator::call::call_builtin;
use rune_evaluator::eval_fn::eval_user_fn;

use rune_ast::{Program, Item, FnDecl, UiFnDecl, Expr, Stmt, Span};

use std::collections::HashMap;

/// Runtime state for RuneLang
pub struct VM {
    pub scopes: ScopeStack,
    pub resources: ResourceTable,
    pub state: StateTable,
    pub ui: UiRuntime,
    pub scheduler: Scheduler,
    pub borrow: BorrowManager,

    /// Storage of user-defined functions
    pub functions: HashMap<String, FnDecl>,
}

impl VM {
    pub fn new() -> Self {
        Self {
            scopes: ScopeStack::new(),
            resources: ResourceTable::new(),
            state: StateTable::new(),
            ui: UiRuntime::new(),
            scheduler: Scheduler::new(),
            borrow: BorrowManager::new(),
            functions: HashMap::new(),
        }
    }

    // Main entry point
    pub fn run_program(&mut self, program: &Program) {
        self.register_functions(program);
        self.run_ui_entry(program);
        self.event_loop();
    }

    // Register all logic functions
    fn register_functions(&mut self, program: &Program) {
        for item in &program.items {
            if let Item::Fn(f) = item {
                self.functions.insert(f.name.clone(), f.clone());
            }
        }
    }

    // Run App() or Main()
    fn run_ui_entry(&mut self, program: &Program) {
        for item in &program.items {
            if let Item::UiFn(ui) = item {
                if ui.name == "App" || ui.name == "Main" {
                    self.ui.mount(self, ui); // OK
                }
            }
        }
    }

    // Main runtime loop
    fn event_loop(&mut self) {
        loop {
            let mut progress = false;

            if self.scheduler.tick(self) { progress = true; }
            if self.ui.process_events(self) { progress = true; }

            if self.ui.needs_redraw() {
                self.ui.render();
                progress = true;
            }

            if !progress {
                break;
            }
        }
    }

    // Shorthands for evaluator
    pub fn eval_expr(&mut self, expr: &Expr) -> Value {
        eval_expr(self, expr)
    }

    pub fn eval_stmt(&mut self, stmt: &Stmt) -> Value {
        eval_stmt(self, stmt)
    }
}

// IMPLEMENTATION OF EvalHost FOR VM
impl EvalHost for VM {

    // Variables
    fn load_var(&mut self, name: &str, _span: Span) -> Value {
        self.scopes.lookup(name).unwrap_or(Value::Unit)
    }

    fn define_var(&mut self, name: &str, value: Value) {
        self.scopes.define(name, value);
    }

    fn push_scope(&mut self) {
        self.scopes.push();
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    // Eval
    fn eval_expr(&mut self, expr: &Expr) -> Value {
        rune_evaluator::eval_expr::eval_expr(self, expr)
    }

    fn eval_block(&mut self, block: &Block) -> Value {
        rune_evaluator::eval_block::eval_block(self, block)
    }

    // Functions (builtins + user)
    fn call_fn(&mut self, name: &str, args: &[Value], span: Span) -> Value {
        // сначала проверим builtin
        match rune_evaluator::call::call_builtin(self, name, args, span) {
            Ok(v) => return v,
            Err(_e) => {
            }
        }

        self.call_user_fn(name, args, span)
    }

    fn call_user_fn(&mut self, name: &str, args: &[Value], span: Span) -> Value {
    
        if let Some(func) = self.functions.get(name) {
            self.push_scope();

            for (param, arg) in func.params.iter().zip(args.iter()) {
                self.define_var(&param.name, arg.clone());
            }

            let result = self.eval_block(&func.body);

            self.pop_scope();

            result
        } else {
            eprintln!("Runtime error: unknown function `{}` at {:?}", name, span);
            Value::Unit
        }
    }
}

