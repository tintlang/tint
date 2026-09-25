// RuneVM v3: Dual Runtime (IR VM + EvalHost)

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

use rune_evaluator::eval_pattern::bind_pattern;
use rune_ir::{SsaCompiler, optimize, IrVM, ProgramIR, ir::Value as IrValue};

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

pub struct RuneVM {
    // Runtime 
    pub scopes: RuntimeScopeStack,
    pub logic_functions: HashMap<String, FnDecl>,
    pub ui_functions: HashMap<String, UiFnDecl>,

    pub resources: ResourceTable,
    pub state: StateStore,
    pub scheduler: Scheduler,
    pub borrow: BorrowManager,
    pub ui: UiRuntime,

    // IR 
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

    // PROGRAM ENTRY
    pub fn run_program(&mut self, program: &Program) {
        println!("Compiling to SSA IR…");

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
        optimize(&mut self.ir_program);
        self.register_functions(program);
        self.mount_ui(program);
        self.last_result = EvalValue::Unit;
    }

    // FUNCTION REGISTRATION
    fn register_functions(&mut self, program: &Program) {
        eprintln!("DEBUG register_functions: registering {} items", program.items.len());
        for item in &program.items {
            match item {
                Item::Fn(f) => {
                    eprintln!("DEBUG register_functions: registering logic function: {}", f.name);
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
                    // `mount` needs `&mut self` (to eval if{}/for{}/text),
                    // but `self.ui` is a field of `self` -- can't borrow
                    // both at once. Swap the UiRuntime out for the call.
                    let mut ui_runtime = std::mem::take(&mut self.ui);
                    ui_runtime.mount(ui, self);
                    self.ui = ui_runtime;
                }
            }
        }
    }

    // CLASSIFICATION: UI or IR?
    fn is_ui_function(&self, name: &str) -> bool {
        self.ui_functions.contains_key(name)
    }

    fn is_ir_function(&self, name: &str) -> bool {
        let result = self.logic_functions.contains_key(name);
        eprintln!("DEBUG is_ir_function: {} = {}", name, result);
        result
    }

    // Eval helpers
    fn eval_to_rt(v: EvalValue) -> RuntimeValue {
        match v {
            EvalValue::Number(n) => RuntimeValue::Number(n),
            EvalValue::String(s) => RuntimeValue::String(s),
            EvalValue::Bool(b) => RuntimeValue::Bool(b),
            EvalValue::Unit => RuntimeValue::Unit,
            EvalValue::Tuple(items) => RuntimeValue::Tuple(items.into_iter().map(Self::eval_to_rt).collect()),
            EvalValue::List(items) => RuntimeValue::List(items.into_iter().map(Self::eval_to_rt).collect()),
            EvalValue::StructInstance { name, fields } => {
                RuntimeValue::StructInstance {
                    name,
                    fields: fields.into_iter().map(|(k, v)| (k, Self::eval_to_rt(v))).collect(),
                }
            }
            EvalValue::EnumInstance { enum_name, variant, args } => {
                RuntimeValue::EnumInstance {
                    enum_name,
                    variant,
                    args: args.into_iter().map(Self::eval_to_rt).collect(),
                }
            }
            EvalValue::Map(map) => {
                RuntimeValue::Map(map.into_iter().map(|(k, v)| (k, Self::eval_to_rt(v))).collect())
            }
            other => panic!("Unsupported EvalValue: {:?}", other),
        }
    }

    fn rt_to_eval(v: &RuntimeValue) -> EvalValue {
        match v {
            RuntimeValue::Number(n) => EvalValue::Number(*n),
            RuntimeValue::String(s) => EvalValue::String(s.clone()),
            RuntimeValue::Bool(b) => EvalValue::Bool(*b),
            RuntimeValue::Tuple(items) => EvalValue::Tuple(items.iter().map(Self::rt_to_eval).collect()),
            RuntimeValue::List(items) => EvalValue::List(items.iter().map(Self::rt_to_eval).collect()),
            RuntimeValue::StructInstance { name, fields } => {
                EvalValue::StructInstance {
                    name: name.clone(),
                    fields: fields.iter().map(|(k, v)| (k.clone(), Self::rt_to_eval(v))).collect(),
                }
            }
            RuntimeValue::EnumInstance { enum_name, variant, args } => {
                EvalValue::EnumInstance {
                    enum_name: enum_name.clone(),
                    variant: variant.clone(),
                    args: args.iter().map(Self::rt_to_eval).collect(),
                }
            }
            RuntimeValue::Map(map) => {
                EvalValue::Map(map.iter().map(|(k, v)| (k.clone(), Self::rt_to_eval(v))).collect())
            }
            _ => EvalValue::Unit,
        }
    }

