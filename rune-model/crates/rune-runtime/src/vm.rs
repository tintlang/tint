// RuneVM v2 — Clean Layered Runtime
// ============================================================================
// RuneVM v3 — Dual Runtime (IR VM + EvalHost)
// ============================================================================

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

use rune_ir::{SsaCompiler, optimize, IrVM, ProgramIR};

use rune_ast::{
    Program, Item, FnDecl, UiFnDecl, Expr, Stmt, Block, Span, FnBody
};

use rune_evaluator::{
    EvalHost, Value as EvalValue,
    eval_expr, eval_stmt,
    call::call_builtin,
};
use rune_evaluator::eval_host::Flow;
use rune_evaluator::errors::EvalResult;

// ============================================================================
// RuneVM — manages two execution engines
// ============================================================================
pub struct RuneVM {
    // ---------------------- Runtime ----------------------
    pub scopes: RuntimeScopeStack,
    pub logic_functions: HashMap<String, FnDecl>,
    pub ui_functions: HashMap<String, UiFnDecl>,

    pub resources: ResourceTable,
    pub state: StateStore,
    pub scheduler: Scheduler,
    pub borrow: BorrowManager,
    pub ui: UiRuntime,

    // ---------------------- IR ----------------------
    pub ir_program: ProgramIR,
    pub last_result: EvalValue,
}

impl RuneVM {
    pub fn new() -> Self {
        Self {
            scopes: RuntimeScopeStack::new(),
            logic_functions: HashMap::new(),
            ui_functions: HashMap::new(),

            resources: ResourceTable::new(),
            state: StateStore::new(),
            scheduler: Scheduler::new(),
            borrow: BorrowManager::new(),
            ui: UiRuntime::new(),

            ir_program: ProgramIR::new(),
            last_result: EvalValue::Unit,
        }
    }

    // =========================================================================
    // PROGRAM ENTRY
    // =========================================================================
    pub fn run_program(&mut self, program: &Program) {
        println!("Compiling to SSA IR…");

        // 1) Compile to IR
        let mut compiler = SsaCompiler::new();
        self.ir_program = compiler.compile_program(program);

        println!("=== SSA IR DUMP ===");
        for f in &self.ir_program.functions {
            println!("fn {}:", f.name);
            for block in &f.blocks {
                println!("  block {}:", block.id);
                for instr in &block.instrs {
                    println!("    {:?}", instr);
                }
            }
        }
        println!("====================\n");

        // 2) Optimize
        optimize(&mut self.ir_program);

        // 3) Register functions
        self.register_functions(program);

        // 4) Mount UI (optional)
        self.mount_ui(program);

        // 5) No auto-run
        self.last_result = EvalValue::Unit;
    }

    // =========================================================================
    // FUNCTION REGISTRATION
    // =========================================================================
    fn register_functions(&mut self, program: &Program) {
        for item in &program.items {
            match item {
                Item::Fn(f) => {
                    self.logic_functions.insert(f.name.clone(), f.clone());
                }
                Item::UiFn(ui) => {
                    self.ui_functions.insert(ui.name.clone(), ui.clone());
                }
                _ => {}
            }
        }
    }

    fn mount_ui(&mut self, program: &Program) {
        for item in &program.items {
            if let Item::UiFn(ui) = item {
                if ui.name == "App" || ui.name == "Main" {
                    self.ui.mount(ui);
                }
            }
        }
    }

    // =========================================================================
    // CLASSIFICATION: UI or IR?
    // =========================================================================
    fn is_ui_function(&self, name: &str) -> bool {
        self.ui_functions.contains_key(name)
    }

    fn is_ir_function(&self, name: &str) -> bool {
        self.logic_functions.contains_key(name)
    }

