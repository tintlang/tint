//! Optional SSA/IR execution tier (cargo feature `ir`, off by default).
//! The tree-walking evaluator is the single execution engine; everything
//! here only decides whether a plain logic function may be handed to the IR VM.

use super::*;

impl TintVM {
    /// `Some(result)` if the IR VM ran `name`; `None` means "use the evaluator".
    pub(super) fn try_call_via_ir(
        &mut self,
        name: &str,
        args: &[EvalValue],
        span: Span,
    ) -> Option<EvalResult<EvalValue>> {
        // A helper called from a UI event/frame handler must stay on the
        // tree-walking VM: that is the VM which owns the persistent `state`
        // scope.  The IR VM is intentionally isolated and is still used for
        // ordinary top-level logic calls.
        if self.treewalk_call_depth > 0 && self.is_ir_function(name) {
            return None;
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
            return None;
        }

        if !self.is_ir_function(name) {
            return None;
        }

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
        return Some(Ok(Self::ir_value_to_eval_value(out)));
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
