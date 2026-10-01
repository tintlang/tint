//! Values crossing to the host as JSON: what a call of a host function (JS or
//! Rust) receives and returns. Numbers are JSON numbers, strings, booleans and
//! lists are themselves, maps are objects, structs are objects tagged with
//! `__tint_struct`, enums are `{ "__tint_enum", "variant", "args" }` (the
//! convention the interpreter's host bridge already uses), `unit` is `null`.

use super::interp::{AdtVal, Val};
use super::ty::*;
use serde_json::{Map, Number, Value as J};
use std::collections::BTreeMap;
use std::rc::Rc;

/// The `Result` a host callback of parameter type `result_ty` receives: `Ok(value)` (the
/// host's JSON converted to the payload type) or `Err(message)`.
pub fn callback_result(types: &TypeTable, result_ty: TyId, outcome: Result<&J, &str>) -> Val {
    let TyKind::Adt(adt) = types.kind(result_ty) else { return Val::Unit };
    let def = types.adt(*adt);
    let (tag, payload) = match outcome {
        Ok(j) => match def.args.first() {
            Some(t) if !matches!(types.kind(*t), TyKind::Unit) => match json_to_val(types, *t, j) {
                Ok(v) => (def.variant_index("Ok").unwrap_or(0), v),
                Err(e) => (def.variant_index("Err").unwrap_or(1), Val::str(e)),
            },
            _ => (def.variant_index("Ok").unwrap_or(0), Val::Unit),
        },
        Err(e) => (def.variant_index("Err").unwrap_or(1), Val::str(e)),
    };
    Val::Adt(Rc::new(AdtVal { adt: *adt, tag, fields: vec![payload] }))
}

/// Can a value of this type be handed to the host (and read back)?
pub fn marshalable(types: &TypeTable, ty: TyId) -> bool {
    fn go(types: &TypeTable, ty: TyId, depth: u32) -> bool {
        if depth > 8 {
            return true;
        }
        match types.kind(ty) {
            TyKind::Unit | TyKind::Bool | TyKind::Num(_) | TyKind::Str => true,
            TyKind::List(t) | TyKind::Map(t) => go(types, *t, depth + 1),
            TyKind::Tuple(ts) => ts.iter().all(|t| go(types, *t, depth + 1)),
            TyKind::Adt(a) => {
                let def = types.adt(*a);
                def.struct_fields().iter().all(|f| go(types, f.ty, depth + 1))
                    && def.variants().iter().all(|v| v.fields.iter().all(|f| go(types, f.ty, depth + 1)))
            }
            TyKind::Fn(..) => false,
        }
    }
    go(types, ty, 0)
}

pub fn val_to_json(types: &TypeTable, ty: TyId, v: &Val) -> Result<J, String> {
    Ok(match (types.kind(ty), v) {
        (TyKind::Unit, _) => J::Null,
        (TyKind::Bool, Val::Bool(b)) => J::Bool(*b),
        (TyKind::Num(k), Val::Int(i)) => {
            if *k == NumKind::U64 {
                J::Number(Number::from(*i as u64))
            } else {
                J::Number(Number::from(*i))
            }
        }
        (TyKind::Num(_), Val::Float(f)) => Number::from_f64(*f).map(J::Number).unwrap_or(J::Null),
        (TyKind::Str, Val::Str(s)) => J::String(s.to_string()),
        (TyKind::List(t), Val::List(items)) => {
            J::Array(items.iter().map(|x| val_to_json(types, *t, x)).collect::<Result<_, _>>()?)
        }
        (TyKind::Map(t), Val::Map(m)) => {
            let mut out = Map::new();
            for (k, x) in m.iter() {
                out.insert(k.clone(), val_to_json(types, *t, x)?);
            }
            J::Object(out)
        }
        (TyKind::Tuple(ts), Val::Tuple(items)) => {
            J::Array(ts.iter().zip(items.iter()).map(|(t, x)| val_to_json(types, *t, x)).collect::<Result<_, _>>()?)
        }
        (TyKind::Adt(a), Val::Adt(av)) => {
            let def = types.adt(*a);
            if def.is_enum() {
                let variant = &def.variants()[av.tag as usize];
                let args = variant
                    .fields
                    .iter()
                    .zip(av.fields.iter())
                    .map(|(f, x)| val_to_json(types, f.ty, x))
                    .collect::<Result<Vec<_>, _>>()?;
                let mut out = Map::new();
                out.insert("__tint_enum".into(), J::String(def.base.clone()));
                out.insert("variant".into(), J::String(variant.name.clone()));
                out.insert("args".into(), J::Array(args));
                J::Object(out)
            } else {
                let mut out = Map::new();
                out.insert("__tint_struct".into(), J::String(def.base.clone()));
                for (f, x) in def.struct_fields().iter().zip(av.fields.iter()) {
                    out.insert(f.name.clone(), val_to_json(types, f.ty, x)?);
                }
                J::Object(out)
            }
        }
        (kind, _) => return Err(format!("cannot pass a {kind:?} value to the host")),
    })
}

