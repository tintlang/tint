use super::*;

impl TintVM {
    pub fn new() -> Self {
        let storage = Rc::new(RefCell::new(HashMap::new()));
        let http_requests = Rc::new(RefCell::new(Vec::new()));
        let mut vm = Self {
            scopes: RuntimeScopeStack::new(),
            logic_functions: HashMap::new(),
            ui_functions: HashMap::new(),
            impl_methods: HashMap::new(),
            native_fns: Rc::new(RefCell::new(HashMap::new())),
            storage: Rc::clone(&storage),
            http_requests: Rc::clone(&http_requests),

            ui: UiRuntime::new(),
            treewalk_call_depth: 0,
            read_logs: Vec::new(),
            #[cfg(feature = "ir")]
            has_constants: false,

            #[cfg(feature = "ir")]
            ir_program: ProgramIR::new(),
            last_result: EvalValue::Unit,
        };
        vm.register_host_builtins(storage, http_requests);
        vm
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

    /// Run an entry point and turn evaluator panics into a regular runtime
    /// error. The evaluator still has a few legacy infallible interfaces;
    /// this boundary keeps those failures from escaping into the host.
    pub fn call_fn(&mut self, name: &str, args: &[EvalValue], span: Span) -> EvalResult<EvalValue> {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.host_call_fn(name, args, span)
        })) {
            Ok(result) => result,
            Err(payload) => {
                let message = payload
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| {
                        payload
                            .downcast_ref::<&str>()
                            .map(|value| (*value).to_owned())
                    })
                    .unwrap_or_else(|| "evaluation failed".into());
                Err(tint_evaluator::errors::EvalError::InvalidOp { msg: message, span })
            }
        }
    }

    // PROGRAM ENTRY
    pub fn run_program(&mut self, program: &Program) {
        #[cfg(feature = "ir")]
        {
            let mut compiler = SsaCompiler::new();
            self.ir_program = compiler.compile_program(program);
            optimize(&mut self.ir_program);
        }
        self.register_functions(program);
        self.register_constants(program);
        self.mount_ui(program);
        self.last_result = EvalValue::Unit;
    }

    // FUNCTION REGISTRATION
    pub(super) fn register_functions(&mut self, program: &Program) {
        for item in &program.items {
            match item {
                Item::Fn(f) | Item::ExportFn(f, _) => {
                    self.logic_functions
                        .insert(f.name.clone(), Rc::new(f.clone()));
                }
                Item::UiFn(ui) => {
                    self.ui_functions
                        .insert(ui.name.clone(), Rc::new(ui.clone()));
                }
                Item::Impl(block) => {
                    if let tint_ast::Type::Simple(target) = &block.target {
                        for method in &block.methods {
                            self.impl_methods.insert(
                                (target.clone(), method.name.clone()),
                                Rc::new(method.clone()),
                            );
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn register_constants(&mut self, program: &Program) {
        for item in &program.items {
            if let Item::Const(constant) = item {
                let value = self.eval_expr(&constant.init);
                let value = match constant.ty.as_ref() {
                    Some(ty) if is_exact_numeric_type(ty) => value
                        .cast_numeric(ty)
                        .unwrap_or_else(|message| panic!("numeric constant type error: {message}")),
                    _ => value,
                };
                self.define_var(&constant.name, value);
                #[cfg(feature = "ir")]
                {
                    self.has_constants = true;
                }
            }
        }
    }

    fn register_host_builtins(
        &mut self,
        storage: Rc<RefCell<HashMap<String, String>>>,
        http_requests: Rc<RefCell<Vec<HttpRequest>>>,
    ) {
        self.register_native("vec2", |args| {
            let x = number_arg(args, 0, "vec2")?;
            let y = number_arg(args, 1, "vec2")?;
            Ok(EvalValue::StructInstance {
                name: "Vec2".into(),
                fields: vec![
                    ("x".into(), EvalValue::Number(x)),
                    ("y".into(), EvalValue::Number(y)),
                ],
            })
        });

        self.register_native("clamp", |args| {
            let value = number_arg(args, 0, "clamp")?;
            let min = number_arg(args, 1, "clamp")?;
            let max = number_arg(args, 2, "clamp")?;
            Ok(EvalValue::Number(value.max(min).min(max)))
        });

        self.register_native("min", |args| {
            let a = number_arg(args, 0, "min")?;
            let b = number_arg(args, 1, "min")?;
            Ok(EvalValue::Number(a.min(b)))
        });

        self.register_native("max", |args| {
            let a = number_arg(args, 0, "max")?;
            let b = number_arg(args, 1, "max")?;
            Ok(EvalValue::Number(a.max(b)))
        });

        self.register_native("abs", |args| {
            Ok(EvalValue::Number(number_arg(args, 0, "abs")?.abs()))
        });

        self.register_native("sign", |args| {
            let value = number_arg(args, 0, "sign")?;
            Ok(EvalValue::Number(if value > 0.0 {
                1.0
            } else if value < 0.0 {
                -1.0
            } else {
                0.0
            }))
        });

        let storage_get = Rc::clone(&storage);
        self.register_native("storage_get", move |args| {
            let key = string_arg(args, 0, "storage_get")?;
            Ok(storage_get
                .borrow()
                .get(&key)
                .cloned()
                .map(EvalValue::String)
                .unwrap_or(EvalValue::Unit))
        });

        let storage_set = Rc::clone(&storage);
        self.register_native("storage_set", move |args| {
            let key = string_arg(args, 0, "storage_set")?;
            let value = string_arg(args, 1, "storage_set")?;
            storage_set.borrow_mut().insert(key, value);
            Ok(EvalValue::Unit)
        });

        let storage_remove = Rc::clone(&storage);
        self.register_native("storage_remove", move |args| {
            let key = string_arg(args, 0, "storage_remove")?;
            storage_remove.borrow_mut().remove(&key);
            Ok(EvalValue::Unit)
        });

        let next_request_id = Rc::new(RefCell::new(1_u64));
        self.register_native("http_get", move |args| {
            let url = string_arg(args, 0, "http_get")?;
            let mut next = next_request_id.borrow_mut();
            let id = *next;
            *next = next.saturating_add(1);
            http_requests.borrow_mut().push(HttpRequest {
                id,
                method: "GET".to_string(),
                url,
            });
            Ok(EvalValue::Number(id as f64))
        });
    }

    pub fn storage_snapshot(&self) -> HashMap<String, String> {
        self.storage.borrow().clone()
    }

    pub fn hydrate_storage(&mut self, values: HashMap<String, String>) {
        self.storage.borrow_mut().extend(values);
    }

    pub fn take_http_requests(&mut self) -> Vec<HttpRequest> {
        std::mem::take(&mut *self.http_requests.borrow_mut())
    }
}

fn is_exact_numeric_type(ty: &tint_ast::Type) -> bool {
    matches!(ty, tint_ast::Type::Simple(name) if matches!(name.as_str(), "i32" | "i64" | "u8" | "u32" | "u64" | "f32" | "f64"))
}

fn string_arg(args: &[EvalValue], index: usize, function: &str) -> EvalResult<String> {
    match args.get(index) {
        Some(EvalValue::String(value)) => Ok(value.clone()),
        _ => Err(tint_evaluator::errors::EvalError::CallError {
            msg: format!("{} expects string argument {}", function, index + 1),
            span: tint_ast::Span::dummy(),
        }),
    }
}

fn number_arg(args: &[EvalValue], index: usize, function: &str) -> EvalResult<f64> {
    match args.get(index) {
        Some(EvalValue::Number(value)) => Ok(*value),
        _ => Err(tint_evaluator::errors::EvalError::CallError {
            msg: format!("{} expects number argument {}", function, index + 1),
            span: tint_ast::Span::dummy(),
        }),
    }
}
