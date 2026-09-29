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

        // A bare `Some(x)` / `Ok(x)` / `Err(x)` builds the variant, unless a
        // user function of that name exists.
        if let ("Some" | "Ok" | "Err", 1, false) =
            (name, args.len(), self.logic_functions.contains_key(name))
        {
            let enum_name = if name == "Some" { "Option" } else { "Result" };
            return Ok(EvalValue::EnumInstance {
                enum_name: enum_name.into(),
                variant: name.into(),
                args: args.to_vec(),
            });
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

        // Optional second tier (cargo feature `ir`): plain logic functions may
        // run on the SSA/IR VM. Off by default -- the tree-walker is the
        // single execution engine.
        #[cfg(feature = "ir")]
        if let Some(result) = self.try_call_via_ir(name, args, span) {
            return result;
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

        let slots = args.iter().cloned().map(Some).collect();
        self.host_call_user_fn_slots(name, slots, span)
    }

    /// A call whose arguments were matched by name (`f{b: 2, a: 1}`).
    pub(super) fn host_call_fn_named(
        &mut self,
        name: &str,
        arg_exprs: &[tint_ast::Expr],
        arg_vals: &[EvalValue],
        span: Span,
    ) -> EvalResult<EvalValue> {
        let invalid = |msg: String| tint_evaluator::errors::EvalError::InvalidOp { msg, span };
        let Some(func) = self.logic_functions.get(name).cloned() else {
            return Err(invalid(format!("named arguments need a user function, `{name}` is not one")));
        };
        let sigs: Vec<_> = func.params.iter().map(|p| p.sig()).collect();
        let bound = tint_ast::bind_call_args(name, &sigs, arg_exprs).map_err(invalid)?;
        let slots = bound.into_iter().map(|slot| slot.map(|i| arg_vals[i].clone())).collect();
        self.host_call_user_fn_slots(name, slots, span)
    }

    /// One slot per parameter (missing trailing slots count as `None`); a
    /// `None` slot takes the parameter's default, evaluated before the
    /// callee's scope exists.
    fn host_call_user_fn_slots(
        &mut self,
        name: &str,
        slots: Vec<Option<EvalValue>>,
        span: Span,
    ) -> EvalResult<EvalValue> {
        let func = match self.logic_functions.get(name).cloned() {
            Some(f) => f,
            None => {
                return Err(tint_evaluator::errors::EvalError::InvalidOp {
                    msg: format!("Unknown function '{}'", name),
                    span,
                })
            }
        };

        let mut args = Vec::with_capacity(func.params.len());
        for (index, param) in func.params.iter().enumerate() {
            match slots.get(index).cloned().flatten() {
                Some(value) => args.push(value),
                None => match &param.default {
                    Some(default) => args.push(self.eval_expr(default.expr())),
                    // A `call_fn` error is swallowed by the evaluator (it then
                    // tries the name as a variable), so this one is a panic
                    // like the parameter type errors below.
                    None => panic!("missing argument {} in call of `{name}`", index + 1),
                },
            }
        }
        let args = &args[..];

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
            if func.generics.is_empty() {
                bind_pattern(self, &param.pattern, &value);
            } else {
                // A type parameter is not enforced at run time.
                bind_pattern(self, &erase_type_params(&param.pattern, &func.generics), &value);
            }
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

    pub(crate) fn host_call_method(
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

        if let Some(result) = self.option_result_method(&receiver, method, args, span) {
            return result.map(|value| (receiver, value));
        }

        if let Some(result) = self.collection_method(&receiver, method, args, span) {
            return result;
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

/// `x: T` as plain `x` when `T` is one of `generics`.
fn erase_type_params(pattern: &tint_ast::Pattern, generics: &[String]) -> tint_ast::Pattern {
    use tint_ast::{Pattern, Type};
    match pattern {
        Pattern::Typed { pat, ty: Type::Simple(name), .. } if generics.contains(name) => {
            erase_type_params(pat, generics)
        }
        Pattern::Mut { inner, span } => Pattern::Mut {
            inner: Box::new(erase_type_params(inner, generics)),
            span: *span,
        },
        other => other.clone(),
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
