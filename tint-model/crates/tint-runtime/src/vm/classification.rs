use super::*;

impl TintVM {
    pub(super) fn mount_ui(&mut self, program: &Program) {
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
    pub(super) fn is_ui_function(&self, name: &str) -> bool {
        self.ui_functions.contains_key(name)
    }

    // Eval helpers
    pub(super) fn eval_to_rt(v: EvalValue) -> RuntimeValue {
        match v {
            EvalValue::Number(n) => RuntimeValue::Number(n),
            EvalValue::I32(n) => RuntimeValue::I32(n),
            EvalValue::I64(n) => RuntimeValue::I64(n),
            EvalValue::U32(n) => RuntimeValue::U32(n),
            EvalValue::U64(n) => RuntimeValue::U64(n),
            EvalValue::U8(n) => RuntimeValue::U8(n),
            EvalValue::F32(n) => RuntimeValue::F32(n),
            EvalValue::F64(n) => RuntimeValue::F64(n),
            EvalValue::String(s) => RuntimeValue::String(s),
            EvalValue::Bool(b) => RuntimeValue::Bool(b),
            EvalValue::Unit => RuntimeValue::Unit,
            EvalValue::Tuple(items) => {
                RuntimeValue::Tuple(items.into_iter().map(Self::eval_to_rt).collect())
            }
            EvalValue::List(items) => {
                RuntimeValue::List(items.into_iter().map(Self::eval_to_rt).collect())
            }
            EvalValue::StructInstance { name, fields } => RuntimeValue::StructInstance {
                name,
                fields: fields
                    .into_iter()
                    .map(|(k, v)| (k, Self::eval_to_rt(v)))
                    .collect(),
            },
            EvalValue::EnumInstance {
                enum_name,
                variant,
                args,
            } => RuntimeValue::EnumInstance {
                enum_name,
                variant,
                args: args.into_iter().map(Self::eval_to_rt).collect(),
            },
            EvalValue::Map(map) => RuntimeValue::Map(
                map.into_iter()
                    .map(|(k, v)| (k, Self::eval_to_rt(v)))
                    .collect(),
            ),
            EvalValue::Lambda {
                params,
                body,
                closure,
            } => RuntimeValue::Lambda {
                params,
                body,
                closure,
            },
            EvalValue::Function {
                params, body, env, ..
            } => RuntimeValue::FunctionValue { params, body, env },
            other => panic!("Unsupported EvalValue: {:?}", other),
        }
    }

    pub(super) fn rt_to_eval(v: &RuntimeValue) -> EvalValue {
        match v {
            RuntimeValue::Number(n) => EvalValue::Number(*n),
            RuntimeValue::I32(n) => EvalValue::I32(*n),
            RuntimeValue::I64(n) => EvalValue::I64(*n),
            RuntimeValue::U32(n) => EvalValue::U32(*n),
            RuntimeValue::U64(n) => EvalValue::U64(*n),
            RuntimeValue::U8(n) => EvalValue::U8(*n),
            RuntimeValue::F32(n) => EvalValue::F32(*n),
            RuntimeValue::F64(n) => EvalValue::F64(*n),
            RuntimeValue::String(s) => EvalValue::String(s.clone()),
            RuntimeValue::Bool(b) => EvalValue::Bool(*b),
            RuntimeValue::Tuple(items) => {
                EvalValue::Tuple(items.iter().map(Self::rt_to_eval).collect())
            }
            RuntimeValue::List(items) => {
                EvalValue::List(items.iter().map(Self::rt_to_eval).collect())
            }
            RuntimeValue::StructInstance { name, fields } => EvalValue::StructInstance {
                name: name.clone(),
                fields: fields
                    .iter()
                    .map(|(k, v)| (k.clone(), Self::rt_to_eval(v)))
                    .collect(),
            },
            RuntimeValue::EnumInstance {
                enum_name,
                variant,
                args,
            } => EvalValue::EnumInstance {
                enum_name: enum_name.clone(),
                variant: variant.clone(),
                args: args.iter().map(Self::rt_to_eval).collect(),
            },
            RuntimeValue::Map(map) => EvalValue::Map(
                map.iter()
                    .map(|(k, v)| (k.clone(), Self::rt_to_eval(v)))
                    .collect(),
            ),
            RuntimeValue::Lambda {
                params,
                body,
                closure,
            } => EvalValue::Lambda {
                params: params.clone(),
                body: body.clone(),
                closure: Rc::clone(closure),
            },
            RuntimeValue::FunctionValue { params, body, env } => EvalValue::Function {
                name: "<function>".into(),
                params: params.clone(),
                body: body.clone(),
                env: Rc::clone(env),
                async_: false,
            },
            _ => EvalValue::Unit,
        }
    }

    // Block execution for old evaluator
    pub(super) fn eval_block_flow(&mut self, block: &Block) -> Flow {
        let mut last = Flow::Value(EvalValue::Unit);

        for stmt in &block.stmts {
            let flow = eval_stmt::eval_stmt(self, stmt);

            match flow {
                Flow::Value(_) => last = flow,
                Flow::Return(_) | Flow::Propagate(_) | Flow::Break | Flow::Continue => return flow,
            }
        }

        last
    }
}
