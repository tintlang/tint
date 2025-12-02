// rune-evaluator/eval_expr.rs

use crate::{value::Value, EvalHost};
use rune_ast::*;

pub fn eval_expr<H: EvalHost>(host: &mut H, expr: &Expr) -> Value {
    match expr {
        Expr::Number(n, _) => Value::Number(n.parse::<f64>().unwrap()),

        Expr::String(s, _) => Value::String(s.clone()),

        Expr::Ident(name, span) => {
            host.load_var(name, *span)
        }

        Expr::Binary { left, op, right, .. } => {
            let l = eval_expr(host, left);
            let r = eval_expr(host, right);

            match (l, op.as_str(), r) {
                (Value::Number(a), "+", Value::Number(b)) =>
                    Value::Number(a + b),

                (Value::Number(a), "-", Value::Number(b)) =>
                    Value::Number(a - b),

                _ => Value::Unit,
            }
        }

        Expr::Call { name, args, span } => {
            let evaluated: Vec<Value> =
                args.iter().map(|e| eval_expr(host, e)).collect();

            host.call_fn(name, &evaluated, *span)
        }

        _ => Value::Unit,
    }
}
