use super::*;

impl TintVM {
    pub fn new() -> Self {
        Self {
            scopes: RuntimeScopeStack::new(),
            logic_functions: HashMap::new(),
            ui_functions: HashMap::new(),
            native_fns: Rc::new(RefCell::new(HashMap::new())),

            ui: UiRuntime::new(),

            ir_program: ProgramIR::new(),
            last_result: EvalValue::Unit,
        }
    }

    /// Registers a real Rust function under `name`, directly callable from
    /// `.tn` source as `name(...)` -- e.g. `vm.register_native("now_ms", |_args| {
    /// Ok(EvalValue::Number(SystemTime::now()...))` }` then `now_ms()` in Tint
    /// runs that exact Rust code.
    ///
    /// This is the direct-interop alternative to teaching Tint's own parser
    /// and semantic checker to understand Rust syntax (and to reimplementing
    /// something like rustc's borrow checker, which isn't a separable
    /// library -- see the discussion this is from). A native fn is just a
    /// Rust closure over already-evaluated `Value` arguments/return; it can
    /// call into any real Rust code (the standard library, other crates
    /// linked into the binary) with zero Tint-side awareness of Rust's
    /// grammar or ownership rules -- borrow-checking happens where it always
    /// does, at `rustc` compile time for the Rust code inside the closure.
    ///
    /// Registered names are checked first in both `call_fn` and
    /// `call_user_fn`, ahead of builtins/UI fns/logic fns, so a native fn
    /// can shadow any of those on purpose.
    pub fn register_native<F>(&mut self, name: impl Into<String>, f: F)
    where
        F: Fn(&[EvalValue]) -> EvalResult<EvalValue> + 'static,
    {
        self.native_fns
            .borrow_mut()
            .insert(name.into(), Box::new(f));
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
    pub(super) fn register_functions(&mut self, program: &Program) {
        eprintln!(
            "DEBUG register_functions: registering {} items",
            program.items.len()
        );
        for item in &program.items {
            match item {
                Item::Fn(f) | Item::ExportFn(f, _) => {
                    eprintln!(
                        "DEBUG register_functions: registering logic function: {}",
                        f.name
                    );
                    self.logic_functions.insert(f.name.clone(), f.clone());
                }
                Item::UiFn(ui) => {
                    self.ui_functions.insert(ui.name.clone(), ui.clone());
                }
                _ => {}
            }
        }
    }
}
