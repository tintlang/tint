use super::super::*;

impl TintVM {
    pub(super) fn host_call_fn(
        &mut self,
        name: &str,
        args: &[EvalValue],
        span: Span,
    ) -> EvalResult<EvalValue> {
        // 0) user-registered native Rust functions (see `register_native`) --
        // checked first so a native fn can shadow a builtin/UI/logic fn on
        // purpose.
        if let Some(f) = self.native_fns.borrow().get(name) {
            return f(args);
        }

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
            let params: Vec<tint_ast::Pattern> = self
                .logic_functions
                .get(name)
                .map(|f| f.params.iter().map(|p| p.pattern.clone()).collect())
                .unwrap_or_default();
            let ir_args: Vec<IrValue> = args
                .iter()
                .map(|a| Self::eval_value_to_ir_value(a.clone()))
                .collect();

            let mut irvm = IrVM::new(self.ir_program.clone());

            // Give the IR VM a way to reach a registered native fn from
            // INSIDE a plain `fn`'s own body (`fn go() { shout("hi") }`,
            // called the normal top-level way, so `Instr::Call` runs
            // inside this very `IrVM`) -- previously unreachable, since
            // `tint-ir` has no dependency on this crate's `native_fns`
            // table at all (see `tint-runtime/tests/native_fn.rs` and
            // `IrVM::set_native_call`'s doc comment). `Rc::clone` here is
            // what lets this closure outlive `self`'s borrow: it owns its
            // own handle to the SAME table, rather than borrowing `self`.
            let native_fns = Rc::clone(&self.native_fns);
            irvm.set_native_call(Box::new(move |name, ir_args| {
                let eval_args: Vec<EvalValue> = ir_args
                    .iter()
                    .cloned()
                    .map(Self::ir_value_to_eval_value)
                    .collect();

                let f = native_fns.borrow();
                let f = f.get(name)?;
                match f(&eval_args) {
                    Ok(v) => Some(Self::eval_value_to_ir_value(v)),
                    // A native fn CAN fail (e.g. bad args) -- that's a
                    // real error, not "no such function", so this
                    // doesn't silently fall back to Unit the way an
                    // unresolved name does; it panics with the native's
                    // own error message, same as any other unrecoverable
                    // runtime error in this VM (`Instr::Index` out of
                    // bounds, `Instr::LoadLocal` on an undefined name, …).
                    Err(e) => panic!("native fn `{}` failed: {}", name, e),
                }
            }));

            let out = irvm.run_with_args(name, &params, &ir_args);
            return Ok(Self::ir_value_to_eval_value(out));
        }

        // 4) fallback
        self.host_call_user_fn(name, args, span)
    }

    pub(super) fn host_call_user_fn(
        &mut self,
        name: &str,
        args: &[EvalValue],
        span: Span,
    ) -> EvalResult<EvalValue> {
        // Native Rust functions are reachable from this path too, so a
        // `click||`/`hover_in||` handler can be a real Rust closure, not
        // just a Tint `fn`.
        if let Some(f) = self.native_fns.borrow().get(name) {
            return f(args);
        }

        let func = match self.logic_functions.get(name).cloned() {
            Some(f) => f,
            None => {
                return Err(tint_evaluator::errors::EvalError::InvalidOp {
                    msg: format!("Unknown function '{}'", name),
                    span,
                })
            }
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
    pub(super) fn host_call_value(
        &mut self,
        v: EvalValue,
        args: &[EvalValue],
        span: Span,
    ) -> EvalValue {
        match v {
            // if the IR passed the function name as a string
            EvalValue::String(name) => {
                return self
                    .host_call_fn(&name, args, span)
                    .unwrap_or(EvalValue::Unit);
            }

            // if the IR passed Unit: treat it as a no-op
            EvalValue::Unit => EvalValue::Unit,

            other => panic!("host_call_value not supported: {:?}", other),
        }
    }
}
