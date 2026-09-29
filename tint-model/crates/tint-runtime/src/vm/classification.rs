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

    pub(super) fn is_ir_function(&self, name: &str) -> bool {
        self.logic_functions.contains_key(name)
    }

    /// The SSA backend does not yet have a closure value or an indirect-call
    /// instruction. Keep lambda-containing functions on the evaluator path so
    /// lambdas are fully usable at runtime (including captured locals) instead
    /// of compiling to the IR backend's old `Unit` placeholder.
    pub(super) fn uses_lambda(&self, name: &str) -> bool {
        let Some(function) = self.logic_functions.get(name) else {
            return false;
        };

        match &function.body {
            FnBody::Expr(expr) => expr_contains_lambda(expr),
            FnBody::Block(block) => block_contains_lambda(block),
        }
    }

    /// Function values cannot cross the current SSA/IR boundary. Any
    /// function accepting an explicit function type therefore stays on the
    /// evaluator path, where indirect calls are supported.
    pub(super) fn uses_function_value(&self, name: &str) -> bool {
        self.logic_functions.get(name).is_some_and(|f| {
            f.params
                .iter()
                .any(|p| matches!(p.ty, Some(tint_ast::Type::Function { .. })))
        })
    }

    /// Loops are currently evaluated by the tree-walking backend. The SSA
    /// compiler can parse these statements but does not lower them yet.
    pub(super) fn uses_interpreter_control_flow(&self, name: &str) -> bool {
        self.logic_functions.get(name).is_some_and(
            |function| matches!(&function.body, FnBody::Block(block) if block_contains_loop(block)),
        )
    }

    pub(super) fn uses_try(&self, name: &str) -> bool {
        self.logic_functions
            .get(name)
            .is_some_and(|f| match &f.body {
                FnBody::Expr(expr) => expr_contains_try(expr),
                FnBody::Block(block) => block.stmts.iter().any(stmt_contains_try),
            })
    }

    pub(super) fn uses_fallible_method(&self, name: &str) -> bool {
        self.logic_functions
            .get(name)
            .is_some_and(|f| match &f.body {
                FnBody::Expr(expr) => expr_contains_fallible_method(expr),
                FnBody::Block(block) => block.stmts.iter().any(|stmt| match stmt {
                    tint_ast::Stmt::Let { init, .. } => expr_contains_fallible_method(init.expr()),
                    tint_ast::Stmt::Expr(expr) | tint_ast::Stmt::Return(expr, _) => {
                        expr_contains_fallible_method(expr)
                    }
                    _ => false,
                }),
            })
    }

    pub(super) fn uses_cast(&self, name: &str) -> bool {
        self.logic_functions
            .get(name)
            .is_some_and(|f| match &f.body {
                FnBody::Expr(expr) => expr_contains_cast(expr),
                FnBody::Block(block) => block.stmts.iter().any(|stmt| match stmt {
                    tint_ast::Stmt::Let { init, .. } => expr_contains_cast(init.expr()),
                    tint_ast::Stmt::Expr(expr) | tint_ast::Stmt::Return(expr, _) => {
                        expr_contains_cast(expr)
                    }
                    _ => false,
                }),
            })
    }

    pub(super) fn uses_exact_numeric(&self, name: &str) -> bool {
        self.logic_functions.get(name).is_some_and(|f| {
            f.ret_ty.as_ref().is_some_and(is_exact_numeric_type)
                || f.params
                    .iter()
                    .any(|param| param.ty.as_ref().is_some_and(is_exact_numeric_type))
                || match &f.body {
                    FnBody::Block(block) => block.stmts.iter().any(|stmt| match stmt {
                        tint_ast::Stmt::Let { pattern, .. } => {
                            pattern_contains_exact_numeric(pattern)
                        }
                        _ => false,
                    }),
                    FnBody::Expr(_) => false,
                }
        })
    }

    pub(super) fn has_function_values(&self) -> bool {
        self.logic_functions
            .values()
            .any(|f| f.params.iter().any(|p| p.ty.is_some()))
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

fn block_contains_lambda(block: &Block) -> bool {
    block.stmts.iter().any(stmt_contains_lambda)
}

fn block_contains_loop(block: &Block) -> bool {
    block.stmts.iter().any(stmt_contains_loop)
}

fn stmt_contains_loop(stmt: &tint_ast::Stmt) -> bool {
    use tint_ast::Stmt;

    match stmt {
        Stmt::While { .. } | Stmt::Loop { .. } | Stmt::For { .. } => true,
        Stmt::If { then, else_, .. } => {
            block_contains_loop(then) || else_.as_ref().is_some_and(block_contains_loop)
        }
        Stmt::Match { arms, .. } => arms.iter().any(|arm| {
            matches!(&arm.expr, tint_ast::Expr::Block(block, _) if block_contains_loop(block))
        }),
        Stmt::Let { .. }
        | Stmt::Assign { .. }
        | Stmt::CompoundAssign { .. }
        | Stmt::Expr(_)
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Return(_, _) => false,
    }
}

fn stmt_contains_lambda(stmt: &tint_ast::Stmt) -> bool {
    use tint_ast::Stmt;

    match stmt {
        Stmt::Let { init, .. } => expr_contains_lambda(init.expr()),
        Stmt::Assign { lhs, rhs, .. } => expr_contains_lambda(lhs) || expr_contains_lambda(rhs),
        Stmt::CompoundAssign { expr, .. } => expr_contains_lambda(expr),
        Stmt::Expr(expr) | Stmt::Return(expr, _) => expr_contains_lambda(expr),
        Stmt::If {
            cond, then, else_, ..
        } => {
            expr_contains_lambda(cond)
                || block_contains_lambda(then)
                || else_.as_ref().is_some_and(block_contains_lambda)
        }
        Stmt::While { cond, body, .. } => expr_contains_lambda(cond) || block_contains_lambda(body),
        Stmt::Loop { body, .. } => block_contains_lambda(body),
        Stmt::Break(_) | Stmt::Continue(_) => false,
        Stmt::For {
            start, end, body, ..
        } => {
            expr_contains_lambda(start) || expr_contains_lambda(end) || block_contains_lambda(body)
        }
        Stmt::Match { expr, arms, .. } => {
            expr_contains_lambda(expr)
                || arms.iter().any(|arm| {
                    arm.guard
                        .as_ref()
                        .is_some_and(|guard| expr_contains_lambda(guard))
                        || expr_contains_lambda(&arm.expr)
                })
        }
    }
}

fn stmt_contains_try(stmt: &tint_ast::Stmt) -> bool {
    use tint_ast::Stmt;
    match stmt {
        Stmt::Let { init, .. } => expr_contains_try(init.expr()),
        Stmt::Assign { lhs, rhs, .. } => expr_contains_try(lhs) || expr_contains_try(rhs),
        Stmt::CompoundAssign { expr, .. } => expr_contains_try(expr),
        Stmt::Expr(expr) | Stmt::Return(expr, _) => expr_contains_try(expr),
        Stmt::If {
            cond, then, else_, ..
        } => {
            expr_contains_try(cond)
                || then.stmts.iter().any(stmt_contains_try)
                || else_
                    .as_ref()
                    .is_some_and(|b| b.stmts.iter().any(stmt_contains_try))
        }
        Stmt::While { cond, body, .. } => {
            expr_contains_try(cond) || body.stmts.iter().any(stmt_contains_try)
        }
        Stmt::Loop { body, .. } => body.stmts.iter().any(stmt_contains_try),
        Stmt::For {
            start, end, body, ..
        } => {
            expr_contains_try(start)
                || expr_contains_try(end)
                || body.stmts.iter().any(stmt_contains_try)
        }
        Stmt::Match { expr, arms, .. } => {
            expr_contains_try(expr)
                || arms.iter().any(|a| {
                    expr_contains_try(&a.expr) || a.guard.as_ref().is_some_and(expr_contains_try)
                })
        }
        Stmt::Break(_) | Stmt::Continue(_) => false,
    }
}

fn expr_contains_lambda(expr: &Expr) -> bool {
    use tint_ast::Expr;

    match expr {
        Expr::Lambda { .. } => true,
        Expr::InterpolatedString { parts, .. } => parts.iter().any(|part| match part {
            tint_ast::StringPart::Text(_) => false,
            tint_ast::StringPart::Expr(expr) => expr_contains_lambda(expr),
        }),
        Expr::Unary { expr, .. }
        | Expr::Paren(expr, _)
        | Expr::Try { expr, .. }
        | Expr::Cast { expr, .. }
        | Expr::Borrow { target: expr, .. }
        | Expr::TupleIndex { target: expr, .. } => expr_contains_lambda(expr),
        Expr::Binary { left, right, .. } => {
            expr_contains_lambda(left) || expr_contains_lambda(right)
        }
        Expr::Field { target, .. }
        | Expr::Namespace { base: target, .. }
        | Expr::Index { target, .. } => expr_contains_lambda(target),
        Expr::Call { target, args, .. } => {
            expr_contains_lambda(target) || args.iter().any(expr_contains_lambda)
        }
        Expr::Match {
            scrutinee, arms, ..
        } => {
            expr_contains_lambda(scrutinee)
                || arms.iter().any(|arm| {
                    arm.guard
                        .as_ref()
                        .is_some_and(|guard| expr_contains_lambda(guard))
                        || expr_contains_lambda(&arm.expr)
                })
        }
        Expr::If { .. } => true,
        Expr::StructInit { fields, .. } | Expr::VariantInit { fields, .. } => {
            fields.iter().any(|field| match field {
                tint_ast::StructInitField::Assign { expr, .. }
                | tint_ast::StructInitField::Tint { expr, .. } => expr_contains_lambda(expr),
            })
        }
        Expr::StructUpdate { base, updates, .. } => {
            expr_contains_lambda(base)
                || updates.iter().any(|field| match field {
                    tint_ast::StructInitField::Assign { expr, .. }
                    | tint_ast::StructInitField::Tint { expr, .. } => expr_contains_lambda(expr),
                })
        }
        Expr::Block(block, _) => block_contains_lambda(block),
        Expr::Tuple { items, .. } | Expr::Array { items, .. } => {
            items.iter().any(expr_contains_lambda)
        }
        Expr::MapInit { entries, .. } => entries.iter().any(|(_, expr)| expr_contains_lambda(expr)),
        Expr::NamedArg { value, .. } => expr_contains_lambda(value),
        Expr::Number(_, _)
        | Expr::String(_, _)
        | Expr::Bool(_, _)
        | Expr::Unit(_)
        | Expr::Ident(_, _)
        | Expr::SelfKw(_) => false,
    }
}

fn expr_contains_try(expr: &Expr) -> bool {
    use tint_ast::Expr;
    match expr {
        Expr::Try { .. } => true,
        Expr::Cast { expr, .. } => expr_contains_try(expr),
        Expr::Unary { expr, .. }
        | Expr::Paren(expr, _)
        | Expr::Borrow { target: expr, .. }
        | Expr::TupleIndex { target: expr, .. } => expr_contains_try(expr),
        Expr::Binary { left, right, .. } => expr_contains_try(left) || expr_contains_try(right),
        Expr::Field { target, .. }
        | Expr::Namespace { base: target, .. }
        | Expr::Index { target, .. } => expr_contains_try(target),
        Expr::Call { target, args, .. } => {
            expr_contains_try(target) || args.iter().any(expr_contains_try)
        }
        Expr::InterpolatedString { parts, .. } => parts
            .iter()
            .any(|p| matches!(p, tint_ast::StringPart::Expr(e) if expr_contains_try(e))),
        Expr::Match {
            scrutinee, arms, ..
        } => {
            expr_contains_try(scrutinee)
                || arms.iter().any(|a| {
                    expr_contains_try(&a.expr) || a.guard.as_ref().is_some_and(expr_contains_try)
                })
        }
        Expr::StructInit { fields, .. } | Expr::VariantInit { fields, .. } => {
            fields.iter().any(|f| match f {
                tint_ast::StructInitField::Assign { expr, .. }
                | tint_ast::StructInitField::Tint { expr, .. } => expr_contains_try(expr),
            })
        }
        Expr::StructUpdate { base, updates, .. } => {
            expr_contains_try(base)
                || updates.iter().any(|f| match f {
                    tint_ast::StructInitField::Assign { expr, .. }
                    | tint_ast::StructInitField::Tint { expr, .. } => expr_contains_try(expr),
                })
        }
        Expr::Block(block, _) => block.stmts.iter().any(stmt_contains_try),
        Expr::Tuple { items, .. } | Expr::Array { items, .. } => {
            items.iter().any(expr_contains_try)
        }
        Expr::MapInit { entries, .. } => entries.iter().any(|(_, e)| expr_contains_try(e)),
        Expr::NamedArg { value, .. } => expr_contains_try(value),
        Expr::If {
            cond, then, else_, ..
        } => {
            expr_contains_try(cond)
                || then.stmts.iter().any(stmt_contains_try)
                || else_.stmts.iter().any(stmt_contains_try)
        }
        Expr::Lambda { body, .. } => expr_contains_try(body),
        Expr::Number(_, _)
        | Expr::String(_, _)
        | Expr::Bool(_, _)
        | Expr::Unit(_)
        | Expr::Ident(_, _)
        | Expr::SelfKw(_) => false,
    }
}

fn expr_contains_fallible_method(expr: &Expr) -> bool {
    match expr {
        Expr::Call { target, args, .. } => {
            matches!(target.as_ref(), Expr::Field { field, .. } if matches!(field.as_str(), "expect" | "unwrap" | "is_some" | "is_none" | "is_ok" | "is_err" | "unwrap_or" | "map" | "and_then"))
                || expr_contains_fallible_method(target)
                || args.iter().any(expr_contains_fallible_method)
        }
        Expr::Unary { expr, .. }
        | Expr::Paren(expr, _)
        | Expr::Try { expr, .. }
        | Expr::Cast { expr, .. } => expr_contains_fallible_method(expr),
        Expr::Binary { left, right, .. } => {
            expr_contains_fallible_method(left) || expr_contains_fallible_method(right)
        }
        Expr::Field { target, .. }
        | Expr::Namespace { base: target, .. }
        | Expr::Index { target, .. } => expr_contains_fallible_method(target),
        Expr::InterpolatedString { parts, .. } => parts.iter().any(
            |p| matches!(p, tint_ast::StringPart::Expr(e) if expr_contains_fallible_method(e)),
        ),
        Expr::NamedArg { value, .. } => expr_contains_fallible_method(value),
        Expr::StructInit { fields, .. } | Expr::VariantInit { fields, .. } => {
            fields.iter().any(|f| match f {
                tint_ast::StructInitField::Assign { expr, .. }
                | tint_ast::StructInitField::Tint { expr, .. } => {
                    expr_contains_fallible_method(expr)
                }
            })
        }
        Expr::Number(_, _)
        | Expr::String(_, _)
        | Expr::Bool(_, _)
        | Expr::Unit(_)
        | Expr::Ident(_, _)
        | Expr::SelfKw(_)
        | Expr::Lambda { .. }
        | Expr::If { .. }
        | Expr::Match { .. }
        | Expr::Array { .. }
        | Expr::Tuple { .. }
        | Expr::TupleIndex { .. }
        | Expr::MapInit { .. }
        | Expr::StructUpdate { .. }
        | Expr::Block { .. }
        | Expr::Borrow { .. } => false,
    }
}

fn expr_contains_cast(expr: &Expr) -> bool {
    match expr {
        Expr::Cast { .. } => true,
        Expr::Unary { expr, .. } | Expr::Paren(expr, _) | Expr::Try { expr, .. } => {
            expr_contains_cast(expr)
        }
        Expr::Binary { left, right, .. } => expr_contains_cast(left) || expr_contains_cast(right),
        Expr::Call { target, args, .. } => {
            expr_contains_cast(target) || args.iter().any(expr_contains_cast)
        }
        Expr::Field { target, .. }
        | Expr::Namespace { base: target, .. }
        | Expr::Index { target, .. } => expr_contains_cast(target),
        Expr::NamedArg { value, .. } => expr_contains_cast(value),
        Expr::InterpolatedString { parts, .. } => parts
            .iter()
            .any(|p| matches!(p, tint_ast::StringPart::Expr(e) if expr_contains_cast(e))),
        _ => false,
    }
}

fn is_exact_numeric_type(ty: &tint_ast::Type) -> bool {
    matches!(ty, tint_ast::Type::Simple(name) if matches!(name.as_str(), "i32" | "i64" | "u8" | "u32" | "u64" | "f32" | "f64"))
}

fn pattern_contains_exact_numeric(pattern: &tint_ast::Pattern) -> bool {
    match pattern {
        tint_ast::Pattern::Typed { ty, .. } => is_exact_numeric_type(ty),
        tint_ast::Pattern::Mut { inner, .. } => pattern_contains_exact_numeric(inner),
        _ => false,
    }
}
