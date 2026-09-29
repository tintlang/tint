use crate::eval_host::Flow;
use crate::{value::Value, EvalHost};
use tint_ast::*;

/// Evaluate a single statement
pub fn eval_stmt<H: EvalHost>(host: &mut H, stmt: &Stmt) -> Flow {
    match stmt {
        Stmt::Let {
            pattern, ty, init, ..
        } => {
            let value = match init {
                LetInit::Assign(expr) => host.eval_expr(expr),
                LetInit::Tint(expr) => host.eval_expr(expr),
            };
            let value = if let Some(tint_ast::Type::Simple(name)) = ty.as_ref() {
                if matches!(name.as_str(), "u8" | "f32" | "f64") {
                    value
                        .cast_numeric(ty.as_ref().expect("typed binding has a type"))
                        .unwrap_or_else(|message| panic!("numeric type error: {message}"))
                } else {
                    value
                }
            } else {
                value
            };
            if let Value::Propagate(value) = value {
                return Flow::Propagate(*value);
            }
            // Type validation is reserved for the typed evaluator pass.
            if let Some(_expected_ty) = ty {
                // TODO
            }
            if !host.bind_pattern(pattern, &value) {
                panic!("Pattern match failed in let-binding");
            }

            Flow::Value(Value::Unit)
        }

        // Assignment supports any target accepted by EvalHost::assign_to.
        Stmt::Assign { lhs, rhs, .. } => {
            let r = host.eval_expr(rhs);
            if let Value::Propagate(value) = r {
                return Flow::Propagate(*value);
            }
            if !host.assign_to(lhs, r.clone()) {
                panic!("Invalid assignment target");
            }
            Flow::Value(r)
        }

        Stmt::CompoundAssign { name, op, expr, .. } => {
            let left = host.load_var(name, expr.span());
            let right = host.eval_expr(expr);
            if let Value::Propagate(value) = right {
                return Flow::Propagate(*value);
            }

            let new = host.apply_compound(&left, op.as_str(), &right);
            host.set_var(name, new.clone());

            Flow::Value(new)
        }

        Stmt::Return(expr, _) => {
            let v = host.eval_expr(expr);
            if let Value::Propagate(value) = v {
                return Flow::Propagate(*value);
            }
            Flow::Return(v)
        }

        Stmt::Break(_) => Flow::Break,
        Stmt::Continue(_) => Flow::Continue,

        Stmt::If {
            cond, then, else_, ..
        } => {
            let c = host.eval_expr(cond).force_bool();

            if c {
                host.eval_block_flow(then)
            } else if let Some(e) = else_ {
                host.eval_block_flow(e)
            } else {
                Flow::Value(Value::Unit)
            }
        }

        Stmt::While { cond, body, .. } => {
            loop {
                if !host.eval_expr(cond).force_bool() {
                    break;
                }

                host.push_scope();
                let flow = host.eval_block_flow(body);
                host.pop_scope();

                match flow {
                    Flow::Continue => continue,
                    Flow::Break => break,
                    Flow::Return(v) => return Flow::Return(v),
                    Flow::Propagate(v) => return Flow::Propagate(v),
                    Flow::Value(_) => {}
                }
            }

            Flow::Value(Value::Unit)
        }

        Stmt::Loop { body, .. } => {
            loop {
                host.push_scope();
                let flow = host.eval_block_flow(body);
                host.pop_scope();

                match flow {
                    Flow::Continue => continue,
                    Flow::Break => break,
                    Flow::Return(v) => return Flow::Return(v),
                    Flow::Propagate(v) => return Flow::Propagate(v),
                    Flow::Value(_) => {}
                }
            }

            Flow::Value(Value::Unit)
        }

        Stmt::For {
            var,
            start,
            end,
            body,
            ..
        } => {
            let s = host.eval_expr(start).as_int();
            let e = host.eval_expr(end).as_int();

            for i in s..e {
                host.push_scope();
                host.define_var(var, Value::Number(i as f64));

                let flow = host.eval_block_flow(body);
                host.pop_scope();

                match flow {
                    Flow::Continue => continue,
                    Flow::Break => break,
                    Flow::Return(v) => return Flow::Return(v),
                    Flow::Propagate(v) => return Flow::Propagate(v),
                    Flow::Value(_) => {}
                }
            }

            Flow::Value(Value::Unit)
        }

        Stmt::ForIn { var, iter, body, .. } => {
            let items = match host.eval_expr(iter) {
                Value::List(items) => items,
                other => return Flow::Value(other),
            };

            for item in items {
                host.push_scope();
                host.define_var(var, item);

                let flow = host.eval_block_flow(body);
                host.pop_scope();

                match flow {
                    Flow::Continue => continue,
                    Flow::Break => break,
                    Flow::Return(v) => return Flow::Return(v),
                    Flow::Propagate(v) => return Flow::Propagate(v),
                    Flow::Value(_) => {}
                }
            }

            Flow::Value(Value::Unit)
        }

        Stmt::Match { expr, arms, .. } => {
            let v = host.eval_expr(expr);

            for arm in arms {
                if host.match_pattern(&v, &arm.pattern) {
                    host.push_scope();
                    if !host.bind_pattern(&arm.pattern, &v) {
                        host.pop_scope();
                        continue;
                    }
                    if let Some(guard) = &arm.guard {
                        if !host.eval_expr(guard).force_bool() {
                            host.pop_scope();
                            continue;
                        }
                    }
                    let result = host.eval_expr(&arm.expr);
                    host.pop_scope();
                    if let Value::Propagate(value) = result {
                        return Flow::Propagate(*value);
                    }
                    return Flow::Value(result);
                }
            }

            Flow::Value(Value::Unit)
        }

        Stmt::Expr(expr) => match host.eval_expr(expr) {
            Value::Propagate(value) => Flow::Propagate(*value),
            value => Flow::Value(value),
        },
    }
}