    // =========================================================================
    // Eval helpers
    // =========================================================================
    fn eval_to_rt(v: EvalValue) -> RuntimeValue {
        match v {
            EvalValue::Number(n) => RuntimeValue::Number(n),
            EvalValue::String(s) => RuntimeValue::String(s),
            EvalValue::Bool(b) => RuntimeValue::Bool(b),
            EvalValue::Unit => RuntimeValue::Unit,
            other => panic!("Unsupported EvalValue: {:?}", other),
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

    // =========================================================================
    // Block execution for old evaluator
    // =========================================================================
    fn eval_block_flow(&mut self, block: &Block) -> Flow {
        let mut last = Flow::Value(EvalValue::Unit);

        for stmt in &block.stmts {
            let flow = eval_stmt::eval_stmt(self, stmt);

            match flow {
                Flow::Value(_) => last = flow,
                Flow::Return(_) | Flow::Break | Flow::Continue => return flow,
            }
        }

        last
    }
        
    fn call_ui_fn(
        &mut self,
        ui: &UiFnDecl,
        args: &[EvalValue],
        span: Span,
    ) -> EvalResult<EvalValue> {


        println!("⛰ UI CALL → {}(", ui.name);
        for (i, a) in args.iter().enumerate() {
            println!("    arg[{i}] = {:?}", a);
        }
        println!(")");
        
        println!("UI FN CALL: {}", ui.name);

        // 1) Параметры — как обычно
        self.push_scope();
        for (param, arg) in ui.params.iter().zip(args) {
            self.define_var(&param.name, arg.clone());
        }

        // 2) Здесь ПОКА НИЧЕГО НЕ ДЕЛАЕМ.
        // Позже:
        // - interpolation
        // - UiBuilder
        // - UiTree
        // - reconciliation
        // - layout
        // - WebGPU render

        println!("UI tree building not implemented yet.");

        self.pop_scope();

        Ok(EvalValue::Unit)
    }

}

// ============================================================================
// EvalHost IMPLEMENTATION — used for UI & fallback logic
// ============================================================================
impl EvalHost for RuneVM {

    // -----------------------
    // VARIABLES
    // -----------------------
    fn load_var(&mut self, name: &str, _span: Span) -> EvalValue {
        match self.scopes.lookup(name) {
            Some(v) => Self::rt_to_eval(&v),
            None => EvalValue::Unit,
        }
    }

    fn define_var(&mut self, name: &str, v: EvalValue) {
        self.scopes.define(name, Self::eval_to_rt(v));
    }

    fn set_var(&mut self, name: &str, v: EvalValue) {
        self.scopes.set(name, Self::eval_to_rt(v));
    }

    fn push_scope(&mut self) { self.scopes.push(); }
    fn pop_scope(&mut self) { self.scopes.pop(); }

    // -----------------------
    // EXPR & BLOCK
    // -----------------------
    fn eval_expr(&mut self, expr: &Expr) -> EvalValue {
        eval_expr::eval_expr(self, expr)
    }

    fn eval_block_flow(&mut self, block: &Block) -> Flow {
        RuneVM::eval_block_flow(self, block)
    }

    fn eval_block(&mut self, block: &Block) -> Flow {
        RuneVM::eval_block_flow(self, block)
    }

    // -----------------------
    // FUNCTION CALL DISPATCH
    // -----------------------
    fn call_fn(&mut self, name: &str, args: &[EvalValue], span: Span)
        -> EvalResult<EvalValue>
    {
        // 1) builtins (math, print, etc.)
        if let Ok(v) = call_builtin(self, name, args, span) {
            return Ok(v);
        }

        // 2) UI functions → EvalHost
        if self.is_ui_function(name) {
            // TAKE OWNERSHIP (clone) to avoid holding immutable borrow on self
            let ui = self.ui_functions.get(name).cloned().unwrap();
            return self.call_ui_fn(&ui, args, span);
        }


        // 3) Logic functions → IR VM
        if self.is_ir_function(name) {
            let mut irvm = IrVM::new(self.ir_program.clone());
            let out = irvm.run(name);
            return Ok(EvalValue::Number(out.unwrap_number()));
        }

        // 4) fallback
        self.call_user_fn(name, args, span)
    }

    fn call_user_fn(
        &mut self,
        name: &str,
        args: &[EvalValue],
        span: Span,
    ) -> EvalResult<EvalValue>
    {
        let func = match self.logic_functions.get(name).cloned() {
            Some(f) => f,
            None => return Err(rune_evaluator::errors::EvalError::InvalidOp {
                msg: format!("Unknown function '{}'", name),
                span,
            }),
        };

        // new scope
        self.scopes.push();

        // bind params
        for (param, arg) in func.params.iter().zip(args.iter()) {
            self.define_var(&param.name, arg.clone());
        }

        // execute
        let result = match &func.body {
            FnBody::Block(block) => match self.eval_block(block) {
                Flow::Value(v) => v,
                Flow::Return(v) => v,
                _ => EvalValue::Unit,
            },
            FnBody::Expr(expr) => self.eval_expr(expr),
        };

        self.scopes.pop();
        Ok(result)
    }

    // ------------------------------------------------------------------------
    // unsupported features for now
    // ------------------------------------------------------------------------
    fn call_value(&mut self, v: EvalValue, args: &[EvalValue], span: Span) -> EvalValue {
        match v {
            // если IR передал имя функции как строку
            EvalValue::String(name) => {
                return self.call_fn(&name, args, span)
                    .unwrap_or(EvalValue::Unit);
            }

            // если IR передал Unit — считаем это no-op
            EvalValue::Unit => EvalValue::Unit,

            other => panic!("call_value not supported: {:?}", other),
        }
    }

    fn namespace_lookup(&mut self, _ns: EvalValue, item: &str) -> EvalValue {
        panic!("Namespaces not supported: {}", item);
    }

    fn field_lookup(&mut self, obj: EvalValue, field: &str) -> EvalValue {
        panic!("Field lookup not supported: obj={:?}, field={}", obj, field);
    }

    fn index_lookup(&mut self, obj: EvalValue, idx: EvalValue) -> EvalValue {
        panic!("Index lookup not supported: obj={:?}, idx={:?}", obj, idx);
    }

    fn capture_env(&mut self) -> std::rc::Rc<rune_evaluator::env::Env> {
        use rune_evaluator::env::Env;
        std::rc::Rc::new(Env::new())
    }

    fn match_pattern(&mut self, value: &EvalValue, pat: &rune_ast::Pattern) -> bool {
        use rune_ast::Pattern;

        match pat {
            Pattern::Wildcard(_) => true,
            Pattern::Ident(_, _) => true,
            Pattern::Number(s, _) => matches!(value, EvalValue::Number(n) if &n.to_string() == s),
            Pattern::String(s, _) => matches!(value, EvalValue::String(v) if v == s),
            Pattern::Variant {..} => panic!("Pattern matching on variants not supported"),
        }
    }
}