    // Block execution for old evaluator
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

        // 1) Параметры
        for (param, arg) in ui.params.iter().zip(args.iter()) {
            bind_pattern(self, &param.pattern, arg);
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


fn eval_value_to_ir_value(v: EvalValue) -> IrValue {
        match v {
            EvalValue::Number(n) => IrValue::Number(n),
            EvalValue::String(s) => IrValue::String(s),
            EvalValue::Bool(b) => IrValue::Bool(b),
            EvalValue::Unit => IrValue::Unit,
            EvalValue::Tuple(items) => IrValue::Tuple(items.into_iter().map(Self::eval_value_to_ir_value).collect()),
            EvalValue::List(items) => IrValue::List(items.into_iter().map(Self::eval_value_to_ir_value).collect()),
            EvalValue::EnumInstance { enum_name, variant, args } => {
                IrValue::EnumInstance {
                    enum_name,
                    variant,
                    args: args.into_iter().map(Self::eval_value_to_ir_value).collect(),
                }
            }
            EvalValue::Map(map) => {
                IrValue::Map(map.into_iter().map(|(k, v)| (k, Self::eval_value_to_ir_value(v))).collect())
            }
            EvalValue::StructInstance { name, fields } => {
                IrValue::StructInstance {
                    name,
                    fields: fields.into_iter().map(|(k, v)| (k, Self::eval_value_to_ir_value(v))).collect(),
                }
            }
            // Lambdas/functions/host-functions/namespaces have no IR-value
            // representation; the IR VM doesn't support calling through them.
            EvalValue::Lambda { .. }
            | EvalValue::Function { .. }
            | EvalValue::HostFunction(_)
            | EvalValue::Namespace { .. } => IrValue::Unit,
        }
    }

fn ir_value_to_eval_value(v: IrValue) -> EvalValue {
        match v {
            IrValue::Number(n) => EvalValue::Number(n),
            IrValue::String(s) => EvalValue::String(s),
            IrValue::Bool(b) => EvalValue::Bool(b),
            IrValue::Unit => EvalValue::Unit,
            IrValue::Tuple(items) => EvalValue::Tuple(items.into_iter().map(Self::ir_value_to_eval_value).collect()),
            IrValue::List(items) => EvalValue::List(items.into_iter().map(Self::ir_value_to_eval_value).collect()),
            IrValue::EnumInstance { enum_name, variant, args } => {
                EvalValue::EnumInstance {
                    enum_name,
                    variant,
                    args: args.into_iter().map(Self::ir_value_to_eval_value).collect(),
                }
            }
            IrValue::Map(map) => {
                EvalValue::Map(map.into_iter().map(|(k, v)| (k, Self::ir_value_to_eval_value(v))).collect())
            }
            IrValue::StructInstance { name, fields } => {
                EvalValue::StructInstance {
                    name,
                    fields: fields.into_iter().map(|(k, v)| (k, Self::ir_value_to_eval_value(v))).collect(),
                }
            }
        }
    }

    /// Mounts `name` (a `ui fn`) fresh and returns its rendered tree --
    /// the top-level nodes in its body, each converted (recursively)
    /// into a `UiRenderNode` carrying resolved `style`/`hover_style` and
    /// best-effort `text` (see ui/render.rs, ui/style.rs, ui/builder.rs).
    ///
    /// Unlike `call_fn`'s UI branch (`call_ui_fn`, still a stub -- see
    /// its comment), this is the one path that actually builds a UI
    /// tree from a named `ui fn`, independent of `run_program`'s
    /// App/Main auto-mount. It always mounts fresh (a new `UiBuilder`
    /// over `ui.body`), so repeated calls don't accumulate state.
    pub fn render_ui_fn(
        &mut self,
        name: &str,
        args: &[EvalValue],
    ) -> EvalResult<Vec<crate::ui::render::UiRenderNode>> {
        let ui = match self.ui_functions.get(name).cloned() {
            Some(ui) => ui,
            None => {
                return Err(rune_evaluator::errors::EvalError::InvalidOp {
                    msg: format!("Unknown ui fn '{}'", name),
                    span: Span::dummy(),
                })
            }
        };

        self.scopes.push();
        for (param, arg) in ui.params.iter().zip(args.iter()) {
            bind_pattern(self, &param.pattern, arg);
        }

        // Same self-borrow swap as `mount_ui` above.
        let mut ui_runtime = std::mem::take(&mut self.ui);
        ui_runtime.mount(&ui, self);
        self.ui = ui_runtime;

        self.scopes.pop();

        let nodes = match self.ui.root {
            Some(root) => crate::ui::render::to_render_tree(&self.ui.tree, root).children,
            None => Vec::new(),
        };

        Ok(nodes)
    }
}

// EvalHost IMPLEMENTATION — used for UI & fallback logic
impl EvalHost for RuneVM {
    // VARIABLES
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

