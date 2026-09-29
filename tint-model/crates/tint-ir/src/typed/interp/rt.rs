//! The runtime library: strings, lists, maps and math.

use super::*;

fn str_of<'a>(v: &'a Val) -> Res<&'a str> {
    match v {
        Val::Str(s) => Ok(s),
        other => Err(bad("expected a string", other)),
    }
}

fn float_of(v: &Val) -> Res<f64> {
    match v {
        Val::Float(f) => Ok(*f),
        other => Err(bad("expected a number", other)),
    }
}

fn bool_val(b: bool) -> Val {
    Val::Bool(b)
}

fn count(n: usize) -> Val {
    Val::Float(n as f64)
}

/// A non-negative integer index or bound (`slice`, `remove`).
fn whole(what: &str, v: &Val) -> Res<usize> {
    match v {
        Val::Float(f) if *f >= 0.0 && f.fract() == 0.0 => Ok(*f as usize),
        Val::Int(i) if *i >= 0 => Ok(*i as usize),
        _ => Err(Trap::new(format!("{what} expects non-negative integer indexes"))),
    }
}

fn slice_bounds(owner: &str, len: usize, args: &[&Val]) -> Res<(usize, usize)> {
    let mut bounds = [0, len];
    for (slot, arg) in bounds.iter_mut().zip(args) {
        *slot = whole(&format!("{owner}.slice"), arg)?.min(len);
    }
    Ok((bounds[0], bounds[1].max(bounds[0])))
}

impl<'m> Interp<'m> {
    /// `Some(value)` / `None` / `Ok(value)` / `Err(value)` of the ADT `ty`.
    fn variant_of(&self, ty: TyId, name: &str, fields: Vec<Val>) -> Res<Val> {
        let adt = self
            .module
            .types
            .as_adt(ty)
            .ok_or_else(|| Trap::new("internal error: runtime call result is not an enum"))?;
        let tag = self
            .module
            .types
            .adt(adt)
            .variant_index(name)
            .ok_or_else(|| Trap::new(format!("internal error: no variant {name}")))?;
        Ok(Val::Adt(Rc::new(AdtVal { adt, tag, fields })))
    }

