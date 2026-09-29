use super::super::*;

impl TintVM {
    pub(crate) fn host_call_fn(
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

        // A helper called from a UI event/frame handler must stay on the
        // tree-walking VM: that is the VM which owns the persistent `state`
        // scope.  The IR VM is intentionally isolated and is still used for
        // ordinary top-level logic calls.
        if self.treewalk_call_depth > 0 && self.is_ir_function(name) {
            return self.host_call_user_fn(name, args, span);
        }

        // The IR backend currently has no closure value or indirect call.
        // Route functions that actually contain a lambda through the
        // evaluator, which already supports lexical capture and lambda calls.
        if self.is_ir_function(name)
            && (self.uses_lambda(name)
                || self.uses_function_value(name)
                || self.uses_interpreter_control_flow(name)
                || self.uses_try(name)
                || self.uses_fallible_method(name)
                || self.uses_cast(name)
                || self.uses_exact_numeric(name)
                || self.has_constants
                || self.has_function_values())
        {
            return self.host_call_user_fn(name, args, span);
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

            let mut method_call = |receiver: IrValue, method: &str, method_args: &[IrValue]| {
                let receiver = Self::ir_value_to_eval_value(receiver);
                let args: Vec<EvalValue> = method_args
                    .iter()
                    .cloned()
                    .map(Self::ir_value_to_eval_value)
                    .collect();
                let updated = match self.host_call_method(receiver, method, &args, span) {
                    Ok(updated) => updated,
                    Err(error) => panic!("IR method call `{}` failed: {:?}", method, error),
                };
                Some((
                    Self::eval_value_to_ir_value(updated.0),
                    Self::eval_value_to_ir_value(updated.1),
                ))
            };
            irvm.set_method_call(&mut method_call);

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
        self.treewalk_call_depth += 1;

        // bind params
        for (param, arg) in func.params.iter().zip(args.iter()) {
            let value = match param.ty.as_ref() {
                Some(ty) if matches!(ty, tint_ast::Type::Simple(name) if matches!(name.as_str(), "i32" | "i64" | "u8" | "u32" | "u64" | "f32" | "f64")) => {
                    arg.cast_numeric(ty)
                        .unwrap_or_else(|message| panic!("numeric parameter type error: {message}"))
                }
                _ => arg.clone(),
            };
            bind_pattern(self, &param.pattern, &value);
        }

        // execute
        let result = match &func.body {
            FnBody::Block(block) => match self.eval_block(block) {
                Flow::Value(v) | Flow::Return(v) | Flow::Propagate(v) => v,
                _ => EvalValue::Unit,
            },
            FnBody::Expr(expr) => self.eval_expr(expr),
        };
        let result = match func.ret_ty.as_ref() {
            Some(ty) if matches!(ty, tint_ast::Type::Simple(name) if matches!(name.as_str(), "i32" | "i64" | "u8" | "u32" | "u64" | "f32" | "f64")) => {
                result
                    .cast_numeric(ty)
                    .unwrap_or_else(|message| panic!("numeric return type error: {message}"))
            }
            _ => result,
        };

        self.treewalk_call_depth -= 1;
        self.scopes.pop();
        Ok(result)
    }

    pub(super) fn host_call_method(
        &mut self,
        receiver: EvalValue,
        method: &str,
        args: &[EvalValue],
        span: Span,
    ) -> EvalResult<(EvalValue, EvalValue)> {
        if let EvalValue::EnumInstance {
            enum_name,
            variant,
            args: values,
        } = &receiver
        {
            if (enum_name == "Option" || enum_name == "Result")
                && (method == "expect" || method == "unwrap")
            {
                if method == "expect" && args.len() != 1 {
                    return Err(tint_evaluator::errors::EvalError::InvalidOp {
                        msg: "expect expects one message argument".into(),
                        span,
                    });
                }
                if method == "unwrap" && !args.is_empty() {
                    return Err(tint_evaluator::errors::EvalError::InvalidOp {
                        msg: "unwrap expects no arguments".into(),
                        span,
                    });
                }

                let success = (enum_name == "Option" && variant == "Some")
                    || (enum_name == "Result" && variant == "Ok");
                if let (true, Some(value)) = (success, values.first()) {
                    return Ok((receiver.clone(), value.clone()));
                }

                return Err(tint_evaluator::errors::EvalError::InvalidOp {
                    msg: if method == "expect" {
                        "expect called on an empty or failed value".into()
                    } else {
                        "unwrap called on an empty or failed value".into()
                    },
                    span,
                });
            }
        }

        if let EvalValue::EnumInstance {
            enum_name,
            variant,
            args: values,
        } = receiver.clone()
        {
            let is_option = enum_name == "Option";
            let is_result = enum_name == "Result";
            if (is_option || is_result)
                && matches!(
                    method,
                    "is_some" | "is_none" | "is_ok" | "is_err" | "unwrap_or" | "map" | "and_then"
                )
            {
                let success = (is_option && variant == "Some") || (is_result && variant == "Ok");
                match method {
                    "is_some" => return Ok((receiver, EvalValue::Bool(is_option && success))),
                    "is_none" => return Ok((receiver, EvalValue::Bool(is_option && !success))),
                    "is_ok" => return Ok((receiver, EvalValue::Bool(is_result && success))),
                    "is_err" => return Ok((receiver, EvalValue::Bool(is_result && !success))),
                    "unwrap_or" => {
                        if args.len() != 1 {
                            return Err(tint_evaluator::errors::EvalError::InvalidOp {
                                msg: "unwrap_or expects one argument".into(),
                                span,
                            });
                        }
                        return Ok((
                            receiver.clone(),
                            values
                                .first()
                                .cloned()
                                .filter(|_| success)
                                .unwrap_or_else(|| args[0].clone()),
                        ));
                    }
                    "map" | "and_then" => {
                        if args.len() != 1 {
                            return Err(tint_evaluator::errors::EvalError::InvalidOp {
                                msg: format!("{method} expects one callable argument"),
                                span,
                            });
                        }
                        if !success {
                            return Ok((receiver.clone(), receiver));
                        }
                        let value = values.first().cloned().unwrap_or(EvalValue::Unit);
                        let mapped = self.host_call_value(args[0].clone(), &[value], span);
                        if method == "and_then" {
                            return Ok((receiver.clone(), mapped));
                        }
                        return Ok((
                            receiver.clone(),
                            EvalValue::EnumInstance {
                                enum_name,
                                variant,
                                args: vec![mapped],
                            },
                        ));
                    }
                    _ => unreachable!(),
                }
            }
        }

        let type_name = match &receiver {
            EvalValue::StructInstance { name, .. } => name.clone(),
            _ => {
                return Err(tint_evaluator::errors::EvalError::InvalidOp {
                    msg: format!("Cannot call method '{}' on a non-struct value", method),
                    span,
                })
            }
        };

        if type_name == "Vec2" {
            return vec2_method(receiver, method, args, span);
        }

        let func = match self
            .impl_methods
            .get(&(type_name.clone(), method.to_string()))
            .cloned()
        {
            Some(func) => func,
            None => {
                return Err(tint_evaluator::errors::EvalError::InvalidOp {
                    msg: format!("Unknown method '{}.{}'", type_name, method),
                    span,
                })
            }
        };

        self.scopes.push();
        self.treewalk_call_depth += 1;

        // Methods receive the receiver as their first argument. `self` and
        // `mut self` are both represented by the existing pattern AST; the
        // borrow spelling can be added later without changing dispatch.
        let mut all_args = Vec::with_capacity(args.len() + 1);
        all_args.push(receiver.clone());
        all_args.extend_from_slice(args);
        for (param, arg) in func.params.iter().zip(all_args.iter()) {
            bind_pattern(self, &param.pattern, arg);
        }

        let result = match &func.body {
            FnBody::Block(block) => match self.eval_block(block) {
                Flow::Value(v) | Flow::Return(v) | Flow::Propagate(v) => v,
                _ => EvalValue::Unit,
            },
            FnBody::Expr(expr) => self.eval_expr(expr),
        };
        let updated_receiver = self.host_load_var("self", span);

        self.treewalk_call_depth -= 1;
        self.scopes.pop();

        let updated_receiver = if matches!(updated_receiver, EvalValue::Unit) {
            receiver
        } else {
            updated_receiver
        };
        Ok((updated_receiver, result))
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

            EvalValue::Lambda {
                params,
                body,
                closure,
            } => {
                self.scopes.push();
                for (name, value) in closure.values() {
                    self.host_define_var(&name, value);
                }
                for (param, arg) in params.iter().zip(args.iter()) {
                    self.host_define_var(param, arg.clone());
                }
                let result = self.eval_expr(&body);
                self.scopes.pop();
                result
            }

            EvalValue::Function {
                params, body, env, ..
            } => {
                self.scopes.push();
                for (name, value) in env.values() {
                    self.host_define_var(&name, value);
                }
                for (param, arg) in params.iter().zip(args.iter()) {
                    self.host_define_var(param, arg.clone());
                }
                let result = match body {
                    tint_evaluator::eval_fn::FnBodyKind::Block(block) => {
                        match self.eval_block(&block) {
                            Flow::Value(v) | Flow::Return(v) | Flow::Propagate(v) => v,
                            Flow::Break | Flow::Continue => EvalValue::Unit,
                        }
                    }
                    tint_evaluator::eval_fn::FnBodyKind::Expr(expr) => self.eval_expr(&expr),
                };
                self.scopes.pop();
                result
            }

            other => panic!("host_call_value not supported: {:?}", other),
        }
    }
}

