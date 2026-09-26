use tint_ast::*;

use crate::{eval_host::Flow, pattern_match::match_pattern, value::Value, EvalHost};

pub fn eval_expr<H: EvalHost>(host: &mut H, expr: &Expr) -> Value {
    match expr {
        Expr::Number(n, _) => Value::Number(n.parse::<f64>().unwrap()),
        Expr::String(s, _) => Value::String(s.clone()),
        Expr::Bool(v, _) => Value::Bool(*v),
        Expr::Unit(_) => Value::Unit,

        Expr::Borrow { target, .. } => {
            // Borrowing is not modeled yet; evaluate the target directly.
            eval_expr(host, target)
        }
        Expr::SelfKw(_) => host.load_var("self", expr.span()),
        Expr::Ident(name, span) => host.load_var(name, *span),

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
        Expr::Unary { op, expr, .. } => {
            let v = eval_expr(host, expr);
            match (op.as_str(), v) {
                ("-", Value::Number(n)) => Value::Number(-n),
                ("!", Value::Bool(b)) => Value::Bool(!b),
                _ => Value::Unit,
            }
        }
        Expr::Binary {
            left, op, right, ..
        } => {
            let a = eval_expr(host, left);
            let b = eval_expr(host, right);

            match (a, op.as_str(), b) {
                (Value::Number(x), "+", Value::Number(y)) => Value::Number(x + y),
                (Value::Number(x), "-", Value::Number(y)) => Value::Number(x - y),
                (Value::Number(x), "*", Value::Number(y)) => Value::Number(x * y),
                (Value::Number(x), "/", Value::Number(y)) => Value::Number(x / y),

                (Value::Bool(x), "&&", Value::Bool(y)) => Value::Bool(x && y),
                (Value::Bool(x), "||", Value::Bool(y)) => Value::Bool(x || y),

                // Comparisons -- previously entirely unhandled (fell through
                // to the `_ => Unit` catch-all below, so e.g. `if{width <
                // 768}` silently evaluated to Unit/falsy no matter what
                // `width` was). `<`/`>`/`<=`/`>=` are numeric-only, matching
                // the four arithmetic ops right above; `==`/`!=` also cover
                // Bool and String since equality checks on those are common
                // and cheap to support (`status == "ready"`, `menu_open ==
                // false`), unlike a full structural-equality impl for every
                // Value variant, which nothing here has asked for.
                (Value::Number(x), "<", Value::Number(y)) => Value::Bool(x < y),
                (Value::Number(x), ">", Value::Number(y)) => Value::Bool(x > y),
                (Value::Number(x), "<=", Value::Number(y)) => Value::Bool(x <= y),
                (Value::Number(x), ">=", Value::Number(y)) => Value::Bool(x >= y),

                (Value::Number(x), "==", Value::Number(y)) => Value::Bool(x == y),
                (Value::Number(x), "!=", Value::Number(y)) => Value::Bool(x != y),
                (Value::Bool(x), "==", Value::Bool(y)) => Value::Bool(x == y),
                (Value::Bool(x), "!=", Value::Bool(y)) => Value::Bool(x != y),
                (Value::String(x), "==", Value::String(y)) => Value::Bool(x == y),
                (Value::String(x), "!=", Value::String(y)) => Value::Bool(x != y),

                _ => Value::Unit,
            }
        }
        Expr::Array { items, .. } => {
            let vals = items.iter().map(|e| eval_expr(host, e)).collect();
            Value::List(vals)
        }
        Expr::Match {
            scrutinee, arms, ..
        } => {
            let val = eval_expr(host, scrutinee);

            for arm in arms {
                if match_pattern(&arm.pattern, &val) {
                    if let Some(ref guard_expr) = arm.guard {
                        let guard_val = eval_expr(host, guard_expr);

                        match guard_val {
                            Value::Bool(true) => {
                                return eval_expr(host, &arm.expr);
                            }
                            Value::Bool(false) => {
                                continue;
                            }
                            _ => panic!("Match guard must evaluate to bool"),
                        }
                    }
                    return eval_expr(host, &arm.expr);
                }
            }

            Value::Unit
        }
        Expr::Field { target, field, .. } => {
            let obj = eval_expr(host, target);
            host.field_lookup(obj, field)
        }
        Expr::Namespace { base, item, .. } => {
            let v = eval_expr(host, base);
            host.namespace_lookup(v, item)
        }
        Expr::Index { target, index, .. } => {
            let arr = eval_expr(host, target);
            let idx = eval_expr(host, index);
            host.index_lookup(arr, idx)
        }
        Expr::Call { target, args, span } => {
            let arg_vals = args.iter().map(|a| eval_expr(host, a)).collect::<Vec<_>>();

            // A bare identifier callee (`foo(...)`) is a *named* call --
            // try it as one first (native fns, builtins, UI fns, logic
            // fns all live outside ordinary variable scope, so
            // `load_var`+`call_value` alone can never reach them: that
            // path only ever finds a value if the same name also happens
            // to be bound as a variable). Anything else (calling a lambda
            // stored in a variable/field, an IIFE, ...) still goes
            // through the original evaluate-then-call_value path.
            if let Expr::Ident(name, _) = target.as_ref() {
                if let Ok(v) = host.call_fn(name, &arg_vals, *span) {
                    return v;
                }
            }

            let func_val = eval_expr(host, target);
            host.call_value(func_val, &arg_vals, *span)
        }
        Expr::Lambda { params, body, .. } => Value::Lambda {
            params: params.clone(),
            body: Box::new((**body).clone()),
            closure: host.capture_env(),
        },
        Expr::StructInit { name, fields, .. } => {
            let mut out = Vec::new();
            for fld in fields {
                match fld {
                    StructInitField::Assign { name, expr, .. } => {
                        out.push((name.clone(), eval_expr(host, expr)))
                    }
                    StructInitField::Tint { name, expr, .. } => {
                        out.push((name.clone(), eval_expr(host, expr)))
                    }
                }
            }

            Value::StructInstance {
                name: name.clone(),
                fields: out,
            }
        }
        Expr::StructUpdate { base, updates, .. } => {
            let v = eval_expr(host, base);

            let Value::StructInstance { name, fields } = v else {
                panic!("Struct update on non-struct");
            };
            let mut new_fields = fields;
            for upd in updates {
                match upd {
                    StructInitField::Assign {
                        name: fname, expr, ..
                    } => {
                        let val = eval_expr(host, expr);
                        if let Some(slot) = new_fields.iter_mut().find(|(k, _)| k == fname) {
                            slot.1 = val;
                        } else {
                            new_fields.push((fname.clone(), val));
                        }
                    }

                    StructInitField::Tint {
                        name: fname, expr, ..
                    } => {
                        let val = eval_expr(host, expr);
                        if let Some(slot) = new_fields.iter_mut().find(|(k, _)| k == fname) {
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
        Expr::NamedArg { name, .. } => {
            panic!(
                "Named argument `{}` cannot appear as standalone expression",
                name
            )
        }
        Expr::Block(block, _) => match host.eval_block(block) {
            Flow::Value(v) => v,
            Flow::Return(v) => v,
            _ => Value::Unit,
        },
        Expr::Tuple { items, .. } => {
            let vals = items.iter().map(|e| eval_expr(host, e)).collect();
            Value::Tuple(vals)
        }
        Expr::TupleIndex { target, index, .. } => {
            let val = eval_expr(host, target);

            match val {
                Value::Tuple(items) => items
                    .get(*index)
                    .cloned()
                    .unwrap_or_else(|| panic!("Tuple index {} out of bounds", index)),
                other => panic!("TupleIndex used on non-tuple value: {:?}", other),
            }
        }
        Expr::VariantInit {
            enum_name,
            variant,
            fields,
            ..
        } => {
            let mut out_fields = Vec::new();

            for f in fields {
                match f {
                    StructInitField::Assign { name, expr, .. } => {
                        let v = eval_expr(host, expr);
                        out_fields.push((name.clone(), v));
                    }
                    StructInitField::Tint { name, expr, .. } => {
                        let v = eval_expr(host, expr);
                        out_fields.push((name.clone(), v));
                    }
                }
            }

            Value::EnumInstance {
                enum_name: enum_name.clone(),
                variant: variant.clone(),
                args: out_fields.into_iter().map(|(_, v)| v).collect(),
            }
        }

        Expr::MapInit { entries, .. } => {
            let mut m = std::collections::HashMap::new();

            for (k, e) in entries {
                let v = eval_expr(host, e);
                m.insert(k.clone(), v);
            }

            Value::Map(m)
        }
        Expr::Paren(inner, ..) => eval_expr(host, inner),
    }
}
