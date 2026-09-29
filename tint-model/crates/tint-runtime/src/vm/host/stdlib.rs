//! Methods on the built-in collection values: `List`, `String` and `Map`.
//!
//! Dispatched from `host_call_method` before user `impl` lookup. Every method
//! returns `(updated_receiver, result)`, the same shape the rest of method
//! dispatch uses, so mutating methods (`push`, `pop`, `remove`, `set`) write
//! back into the variable they were called on.
//!
//! Notes on semantics:
//! - Lengths and indices are `number` (f64), like every untyped numeric value.
//! - `String::len` counts characters, not bytes.
//! - `Map` is keyed by string; a non-string key is converted with its display
//!   text. `keys()`/`values()` come back sorted by key so output is stable.

use super::super::*;
use tint_evaluator::errors::EvalError;

type MethodResult = EvalResult<(EvalValue, EvalValue)>;

fn fail<T>(span: Span, msg: impl Into<String>) -> EvalResult<T> {
    Err(EvalError::InvalidOp {
        msg: msg.into(),
        span,
    })
}

fn expect_args(ty: &str, method: &str, args: &[EvalValue], n: usize, span: Span) -> EvalResult<()> {
    if args.len() == n {
        Ok(())
    } else {
        fail(
            span,
            format!("{ty}.{method} expects {n} argument(s), got {}", args.len()),
        )
    }
}

fn str_arg<'a>(ty: &str, method: &str, args: &'a [EvalValue], span: Span) -> EvalResult<&'a str> {
    match args.first() {
        Some(EvalValue::String(s)) => Ok(s),
        _ => fail(span, format!("{ty}.{method} expects a string argument")),
    }
}

fn some(value: EvalValue) -> EvalValue {
    EvalValue::EnumInstance {
        enum_name: "Option".into(),
        variant: "Some".into(),
        args: vec![value],
    }
}

fn none() -> EvalValue {
    EvalValue::EnumInstance {
        enum_name: "Option".into(),
        variant: "None".into(),
        args: Vec::new(),
    }
}

fn list_of_strings(parts: impl Iterator<Item = String>) -> EvalValue {
    EvalValue::List(parts.map(EvalValue::String).collect())
}

/// `[start, end)` from `slice(start)` / `slice(start, end)`, clamped to `len`.
fn slice_bounds(
    ty: &str,
    len: usize,
    args: &[EvalValue],
    span: Span,
) -> EvalResult<(usize, usize)> {
    if args.is_empty() || args.len() > 2 {
        return fail(span, format!("{ty}.slice expects 1 or 2 arguments"));
    }
    let mut bounds = [0, len];
    for (slot, arg) in bounds.iter_mut().zip(args) {
        match arg.as_number() {
            Some(n) if n >= 0.0 && n.fract() == 0.0 => *slot = (n as usize).min(len),
            _ => {
                return fail(
                    span,
                    format!("{ty}.slice expects non-negative integer indexes"),
                )
            }
        }
    }
    Ok((bounds[0], bounds[1].max(bounds[0])))
}

fn sorted(items: &[EvalValue], span: Span) -> EvalResult<Vec<EvalValue>> {
    let mut out = items.to_vec();
    if out.iter().all(|v| matches!(v, EvalValue::String(_))) {
        out.sort_by(|a, b| a.to_string().cmp(&b.to_string()));
    } else if out.iter().all(|v| v.as_number().is_some()) {
        out.sort_by(|a, b| {
            a.as_number()
                .unwrap_or_default()
                .total_cmp(&b.as_number().unwrap_or_default())
        });
    } else {
        return fail(span, "List.sort expects only numbers or only strings");
    }
    Ok(out)
}

fn values_equal(a: &EvalValue, b: &EvalValue) -> bool {
    use EvalValue::*;
    match (a, b) {
        (String(a), String(b)) => a == b,
        (Bool(a), Bool(b)) => a == b,
        (Unit, Unit) => true,
        (List(a), List(b)) | (Tuple(a), Tuple(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(x, y)| values_equal(x, y))
        }
        _ => match (a.as_number(), b.as_number()) {
            (Some(x), Some(y)) => x == y,
            _ => false,
        },
    }
}