    fn assign_to(&mut self, lhs: &Expr, value: EvalValue) -> bool {
        match lhs {
            // x = value
            Expr::Ident(name, _) => {
                self.set_var(name, value);
                true
            }

            // obj.field = value
            Expr::Field { target, field, .. } => {
                // 1) evaluate object
                let mut obj = self.eval_expr(target);

                // 2) mutate in-place
                obj.set_field(field, value.clone());

                // 3) write mutated object back
                if !self.assign_to(target, obj) {
                    panic!("Failed to assign back into parent object");
                }

                true
            }

            // arr[i] = value
            Expr::Index { target, index, .. } => {
                let mut arr = self.eval_expr(target);
                let idx = self.eval_expr(index).as_int();

                arr.set_index(idx, value.clone());

                if !self.assign_to(target, arr) {
                    panic!("Failed to assign updated array back");
                }

                true
            }

            _ => false,
        }
    }

    // EXPR & BLOCK
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

        // 2) UI functions -> EvalHost
        if self.is_ui_function(name) {
            // TAKE OWNERSHIP (clone) to avoid holding immutable borrow on self
            let ui = self.ui_functions.get(name).cloned().unwrap();
            return self.call_ui_fn(&ui, args, span);
        }


        // 3) Logic functions -> IR VM
        if self.is_ir_function(name) {
            let params: Vec<rune_ast::Pattern> = self
                .logic_functions
                .get(name)
                .map(|f| f.params.iter().map(|p| p.pattern.clone()).collect())
                .unwrap_or_default();
            let ir_args: Vec<IrValue> = args
                .iter()
                .map(|a| Self::eval_value_to_ir_value(a.clone()))
                .collect();

            let mut irvm = IrVM::new(self.ir_program.clone());
            let out = irvm.run_with_args(name, &params, &ir_args);
            return Ok(Self::ir_value_to_eval_value(out));
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

        self.scopes.push();

        // bind params
        for (param, arg) in func.params.iter().zip(args.iter()) {
            bind_pattern(self, &param.pattern, arg);
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

    // unsupported features for now
    fn call_value(&mut self, v: EvalValue, args: &[EvalValue], span: Span) -> EvalValue {
        match v {
            // если IR передал имя функции как строку
            EvalValue::String(name) => {
                return self.call_fn(&name, args, span)
                    .unwrap_or(EvalValue::Unit);
            }

            // если IR передал Unit: считаем это no-op
            EvalValue::Unit => EvalValue::Unit,

            other => panic!("call_value not supported: {:?}", other),
        }
    }

    fn bind_pattern(&mut self, pat: &rune_ast::Pattern, value: &EvalValue) -> bool {
    use rune_ast::{Pattern, PatternField};

    match pat {
        Pattern::Wildcard(_) => true,

        Pattern::Ident(name, _) => {
            self.define_var(name, value.clone());
            true
        }

        Pattern::Mut { inner, .. } => {
            self.bind_pattern(inner, value)
        }

        Pattern::Number(s, _) => match value {
            EvalValue::Number(n) => n.to_string() == *s,
            _ => false,
        },

        Pattern::String(s, _) => match value {
            EvalValue::String(v) => v == s,
            _ => false,
        },

                // Group pattern: { a, b: pat }
        Pattern::Group { fields, .. } => match value {
            EvalValue::Map(map) => {
                for f in fields {
                    match f {
                        // shorthand: { a }
                        PatternField::Shorthand { field, .. } => {
                            if let Some(v) = map.get(field) {
                                self.define_var(field, v.clone());
                            } else {
                                return false;
                            }
                        }

                        // assign: { a: pat }
                        PatternField::Assign { field, pat, .. } => {
                            if let Some(v) = map.get(field) {
                                if !self.bind_pattern(pat, v) {
                                    return false;
                                }
                            } else {
                                return false;
                            }
                        }

                        PatternField::Rest(_) => { /* ignore */ }
                    }
                }
                true
            }
            _ => false,
        },

        // Tuple pattern: (a, b, c)
        Pattern::Tuple(p_items, _) => match value {
            EvalValue::Tuple(v_items) => {
                if p_items.len() != v_items.len() {
                    return false;
                }
                for (p, v) in p_items.iter().zip(v_items.iter()) {
                    if !self.bind_pattern(p, v) {
                        return false;
                    }
                }
                true
            }
            _ => false,
        },

        // Struct pattern: User { x, y }
        Pattern::Struct { name, fields, .. } => match value {
            EvalValue::StructInstance { name: vname, fields: vfields } => {
                if vname != name {
                    return false;
                }

                for pf in fields {
                    match pf {
                        PatternField::Shorthand { field, .. } => {
                            // get value from struct
                            let Some(v) = vfields
                                .iter()
                                .find(|(k,_)| k == field)
                                .map(|(_,v)| v)
                            else {
                                return false;
                            };

                            // bind variable
                            self.define_var(field, v.clone());
                        }

                        PatternField::Assign { field, pat, .. } => {
                            let Some(v) = vfields
                                .iter()
                                .find(|(k,_)| k == field)
                                .map(|(_,v)| v)
                            else {
                                return false;
                            };

                            if !self.bind_pattern(pat, v) {
                                return false;
                            }
                        }

                        PatternField::Rest(_) => continue,
                    }
                }

                true
            }
            _ => false,
        },

        // Map pattern: map { x, y: pat, .. }
        Pattern::Map { fields, .. } => match value {
            EvalValue::Map(hm) => {
                for pf in fields {
                    match pf {
                        // x  -> bind variable x = hm["x"]
                        PatternField::Shorthand { field, .. } => {
                            let Some(v) = hm.get(field) else {
                                return false;
                            };
                            self.define_var(field, v.clone());
                        }

                        // x: pat -> recursively bind
                        PatternField::Assign { field, pat, .. } => {
                            let Some(v) = hm.get(field) else {
                                return false;
                            };
                            if !self.bind_pattern(pat, v) {
                                return false;
                            }
                        }

                        // .. -> allow extra keys
                        PatternField::Rest(_) => continue,
                    }
                }

                true
            }

            _ => false,
        },

            // Enum pattern: Ok(x), Error(msg)
            Pattern::Variant { name, args, .. } => match value {
                EvalValue::EnumInstance { variant, args: v_args, .. } => {
                    if variant != name {
                        return false;
                    }
                    if args.len() != v_args.len() {
                        return false;
                    }
                    for (p, v) in args.iter().zip(v_args.iter()) {
                        if !self.bind_pattern(p, v) {
                            return false;
                        }
                    }
                    true
                }
                _ => false,
            },

            Pattern::Typed { pat, ty, .. } => {
                if !value.matches_type(ty) {
                    return false;
                }
                return self.bind_pattern(pat, value);
            }
        }
    }

    fn apply_compound(
        &mut self,
        left: &EvalValue,
        op: &str,
        right: &EvalValue
    ) -> EvalValue {
        match (left, op, right) {
            (EvalValue::Number(a), "+=", EvalValue::Number(b)) =>
                EvalValue::Number(a + b),

            (EvalValue::Number(a), "-=", EvalValue::Number(b)) =>
                EvalValue::Number(a - b),

            (EvalValue::Number(a), "*=", EvalValue::Number(b)) =>
                EvalValue::Number(a * b),

            (EvalValue::Number(a), "/=", EvalValue::Number(b)) =>
                EvalValue::Number(a / b),

            // Strings: s += "text"
            (EvalValue::String(a), "+=", EvalValue::String(b)) =>
                EvalValue::String(format!("{}{}", a, b)),

            // Lists: list += elem
            (EvalValue::List(a), "+=", v) => {
                let mut out = a.clone();
                out.push(v.clone());
                EvalValue::List(out)
            }

            _ => EvalValue::Unit,
        }
    }


    fn namespace_lookup(&mut self, _ns: EvalValue, item: &str) -> EvalValue {
        panic!("Namespaces not supported: {}", item);
    }

    fn field_lookup(&mut self, obj: EvalValue, field: &str) -> EvalValue {
        match obj {
            EvalValue::StructInstance { fields, .. } => {
                for (name, value) in fields {
                    if name == field {
                        return value;
                    }
                }
                panic!("Field '{}' not found in struct", field);
            }
            EvalValue::Map(map) => {
                map.get(field).cloned().unwrap_or(EvalValue::Unit)
            }
            other => panic!("Field lookup not supported: obj={:?}, field={}", other, field),
        }
    }

    fn index_lookup(&mut self, obj: EvalValue, idx: EvalValue) -> EvalValue {
        match (obj, idx) {
            (EvalValue::List(items), EvalValue::Number(n)) => {
                let index = n as usize;
                if index < items.len() {
                    items[index].clone()
                } else {
                    panic!("List index {} out of bounds (len={})", index, items.len());
                }
            }
            (EvalValue::Tuple(items), EvalValue::Number(n)) => {
                let index = n as usize;
                if index < items.len() {
                    items[index].clone()
                } else {
                    panic!("Tuple index {} out of bounds (len={})", index, items.len());
                }
            }
            (EvalValue::Map(map), EvalValue::String(key)) => {
                map.get(&key).cloned().unwrap_or(EvalValue::Unit)
            }
            (obj, idx) => panic!("Index lookup not supported: obj={:?}, idx={:?}", obj, idx),
        }
    }

    fn capture_env(&mut self) -> std::rc::Rc<rune_evaluator::env::Env> {
        use rune_evaluator::env::Env;
        std::rc::Rc::new(Env::new())
    }

    fn match_pattern(&mut self, value: &EvalValue, pat: &rune_ast::Pattern) -> bool {
        use rune_ast::{Pattern, PatternField};

        match pat {
            // `_`
            Pattern::Wildcard(_) => true,

            // `mut x`
            Pattern::Mut { inner, .. } => {
                self.match_pattern(value, inner)
            }

            // `_ = expr`
            Pattern::Ident(_, _) => true,

            // literal numbers
            Pattern::Number(s, _) => match value {
                EvalValue::Number(n) => &n.to_string() == s,
                _ => false,
            },

            // literal string
            Pattern::String(s, _) => match value {
                EvalValue::String(v) => v == s,
                _ => false,
            },

            // group pattern: { a, b: pat }
            Pattern::Group { fields, .. } => match value {
                EvalValue::Map(map) => {
                    for pf in fields {
                        match pf {
                            // shorthand: { a }
                            PatternField::Shorthand { field, .. } => {
                                if !map.contains_key(field) {
                                    return false;
                                }
                            }

                            // assign: { a: pat }
                            PatternField::Assign { field, pat, .. } => {
                                let Some(v) = map.get(field) else {
                                    return false;
                                };

                                if !self.match_pattern(v, pat) {
                                    return false;
                                }
                            }

                            PatternField::Rest(_) => continue,
                        }
                    }

                    true
                }

                _ => false,
            },

            // tuple patterns: (a, b, c)
            Pattern::Tuple(p_items, _) => match value {
                EvalValue::Tuple(v_items) => {
                    if p_items.len() != v_items.len() {
                        return false;
                    }

                    for (p, v) in p_items.iter().zip(v_items.iter()) {
                        if !self.match_pattern(v, p) {
                            return false;
                        }
                    }

                    true
                }
                _ => false,
            },

            // struct patterns: User { id, name }
            Pattern::Struct { name, fields, .. } => match value {
                EvalValue::StructInstance { name: vname, fields: vfields } => {
                    if vname != name {
                        return false;
                    }

                    for pf in fields {
                        match pf {
                            // shorthand: name → just check exists
                            PatternField::Shorthand { field, .. } => {
                                if vfields.iter().all(|(k, _)| k != field) {
                                    return false;
                                }
                            }

                            // field: pattern
                            PatternField::Assign { field, pat, .. } => {
                                let Some(v) =
                                    vfields.iter().find(|(k,_)| k == field).map(|(_,v)| v)
                                else {
                                    return false;
                                };

                                if !self.match_pattern(v, pat) {
                                    return false;
                                }
                            }

                            // `..` always ok
                            PatternField::Rest(_) => continue,
                        }
                    }

                    true
                }
                _ => false,
            },

            // map { x, y: pat, .. }
            Pattern::Map { fields, .. } => match value {
                EvalValue::Map(hm) => {
                    for pf in fields {
                        match pf {
                            // shorthand: x → must contain key "x"
                            PatternField::Shorthand { field, .. } => {
                                if !hm.contains_key(field) {
                                    return false;
                                }
                            }

                            // x: pat -> recursively match
                            PatternField::Assign { field, pat, .. } => {
                                let Some(v) = hm.get(field) else {
                                    return false;
                                };
                                if !self.match_pattern(v, pat) {
                                    return false;
                                }
                            }

                            // { .. } — allow other keys
                            PatternField::Rest(_) => continue,
                        }
                    }
                    true
                }
                _ => false,
            },

            // enums unsupported (как ты и хотел)
            Pattern::Variant { .. } => {
                panic!("Pattern matching on variants not supported");
            }

            // typed pattern: x: i32
            Pattern::Typed { pat, ty, .. } => {
                if !value.matches_type(ty) {
                    return false;
                }
                return self.match_pattern(value, pat);
            }

        }
    }
}