fn vec2_method(
    receiver: EvalValue,
    method: &str,
    args: &[EvalValue],
    span: Span,
) -> EvalResult<(EvalValue, EvalValue)> {
    let EvalValue::StructInstance { fields, .. } = &receiver else {
        unreachable!("Vec2 method receiver was checked before dispatch")
    };

    let field = |name: &str| {
        fields
            .iter()
            .find(|(field, _)| field == name)
            .and_then(|(_, value)| match value {
                EvalValue::Number(value) => Some(*value),
                _ => None,
            })
            .ok_or(tint_evaluator::errors::EvalError::InvalidOp {
                msg: format!("Vec2 field '{}' must be a number", name),
                span,
            })
    };

    let x = field("x")?;
    let y = field("y")?;
    let make = |x, y| EvalValue::StructInstance {
        name: "Vec2".into(),
        fields: vec![
            ("x".into(), EvalValue::Number(x)),
            ("y".into(), EvalValue::Number(y)),
        ],
    };

    let result = match method {
        "add" | "sub" => {
            let other = match args.first() {
                Some(EvalValue::StructInstance { name, fields }) if name == "Vec2" => {
                    let get = |name: &str| {
                        fields
                            .iter()
                            .find(|(field, _)| field == name)
                            .and_then(|(_, value)| match value {
                                EvalValue::Number(value) => Some(*value),
                                _ => None,
                            })
                            .ok_or(tint_evaluator::errors::EvalError::InvalidOp {
                                msg: format!("Vec2 field '{}' must be a number", name),
                                span,
                            })
                    };
                    (get("x")?, get("y")?)
                }
                _ => {
                    return Err(tint_evaluator::errors::EvalError::InvalidOp {
                        msg: format!("Vec2.{} expects another Vec2", method),
                        span,
                    })
                }
            };
            if method == "add" {
                make(x + other.0, y + other.1)
            } else {
                make(x - other.0, y - other.1)
            }
        }
        "scale" => {
            let scalar = match args.first() {
                Some(EvalValue::Number(value)) => *value,
                _ => {
                    return Err(tint_evaluator::errors::EvalError::InvalidOp {
                        msg: "Vec2.scale expects a number".into(),
                        span,
                    })
                }
            };
            make(x * scalar, y * scalar)
        }
        "length" => EvalValue::Number((x * x + y * y).sqrt()),
        "normalized" => {
            let length = (x * x + y * y).sqrt();
            if length == 0.0 {
                make(0.0, 0.0)
            } else {
                make(x / length, y / length)
            }
        }
        _ => {
            return Err(tint_evaluator::errors::EvalError::InvalidOp {
                msg: format!("Unknown Vec2 method '{}.{}'", "Vec2", method),
                span,
            })
        }
    };

    Ok((receiver, result))
}