impl TintVM {
    /// `None` when `receiver` is not a `List`, `String` or `Map`.
    pub(super) fn collection_method(
        &mut self,
        receiver: &EvalValue,
        method: &str,
        args: &[EvalValue],
        span: Span,
    ) -> Option<MethodResult> {
        match receiver {
            EvalValue::List(items) => Some(self.list_method(items, method, args, span)),
            EvalValue::String(s) => Some(string_method(s, method, args, span)),
            EvalValue::Map(map) => Some(map_method(map, method, args, span)),
            _ => None,
        }
    }

    fn list_method(
        &mut self,
        items: &[EvalValue],
        method: &str,
        args: &[EvalValue],
        span: Span,
    ) -> MethodResult {
        let same = || EvalValue::List(items.to_vec());
        match method {
            "len" => {
                expect_args("List", method, args, 0, span)?;
                Ok((same(), EvalValue::Number(items.len() as f64)))
            }
            "is_empty" => {
                expect_args("List", method, args, 0, span)?;
                Ok((same(), EvalValue::Bool(items.is_empty())))
            }
            "contains" => {
                expect_args("List", method, args, 1, span)?;
                let found = items.iter().any(|item| values_equal(item, &args[0]));
                Ok((same(), EvalValue::Bool(found)))
            }
            "join" => {
                expect_args("List", method, args, 1, span)?;
                let sep = str_arg("List", method, args, span)?;
                let joined = items
                    .iter()
                    .map(|item| item.to_string())
                    .collect::<Vec<_>>()
                    .join(sep);
                Ok((same(), EvalValue::String(joined)))
            }
            "push" => {
                expect_args("List", method, args, 1, span)?;
                let mut updated = items.to_vec();
                updated.push(args[0].clone());
                Ok((EvalValue::List(updated), EvalValue::Unit))
            }
            "pop" => {
                expect_args("List", method, args, 0, span)?;
                let mut updated = items.to_vec();
                let popped = updated.pop().map(some).unwrap_or_else(none);
                Ok((EvalValue::List(updated), popped))
            }
            "remove" => {
                expect_args("List", method, args, 1, span)?;
                let index = match args[0].as_number() {
                    Some(n) if n >= 0.0 && n.fract() == 0.0 => n as usize,
                    _ => return fail(span, "List.remove expects a non-negative integer index"),
                };
                if index >= items.len() {
                    return fail(
                        span,
                        format!(
                            "List.remove index {index} out of bounds (len={})",
                            items.len()
                        ),
                    );
                }
                let mut updated = items.to_vec();
                let removed = updated.remove(index);
                Ok((EvalValue::List(updated), removed))
            }
            "reverse" => {
                expect_args("List", method, args, 0, span)?;
                let mut out = items.to_vec();
                out.reverse();
                Ok((same(), EvalValue::List(out)))
            }
            "sort" => {
                expect_args("List", method, args, 0, span)?;
                Ok((same(), EvalValue::List(sorted(items, span)?)))
            }
            "slice" => {
                let (start, end) = slice_bounds("List", items.len(), args, span)?;
                Ok((same(), EvalValue::List(items[start..end].to_vec())))
            }
            "find" => {
                expect_args("List", method, args, 1, span)?;
                let found = items.iter().find(|item| {
                    self.host_call_value(args[0].clone(), &[(*item).clone()], span)
                        .force_bool()
                });
                Ok((same(), found.cloned().map(some).unwrap_or_else(none)))
            }
            "map" => {
                expect_args("List", method, args, 1, span)?;
                let mapped = items
                    .iter()
                    .map(|item| self.host_call_value(args[0].clone(), &[item.clone()], span))
                    .collect();
                Ok((same(), EvalValue::List(mapped)))
            }
            "filter" => {
                expect_args("List", method, args, 1, span)?;
                let kept = items
                    .iter()
                    .filter(|item| {
                        self.host_call_value(args[0].clone(), &[(*item).clone()], span)
                            .force_bool()
                    })
                    .cloned()
                    .collect();
                Ok((same(), EvalValue::List(kept)))
            }
            _ => fail(span, format!("Unknown method 'List.{method}'")),
        }
    }
}

