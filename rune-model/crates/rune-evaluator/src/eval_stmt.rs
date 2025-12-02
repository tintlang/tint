// rune-evaluator/stmt.rs

use crate::{value::Value, EvalHost};
use rune_ast::*;

/// Evaluate a single statement.
pub fn eval_stmt<H: EvalHost>(host: &mut H, stmt: &Stmt) -> Value {
    match stmt {
        // ------------------------------------------------------
        // let x = expr
        // ------------------------------------------------------
        Stmt::Let { name, expr, .. } => {
            let val = host.eval_expr(expr);
            host.define_var(name, val.clone());
            val
        }

        // ------------------------------------------------------
        // x = expr
        // ------------------------------------------------------
        Stmt::Assign { name, expr, .. } => {
            let val = host.eval_expr(expr);
            host.define_var(name, val.clone());
            val
        }

        // ------------------------------------------------------
        // if(cond) { then } else { else }
        // ------------------------------------------------------
        Stmt::If { cond, then, else_, .. } => {
            let c = host
                .eval_expr(cond)
                .as_bool()
                .unwrap_or(false);

            if c {
                host.eval_block(then)
            } else if let Some(e) = else_ {
                host.eval_block(e)
            } else {
                Value::Unit
            }
        }

        // ------------------------------------------------------
        // for i in start..end { body }
        // ------------------------------------------------------
        Stmt::For { var, start, end, body, .. } => {
            let s = host.eval_expr(start).as_number().unwrap_or(0.0) as i64;
            let e = host.eval_expr(end).as_number().unwrap_or(0.0) as i64;

            host.push_scope();
            for i in s..e {
                host.define_var(var, Value::Number(i as f64));
                host.eval_block(body);
            }
            host.pop_scope();

            Value::Unit
        }

        // ------------------------------------------------------
        // bare expression
        // ------------------------------------------------------
        Stmt::Expr(expr) => host.eval_expr(expr),

        // ------------------------------------------------------
        // unimplemented: match, struct destructuring etc.
        // ------------------------------------------------------
        _ => Value::Unit,
    }
}
