// rune-evaluator/stmt.rs

use crate::{value::Value, EvalHost};
use crate::eval_host::Flow;    
use rune_ast::*;

/// Evaluate a single statement
pub fn eval_stmt<H: EvalHost>(host: &mut H, stmt: &Stmt) -> Flow {
    match stmt {

        Stmt::Let { name, expr, .. } => {
            let val = host.eval_expr(expr);
            host.define_var(name, val.clone());
            Flow::Value(Value::Unit)
        }

        Stmt::Assign { name, expr, .. } => {
            let val = host.eval_expr(expr);
            host.set_var(name, val.clone());
            Flow::Value(val)
        }

        Stmt::CompoundAssign { name, op, expr, .. } => {
            let old = host.load_var(name, expr.span());
            let rhs = host.eval_expr(expr);

            let new = match (old, op.as_str(), rhs) {
                (Value::Number(a), "+=", Value::Number(b)) => Value::Number(a + b),
                (Value::Number(a), "-=", Value::Number(b)) => Value::Number(a - b),
                (Value::Number(a), "*=", Value::Number(b)) => Value::Number(a * b),
                (Value::Number(a), "/=", Value::Number(b)) => Value::Number(a / b),
                _ => Value::Unit,
            };

            host.set_var(name, new.clone());
            Flow::Value(new)
        }

        Stmt::Return(expr, _) => {
            let v = host.eval_expr(expr);
            Flow::Return(v)
        }

        Stmt::Break(_) => Flow::Break,
        Stmt::Continue(_) => Flow::Continue,

        Stmt::If { cond, then, else_, .. } => {
            let c = host.eval_expr(cond).as_bool().unwrap_or(false);

            if c {
                // then-block → может вернуть Flow::Return/Break/Continue
                return host.eval_block_flow(then);
            } else if let Some(e) = else_ {
                return host.eval_block_flow(e);
            }

            Flow::Value(Value::Unit)
            }

            Stmt::While { cond, body, .. } => {
                loop {
                    let c = host.eval_expr(cond).as_bool().unwrap_or(false);
                    if !c { break; }

                    host.push_scope();
                    let flow = host.eval_block_flow(body);
                    host.pop_scope();

                    match flow {
                        Flow::Value(_) => {}
                        Flow::Continue => continue,
                        Flow::Break => break,
                        Flow::Return(v) => return Flow::Return(v),
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
                    Flow::Value(_) => {}
                }
            }
            Flow::Value(Value::Unit)
        }

        Stmt::For { var, start, end, body, .. } => {
            let s = host.eval_expr(start).as_number().unwrap_or(0.0) as i64;
            let e = host.eval_expr(end).as_number().unwrap_or(0.0) as i64;

            for i in s..e {
                host.push_scope();
                host.define_var(var, Value::Number(i as f64));

                let flow = host.eval_block_flow(body);
                host.pop_scope();

                match flow {
                    Flow::Break => break,
                    Flow::Continue => continue,
                    Flow::Return(v) => return Flow::Return(v),
                    Flow::Value(_) => {}
                }
            }

            Flow::Value(Value::Unit)
        }

        Stmt::Match { expr, arms, .. } => {
            let v = host.eval_expr(expr);

            for arm in arms {
                if host.match_pattern(&v, &arm.pattern) {
                    return Flow::Value(host.eval_expr(&arm.expr));
                }
            }

            Flow::Value(Value::Unit)
        }

        Stmt::Expr(expr) => Flow::Value(host.eval_expr(expr)),
    }
}