fn string_method(s: &str, method: &str, args: &[EvalValue], span: Span) -> MethodResult {
    let same = || EvalValue::String(s.to_string());
    let text = |value: String| Ok((same(), EvalValue::String(value)));
    let flag = |value: bool| Ok((same(), EvalValue::Bool(value)));
    match method {
        "len" => {
            expect_args("String", method, args, 0, span)?;
            Ok((same(), EvalValue::Number(s.chars().count() as f64)))
        }
        "is_empty" => {
            expect_args("String", method, args, 0, span)?;
            flag(s.is_empty())
        }
        "trim" => {
            expect_args("String", method, args, 0, span)?;
            text(s.trim().to_string())
        }
        "to_upper" => {
            expect_args("String", method, args, 0, span)?;
            text(s.to_uppercase())
        }
        "to_lower" => {
            expect_args("String", method, args, 0, span)?;
            text(s.to_lowercase())
        }
        "contains" => {
            expect_args("String", method, args, 1, span)?;
            flag(s.contains(str_arg("String", method, args, span)?))
        }
        "starts_with" => {
            expect_args("String", method, args, 1, span)?;
            flag(s.starts_with(str_arg("String", method, args, span)?))
        }
        "ends_with" => {
            expect_args("String", method, args, 1, span)?;
            flag(s.ends_with(str_arg("String", method, args, span)?))
        }
        "split" => {
            expect_args("String", method, args, 1, span)?;
            let sep = str_arg("String", method, args, span)?;
            let parts = if sep.is_empty() {
                list_of_strings(s.chars().map(|c| c.to_string()))
            } else {
                list_of_strings(s.split(sep).map(str::to_string))
            };
            Ok((same(), parts))
        }
        "slice" => {
            let chars: Vec<char> = s.chars().collect();
            let (start, end) = slice_bounds("String", chars.len(), args, span)?;
            text(chars[start..end].iter().collect())
        }
        "replace" => {
            expect_args("String", method, args, 2, span)?;
            match (&args[0], &args[1]) {
                (EvalValue::String(from), EvalValue::String(to)) => text(s.replace(from, to)),
                _ => fail(span, "String.replace expects two string arguments"),
            }
        }
        _ => fail(span, format!("Unknown method 'String.{method}'")),
    }
}

fn map_method(
    map: &std::collections::HashMap<String, EvalValue>,
    method: &str,
    args: &[EvalValue],
    span: Span,
) -> MethodResult {
    let same = || EvalValue::Map(map.clone());
    let sorted_keys = || {
        let mut keys: Vec<&String> = map.keys().collect();
        keys.sort();
        keys
    };
    match method {
        "len" => {
            expect_args("Map", method, args, 0, span)?;
            Ok((same(), EvalValue::Number(map.len() as f64)))
        }
        "is_empty" => {
            expect_args("Map", method, args, 0, span)?;
            Ok((same(), EvalValue::Bool(map.is_empty())))
        }
        "has" => {
            expect_args("Map", method, args, 1, span)?;
            Ok((
                same(),
                EvalValue::Bool(map.contains_key(&args[0].to_string())),
            ))
        }
        "get" => {
            expect_args("Map", method, args, 1, span)?;
            let found = map.get(&args[0].to_string()).cloned().map(some);
            Ok((same(), found.unwrap_or_else(none)))
        }
        "set" => {
            expect_args("Map", method, args, 2, span)?;
            let mut updated = map.clone();
            updated.insert(args[0].to_string(), args[1].clone());
            Ok((EvalValue::Map(updated), EvalValue::Unit))
        }
        "remove" => {
            expect_args("Map", method, args, 1, span)?;
            let mut updated = map.clone();
            let removed = updated.remove(&args[0].to_string()).map(some);
            Ok((EvalValue::Map(updated), removed.unwrap_or_else(none)))
        }
        "keys" => {
            expect_args("Map", method, args, 0, span)?;
            Ok((same(), list_of_strings(sorted_keys().into_iter().cloned())))
        }
        "values" => {
            expect_args("Map", method, args, 0, span)?;
            let values = sorted_keys().into_iter().map(|k| map[k].clone()).collect();
            Ok((same(), EvalValue::List(values)))
        }
        _ => fail(span, format!("Unknown method 'Map.{method}'")),
    }
}