    pub(super) fn call_rt(
        &mut self,
        func: &Func,
        regs: &mut Vec<Val>,
        dst: Reg,
        f: RtFn,
        args: &[Reg],
    ) -> Res<()> {
        let dst_ty = func.reg_ty(dst);
        let types = &self.module.types;
        let arg = |i: usize| -> &Val { &regs[args[i].0 as usize] };
        let result: Val = match f {
            RtFn::StrConcat => {
                let mut out = String::new();
                for a in args {
                    out.push_str(str_of(&regs[a.0 as usize])?);
                }
                Val::str(out)
            }
            RtFn::StrLen => count(str_of(arg(0))?.chars().count()),
            RtFn::StrIsEmpty => bool_val(str_of(arg(0))?.is_empty()),
            RtFn::StrTrim => Val::str(str_of(arg(0))?.trim()),
            RtFn::StrUpper => Val::str(str_of(arg(0))?.to_uppercase()),
            RtFn::StrLower => Val::str(str_of(arg(0))?.to_lowercase()),
            RtFn::StrContains => bool_val(str_of(arg(0))?.contains(str_of(arg(1))?)),
            RtFn::StrStartsWith => bool_val(str_of(arg(0))?.starts_with(str_of(arg(1))?)),
            RtFn::StrEndsWith => bool_val(str_of(arg(0))?.ends_with(str_of(arg(1))?)),
            RtFn::StrSlice => {
                let chars: Vec<char> = str_of(arg(0))?.chars().collect();
                let bounds: Vec<&Val> = (1..args.len()).map(|i| arg(i)).collect();
                let (start, end) = slice_bounds("String", chars.len(), &bounds)?;
                Val::str(chars[start..end].iter().collect::<String>())
            }
            RtFn::StrSplit => {
                let (s, sep) = (str_of(arg(0))?, str_of(arg(1))?);
                let parts: Vec<Val> = if sep.is_empty() {
                    s.chars().map(|c| Val::str(c.to_string())).collect()
                } else {
                    s.split(sep).map(Val::str).collect()
                };
                Val::list(parts)
            }
            RtFn::StrReplace => {
                Val::str(str_of(arg(0))?.replace(str_of(arg(1))?, str_of(arg(2))?))
            }

            RtFn::ListLen | RtFn::ListIsEmpty | RtFn::MapLen | RtFn::MapIsEmpty => {
                let len = match arg(0) {
                    Val::List(l) => l.len(),
                    Val::Map(m) => m.len(),
                    other => return Err(bad("length of a non-collection", other)),
                };
                if matches!(f, RtFn::ListLen | RtFn::MapLen) {
                    count(len)
                } else {
                    bool_val(len == 0)
                }
            }
            RtFn::ListPush => {
                let item = arg(1).clone();
                match &mut regs[args[0].0 as usize] {
                    Val::List(l) => Rc::make_mut(l).push(item),
                    other => return Err(bad("push on a non-list", other)),
                }
                Val::Unit
            }
            RtFn::ListPop => {
                let popped = match &mut regs[args[0].0 as usize] {
                    Val::List(l) => Rc::make_mut(l).pop(),
                    other => return Err(bad("pop on a non-list", other)),
                };
                match popped {
                    Some(v) => self.variant_of(dst_ty, "Some", vec![v])?,
                    None => self.variant_of(dst_ty, "None", vec![])?,
                }
            }
            RtFn::ListRemove => {
                let index = whole("List.remove", arg(1))?;
                match &mut regs[args[0].0 as usize] {
                    Val::List(l) => {
                        if index >= l.len() {
                            return Err(Trap::new(format!(
                                "List.remove index {index} out of bounds (len={})",
                                l.len()
                            )));
                        }
                        Rc::make_mut(l).remove(index)
                    }
                    other => return Err(bad("remove on a non-list", other)),
                }
            }
            RtFn::ListReverse => match arg(0) {
                Val::List(l) => Val::list(l.iter().rev().cloned().collect()),
                other => return Err(bad("reverse of a non-list", other)),
            },
            RtFn::ListSort => {
                let Val::List(items) = arg(0) else {
                    return Err(bad("sort of a non-list", arg(0)));
                };
                let mut out: Vec<Val> = items.as_ref().clone();
                let item_ty = match types.kind(func.reg_ty(args[0])) {
                    TyKind::List(item) => types.kind(*item),
                    _ => return Err(Trap::new("internal error: sort of a non-list type")),
                };
                match item_ty {
                    TyKind::Str => out.sort_by(|a, b| match (a, b) {
                        (Val::Str(x), Val::Str(y)) => x.as_ref().cmp(y.as_ref()),
                        _ => std::cmp::Ordering::Equal,
                    }),
                    TyKind::Num(kind) if kind.is_float() => out.sort_by(|a, b| match (a, b) {
                        (Val::Float(x), Val::Float(y)) => x.total_cmp(y),
                        _ => std::cmp::Ordering::Equal,
                    }),
                    TyKind::Num(kind) => {
                        let kind = *kind;
                        out.sort_by_key(|v| int_of(kind, v).unwrap_or(0));
                    }
                    _ => return Err(Trap::new("List.sort expects only numbers or only strings")),
                }
                Val::list(out)
            }
            RtFn::ListSlice => {
                let Val::List(items) = arg(0) else {
                    return Err(bad("slice of a non-list", arg(0)));
                };
                let bounds: Vec<&Val> = (1..args.len()).map(|i| arg(i)).collect();
                let (start, end) = slice_bounds("List", items.len(), &bounds)?;
                Val::list(items[start..end].to_vec())
            }
            RtFn::ListContains => {
                let Val::List(items) = arg(0) else {
                    return Err(bad("contains on a non-list", arg(0)));
                };
                let item_ty = match types.kind(func.reg_ty(args[0])) {
                    TyKind::List(item) => *item,
                    _ => return Err(Trap::new("internal error: contains on a non-list type")),
                };
                let mut found = false;
                for item in items.iter() {
                    if val_eq(types, item_ty, item, arg(1))? {
                        found = true;
                        break;
                    }
                }
                bool_val(found)
            }
            RtFn::ListJoin => {
                let Val::List(items) = arg(0) else {
                    return Err(bad("join of a non-list", arg(0)));
                };
                let item_ty = match types.kind(func.reg_ty(args[0])) {
                    TyKind::List(item) => *item,
                    _ => return Err(Trap::new("internal error: join of a non-list type")),
                };
                let parts: Vec<String> =
                    items.iter().map(|v| display(self.module, item_ty, v)).collect();
                Val::str(parts.join(str_of(arg(1))?))
            }

            RtFn::MapHas => match arg(0) {
                Val::Map(m) => bool_val(m.contains_key(str_of(arg(1))?)),
                other => return Err(bad("has on a non-map", other)),
            },
            RtFn::MapGet => {
                let found = match arg(0) {
                    Val::Map(m) => m.get(str_of(arg(1))?).cloned(),
                    other => return Err(bad("get on a non-map", other)),
                };
                match found {
                    Some(v) => self.variant_of(dst_ty, "Some", vec![v])?,
                    None => self.variant_of(dst_ty, "None", vec![])?,
                }
            }
            RtFn::MapSet => {
                let key = str_of(arg(1))?.to_string();
                let value = arg(2).clone();
                match &mut regs[args[0].0 as usize] {
                    Val::Map(m) => {
                        Rc::make_mut(m).insert(key, value);
                    }
                    other => return Err(bad("set on a non-map", other)),
                }
                Val::Unit
            }
            RtFn::MapRemove => {
                let key = str_of(arg(1))?.to_string();
                let removed = match &mut regs[args[0].0 as usize] {
                    Val::Map(m) => Rc::make_mut(m).remove(&key),
                    other => return Err(bad("remove on a non-map", other)),
                };
                match removed {
                    Some(v) => self.variant_of(dst_ty, "Some", vec![v])?,
                    None => self.variant_of(dst_ty, "None", vec![])?,
                }
            }
            RtFn::MapKeys => match arg(0) {
                Val::Map(m) => Val::list(m.keys().map(Val::str).collect()),
                other => return Err(bad("keys of a non-map", other)),
            },
            RtFn::MapValues => match arg(0) {
                Val::Map(m) => Val::list(m.values().cloned().collect()),
                other => return Err(bad("values of a non-map", other)),
            },

            RtFn::Sqrt => Val::Float(float_of(arg(0))?.sqrt()),
            RtFn::Min => Val::Float(float_of(arg(0))?.min(float_of(arg(1))?)),
            RtFn::Max => Val::Float(float_of(arg(0))?.max(float_of(arg(1))?)),
            RtFn::Abs => Val::Float(float_of(arg(0))?.abs()),
            RtFn::Sign => {
                let x = float_of(arg(0))?;
                Val::Float(if x > 0.0 {
                    1.0
                } else if x < 0.0 {
                    -1.0
                } else {
                    0.0
                })
            }
            RtFn::Clamp => {
                Val::Float(float_of(arg(0))?.max(float_of(arg(1))?).min(float_of(arg(2))?))
            }
            RtFn::ParseNumber => {
                let text = str_of(arg(0))?;
                match text.trim().parse::<f64>() {
                    Ok(n) => self.variant_of(dst_ty, "Ok", vec![Val::Float(n)])?,
                    Err(_) => self.variant_of(
                        dst_ty,
                        "Err",
                        vec![Val::str(format!("invalid number: {text}"))],
                    )?,
                }
            }
            RtFn::Vec2Length | RtFn::Vec2Normalized => {
                let Val::Adt(v) = arg(0) else {
                    return Err(bad("Vec2 method on a non-Vec2", arg(0)));
                };
                let (x, y) = (float_of(&v.fields[0])?, float_of(&v.fields[1])?);
                let length = (x * x + y * y).sqrt();
                if f == RtFn::Vec2Length {
                    Val::Float(length)
                } else {
                    let (x, y) = if length == 0.0 { (0.0, 0.0) } else { (x / length, y / length) };
                    Val::Adt(Rc::new(AdtVal {
                        adt: v.adt,
                        tag: 0,
                        fields: vec![Val::Float(x), Val::Float(y)],
                    }))
                }
            }
        };
        regs[dst.0 as usize] = result;
        Ok(())
    }
}