pub fn json_to_val(types: &TypeTable, ty: TyId, j: &J) -> Result<Val, String> {
    let want = |what: &str| format!("the host returned {j} where {what} was expected");
    Ok(match types.kind(ty) {
        TyKind::Unit => Val::Unit,
        TyKind::Bool => Val::Bool(j.as_bool().ok_or_else(|| want("a bool"))?),
        TyKind::Num(k) => {
            let f = j.as_f64().ok_or_else(|| want("a number"))?;
            if k.is_float() {
                Val::Float(if *k == NumKind::F32 { f as f32 as f64 } else { f })
            } else {
                let (lo, hi) = k.int_range();
                if f.fract() != 0.0 || (f as i128) < lo || (f as i128) > hi {
                    return Err(want(&format!("a {}", k.name())));
                }
                Val::Int(if *k == NumKind::U64 { (f as u64) as i64 } else { f as i64 })
            }
        }
        TyKind::Str => Val::str(j.as_str().ok_or_else(|| want("a string"))?),
        TyKind::List(t) => Val::List(Rc::new(
            j.as_array()
                .ok_or_else(|| want("a list"))?
                .iter()
                .map(|x| json_to_val(types, *t, x))
                .collect::<Result<_, _>>()?,
        )),
        TyKind::Map(t) => {
            let mut out = BTreeMap::new();
            for (k, x) in j.as_object().ok_or_else(|| want("a map"))? {
                out.insert(k.clone(), json_to_val(types, *t, x)?);
            }
            Val::Map(Rc::new(out))
        }
        TyKind::Tuple(ts) => {
            let items = j.as_array().ok_or_else(|| want("a tuple"))?;
            if items.len() != ts.len() {
                return Err(want("a tuple of the right size"));
            }
            Val::Tuple(Rc::new(
                ts.iter().zip(items).map(|(t, x)| json_to_val(types, *t, x)).collect::<Result<_, _>>()?,
            ))
        }
        TyKind::Adt(a) => {
            let def = types.adt(*a);
            let obj = j.as_object().ok_or_else(|| want("an object"))?;
            if def.is_enum() {
                let name = obj.get("variant").and_then(J::as_str).ok_or_else(|| want("an enum value"))?;
                let tag = def.variant_index(name).ok_or_else(|| want(&format!("a variant of {}", def.base)))?;
                let variant = &def.variants()[tag as usize];
                let args = obj.get("args").and_then(J::as_array).cloned().unwrap_or_default();
                if args.len() != variant.fields.len() {
                    return Err(want("the variant's payload"));
                }
                let fields = variant
                    .fields
                    .iter()
                    .zip(&args)
                    .map(|(f, x)| json_to_val(types, f.ty, x))
                    .collect::<Result<_, _>>()?;
                Val::Adt(Rc::new(AdtVal { adt: *a, tag, fields }))
            } else {
                let fields = def
                    .struct_fields()
                    .iter()
                    .map(|f| {
                        let x = obj.get(&f.name).ok_or_else(|| want(&format!("a field `{}`", f.name)))?;
                        json_to_val(types, f.ty, x)
                    })
                    .collect::<Result<_, _>>()?;
                Val::Adt(Rc::new(AdtVal { adt: *a, tag: 0, fields }))
            }
        }
        TyKind::Fn(..) => return Err("a function cannot come from the host".into()),
    })
}

/// The canonical JSON of a host component's props: struct fields in declaration order, map keys
/// sorted, tuples as arrays, enums and anything else `null` (what the interpreter's UI builder writes).
pub fn props_json(types: &TypeTable, ty: TyId, v: &Val) -> String {
    let mut out = String::new();
    props(types, ty, v, &mut out);
    out
}

fn props(types: &TypeTable, ty: TyId, v: &Val, out: &mut String) {
    match (types.kind(ty), v) {
        (TyKind::Bool, Val::Bool(b)) => out.push_str(if *b { "true" } else { "false" }),
        (TyKind::Num(k), Val::Int(i)) => {
            if *k == NumKind::U64 {
                out.push_str(&(*i as u64).to_string())
            } else {
                out.push_str(&i.to_string())
            }
        }
        (TyKind::Num(_), Val::Float(f)) if f.is_finite() => out.push_str(&f.to_string()),
        (TyKind::Str, Val::Str(s)) => out.push_str(&J::String(s.to_string()).to_string()),
        (TyKind::List(t), Val::List(items)) => {
            out.push('[');
            for (i, x) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                props(types, *t, x, out);
            }
            out.push(']');
        }
        (TyKind::Tuple(ts), Val::Tuple(items)) => {
            out.push('[');
            for (i, (t, x)) in ts.iter().zip(items.iter()).enumerate() {
                if i > 0 {
                    out.push(',');
                }
                props(types, *t, x, out);
            }
            out.push(']');
        }
        (TyKind::Map(t), Val::Map(m)) => {
            let sorted: BTreeMap<&String, &Val> = m.iter().map(|(k, x)| (k, x)).collect();
            out.push('{');
            for (i, (k, x)) in sorted.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&J::String(k.clone()).to_string());
                out.push(':');
                props(types, *t, x, out);
            }
            out.push('}');
        }
        (TyKind::Adt(a), Val::Adt(av)) if !types.adt(*a).is_enum() => {
            out.push('{');
            for (i, (f, x)) in types.adt(*a).struct_fields().iter().zip(av.fields.iter()).enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&J::String(f.name.clone()).to_string());
                out.push(':');
                props(types, f.ty, x, out);
            }
            out.push('}');
        }
        _ => out.push_str("null"),
    }
}
