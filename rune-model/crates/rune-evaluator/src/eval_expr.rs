// rune-evaluator/eval_expr.rs

use crate::{value::Value, EvalHost};
use crate::eval_host::Flow;
use rune_ast::*;
use crate::pattern_match::match_pattern;


pub fn eval_expr<H: EvalHost>(host: &mut H, expr: &Expr) -> Value {
    match expr {
        // NUMBER
        Expr::Number(n, _) =>
            Value::Number(n.parse::<f64>().unwrap()),

        // STRING
        Expr::String(s, _) =>
            Value::String(s.clone()),

        // BOOL
        Expr::Bool(v, _) =>
            Value::Bool(*v),

        // UNIT
        Expr::Unit(_) =>
            Value::Unit,

        // IDENT
        Expr::Ident(name, span) =>
            host.load_var(name, *span),

        // INTERPOLATED STRING
        Expr::InterpolatedString { parts, .. } => {
            let mut out = String::new();
            for part in parts {
                match part {
                    StringPart::Text(txt) => out.push_str(txt),
                    StringPart::Expr(inner) => {
                        let v = eval_expr(host, inner);
                        out.push_str(&v.to_string());
                    }
                }
            }
            Value::String(out)
        }

        // UNARY
        Expr::Unary { op, expr, .. } => {
            let v = eval_expr(host, expr);
            match (op.as_str(), v) {
                ("-", Value::Number(n)) => Value::Number(-n),
                ("!", Value::Bool(b)) => Value::Bool(!b),
                _ => Value::Unit,
            }
        }

        // BINARY
        Expr::Binary { left, op, right, .. } => {
            let a = eval_expr(host, left);
            let b = eval_expr(host, right);

            match (a, op.as_str(), b) {
                (Value::Number(x), "+", Value::Number(y)) => Value::Number(x + y),
                (Value::Number(x), "-", Value::Number(y)) => Value::Number(x - y),
                (Value::Number(x), "*", Value::Number(y)) => Value::Number(x * y),
                (Value::Number(x), "/", Value::Number(y)) => Value::Number(x / y),

                (Value::Bool(x), "&&", Value::Bool(y)) => Value::Bool(x && y),
                (Value::Bool(x), "||", Value::Bool(y)) => Value::Bool(x || y),

                _ => Value::Unit,
            }
        }

        // ARRAY → LIST
        Expr::Array { items, .. } => {
            let vals = items.iter().map(|e| eval_expr(host, e)).collect();
            Value::List(vals)
        }

        // MATCH EXPRESSION
        Expr::Match { scrutinee, arms, .. } => {
            let val = eval_expr(host, scrutinee);

            for (pat, body) in arms {
                if match_pattern(pat, &val) {
                    return eval_expr(host, body);
                }
            }

            Value::Unit
        }

        // FIELD: obj.field
        Expr::Field { target, field, .. } => {
            let obj = eval_expr(host, target);
            host.field_lookup(obj, field)
        }

        // module::item
        Expr::Namespace { base, item, .. } => {
            let v = eval_expr(host, base);
            host.namespace_lookup(v, item)
        }

        // arr[i]
        Expr::Index { target, index, .. } => {
            let arr = eval_expr(host, target);
            let idx = eval_expr(host, index);
            host.index_lookup(arr, idx)
        }

        // CALL
        Expr::Call { target, args, span } => {
            let func_val = eval_expr(host, target);
            let arg_vals = args.iter().map(|a| eval_expr(host, a)).collect::<Vec<_>>();
            host.call_value(func_val, &arg_vals, *span)
        }

        // LAMBDA
        Expr::Lambda { params, body, .. } => {
            Value::Lambda {
                params: params.clone(),
                body: Box::new((**body).clone()),
                closure: host.capture_env(),
            }
        }

        // STRUCT INIT
        Expr::StructInit { name, fields, .. } => {
            let mut out = Vec::new();
            for fld in fields {
                match fld {
                    StructInitField::Assign { name, expr, .. } =>
                        out.push((name.clone(), eval_expr(host, expr))),
                    StructInitField::Rune { name, expr, .. } =>
                        out.push((name.clone(), eval_expr(host, expr))),
                }
            }

            Value::StructInstance {
                name: name.clone(),
                fields: out,
            }
        }

        // STRUCT UPDATE — new
        Expr::StructUpdate { base, updates, .. } => {
            // 1. Evaluate the base struct
            let v = eval_expr(host, base);

            let Value::StructInstance { name, fields } = v else {
                panic!("Struct update on non-struct");
            };

            // 2. Copy fields
            let mut new_fields = fields;

            // 3. Apply updates
            for upd in updates {
                match upd {
                    StructInitField::Assign { name: fname, expr, .. } => {
                        let val = eval_expr(host, expr);
                        // replace field
                        if let Some(slot) = new_fields.iter_mut().find(|(k,_)| k == fname) {
                            slot.1 = val;
                        } else {
                            new_fields.push((fname.clone(), val));
                        }
                    }

                    StructInitField::Rune { name: fname, expr, .. } => {
                        let val = eval_expr(host, expr);
                        if let Some(slot) = new_fields.iter_mut().find(|(k,_)| k == fname) {
                            slot.1 = val;
                        } else {
                            new_fields.push((fname.clone(), val));
                        }
                    }
                }
            }

            Value::StructInstance {
                name,
                fields: new_fields,
            }
        }

        // NAMED ARG (illegal outside call)
        Expr::NamedArg { name, .. } => {
            panic!("Named argument `{}` cannot appear as standalone expression", name)
        }

        // BLOCK { … }
        Expr::Block(block, _) => {
            match host.eval_block(block) {
                Flow::Value(v) => v,
                Flow::Return(v) => v,
                _ => Value::Unit,
            }
        }

        // TUPLE EXPRESSION
        Expr::Tuple { items, .. } => {
            let vals = items.iter().map(|e| eval_expr(host, e)).collect();
            Value::Tuple(vals)
        }

        // PAREN
        Expr::Paren(inner, ..) =>
            eval_expr(host, inner),
    }
}
