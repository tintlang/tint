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
        Expr::Ident(name, span) => host
            .lookup_var(name)
            .or_else(|| host.resolve_function(name))
            .unwrap_or_else(|| host.load_var(name, *span)),

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

                (left, "+", right) => match (vec2_components(&left), vec2_components(&right)) {
                    (Some((x1, y1)), Some((x2, y2))) => vec2_value(x1 + x2, y1 + y2),
                    _ => Value::Unit,
                },
                (left, "-", right) => match (vec2_components(&left), vec2_components(&right)) {
                    (Some((x1, y1)), Some((x2, y2))) => vec2_value(x1 - x2, y1 - y2),
                    _ => Value::Unit,
                },
                (left @ Value::StructInstance { .. }, "*", Value::Number(scalar)) => {
                    match vec2_components(&left) {
                        Some((x, y)) => vec2_value(x * scalar, y * scalar),
                        None => Value::Unit,
                    }
                }
                (Value::Number(scalar), "*", right @ Value::StructInstance { .. }) => {
                    match vec2_components(&right) {
                        Some((x, y)) => vec2_value(x * scalar, y * scalar),
                        None => Value::Unit,
                    }
                }

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
                    // Pattern variables must be visible while evaluating the
                    // guard and the arm expression. Keep them in a temporary
                    // scope so a failed guarded arm cannot leak bindings
                    // into the following arms or the surrounding function.
                    host.push_scope();
                    if !host.bind_pattern(&arm.pattern, &val) {
                        host.pop_scope();
                        continue;
                    }

                    if let Some(ref guard_expr) = arm.guard {
                        let guard_val = eval_expr(host, guard_expr);

                        match guard_val {
                            Value::Bool(true) => {
                                let result = eval_expr(host, &arm.expr);
                                host.pop_scope();
                                return result;
                            }
                            Value::Bool(false) => {
                                host.pop_scope();
                                continue;
                            }
                            _ => {
                                host.pop_scope();
                                panic!("Match guard must evaluate to bool");
                            }
                        }
                    }

                    let result = eval_expr(host, &arm.expr);
                    host.pop_scope();
                    return result;
                }
            }

            Value::Unit
        }
        Expr::If {
            cond, then, else_, ..
        } => {
            let block = if eval_expr(host, cond).force_bool() {
                then
            } else {
                else_
            };
            match host.eval_block_flow(block) {
                Flow::Value(v) | Flow::Return(v) => v,
                Flow::Break | Flow::Continue => Value::Unit,
            }
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

            // A field-shaped call (`point.move_by(...)`) is a method call
            // candidate. The host returns the updated receiver so a mutable
            // method can write it back into `point` or a UI state binding.
            if let Expr::Field {
                target: receiver_expr,
                field: method,
                ..
            } = target.as_ref()
            {
                let receiver = eval_expr(host, receiver_expr);
                if let Ok((updated_receiver, result)) =
                    host.call_method(receiver, method, &arg_vals, *span)
                {
                    let _ = host.assign_to(receiver_expr, updated_receiver);
                    return result;
                }
            }

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

fn vec2_components(value: &Value) -> Option<(f64, f64)> {
    let Value::StructInstance { name, fields } = value else {
        return None;
    };
    if name != "Vec2" {
        return None;
    }

    let number = |field: &str| {
        fields
            .iter()
            .find(|(name, _)| name == field)
            .and_then(|(_, value)| value.as_number())
    };
    Some((number("x")?, number("y")?))
}

fn vec2_value(x: f64, y: f64) -> Value {
    Value::StructInstance {
        name: "Vec2".into(),
        fields: vec![
            ("x".into(), Value::Number(x)),
            ("y".into(), Value::Number(y)),
        ],
    }
}
