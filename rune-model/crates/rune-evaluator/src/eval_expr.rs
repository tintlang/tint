// rune-evaluator/eval_expr.rs

use crate::{value::Value, EvalHost};
use rune_ast::*;
use std::rc::Rc;

/// Evaluate an expression
pub fn eval_expr<H: EvalHost>(host: &mut H, expr: &Expr) -> Value {
    match expr {
        
        // Literals
        Expr::Number(n, _) => {
            Value::Number(n.parse::<f64>().unwrap())
        }

        Expr::String(s, _) => {
            Value::String(s.clone())
        }

        // Ident
        Expr::Ident(name, span) => {
            host.load_var(name, *span)
        }

        // Unary
        Expr::Unary { op, expr, .. } => {
            let v = eval_expr(host, expr);

            match (op.as_str(), v) {
                ("-", Value::Number(n)) => Value::Number(-n),
                ("!", Value::Bool(b)) => Value::Bool(!b),
                _ => Value::Unit,
            }
        }

        // Binary
        Expr::Binary { left, op, right, .. } => {
            let a = eval_expr(host, left);
            let b = eval_expr(host, right);

            match (a, op.as_str(), b) {
                (Value::Number(x), "+", Value::Number(y)) =>
                    Value::Number(x + y),

                (Value::Number(x), "-", Value::Number(y)) =>
                    Value::Number(x - y),

                (Value::Number(x), "*", Value::Number(y)) =>
                    Value::Number(x * y),

                (Value::Number(x), "/", Value::Number(y)) =>
                    Value::Number(x / y),

                (Value::Bool(x), "&&", Value::Bool(y)) =>
                    Value::Bool(x && y),

                (Value::Bool(x), "||", Value::Bool(y)) =>
                    Value::Bool(x || y),

                _ => Value::Unit,
            }
        }

        // Namespace:   module::item
        Expr::Namespace { base, item, .. } => {
            let m = eval_expr(host, base);
            host.namespace_lookup(m, item)
        }

        // Field:   obj.field
        Expr::Field { target, field, .. } => {
            let obj = eval_expr(host, target);
            host.field_lookup(obj, field)
        }

        // Index:   arr[i]
        Expr::Index { target, index, .. } => {
            let arr = eval_expr(host, target);
            let idx = eval_expr(host, index);
            host.index_lookup(arr, idx)
        }

        // Call:   func(args...)
        Expr::Call { target, args, span } => {
            let value = eval_expr(host, target);

            // evaluate args
            let eval_args: Vec<Value> = args
                .iter()
                .map(|a| eval_expr(host, a))
                .collect();

            host.call_value(value, &eval_args, *span)
        }

        // Lambda
        Expr::Lambda { params, body, .. } => {
            Value::Lambda {
                params: params.clone(),
                body: Box::new((**body).clone()),
                closure: host.capture_env(),
            }
        }
        // Struct init: Point { x:1, y:2 }
        Expr::StructInit { name, fields, .. } => {
            let vals = fields
                .iter()
                .map(|(k, v)| (k.clone(), eval_expr(host, v)))
                .collect();

            Value::StructInstance {
                name: name.clone(),
                fields: vals,
            }
        }

        // Paren
        Expr::Paren(inner, _) => eval_expr(host, inner),

        _ => Value::Unit,
    }
}
