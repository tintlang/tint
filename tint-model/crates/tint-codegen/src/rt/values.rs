// ---- values <-> heap objects ------------------------------------------------

fn scalar_val(kind: NumKind, bits: u64) -> Val {
    if kind.is_float() {
        Val::Float(f64::from_bits(bits))
    } else {
        Val::Int(bits as i64)
    }
}

/// Reads the elements of the list `p` (elements of type `elem`).
unsafe fn list_to_val(m: &Module, elem: TyId, p: Ptr) -> Val {
    let l = &*(p as *const List);
    let layout = elem_layout(m, elem);
    let mut items = Vec::with_capacity(l.len);
    for i in 0..l.len {
        let at = l.ptr.add(i * l.esize);
        let bits = if layout.heap {
            *(at as *const u64)
        } else {
            let mut raw = [0u8; 8];
            std::ptr::copy_nonoverlapping(at, raw.as_mut_ptr(), l.esize);
            let v = u64::from_le_bytes(raw);
            if layout.signed && l.esize == 4 {
                v as u32 as i32 as i64 as u64
            } else {
                v
            }
        };
        items.push(to_val(m, elem, bits));
    }
    Val::list(items)
}

/// An interpreter value for the register value `bits` of type `ty`
/// (the object stays owned by the caller).
pub fn to_val(m: &Module, ty: TyId, bits: u64) -> Val {
    let p = bits as Ptr;
    unsafe {
        match m.types.kind(ty) {
            TyKind::Unit => Val::Unit,
            TyKind::Bool => Val::Bool(bits & 0xff != 0),
            TyKind::Num(k) => scalar_val(*k, bits),
            TyKind::Str => Val::str(str_of(p)),
            TyKind::List(e) => list_to_val(m, *e, p),
            TyKind::Tuple(items) => {
                let vals = items
                    .iter()
                    .enumerate()
                    .map(|(i, t)| to_val(m, *t, *(slot_ptr(p, i) as *const u64)))
                    .collect();
                Val::Tuple(Rc::new(vals))
            }
            TyKind::Adt(adt) => {
                let def = m.types.adt(*adt);
                let (tag, fields) = match &def.body {
                    AdtBody::Struct(fields) => (0, fields.iter().map(|f| f.ty).collect::<Vec<_>>()),
                    AdtBody::Enum(variants) => {
                        let tag = *(slot_ptr(p, 0) as *const u64) as usize;
                        (
                            tag as u32,
                            variants[tag].fields.iter().map(|f| f.ty).collect(),
                        )
                    }
                };
                let base = if matches!(def.body, AdtBody::Enum(_)) {
                    1
                } else {
                    0
                };
                let vals = fields
                    .iter()
                    .enumerate()
                    .map(|(i, t)| to_val(m, *t, *(slot_ptr(p, base + i) as *const u64)))
                    .collect();
                Val::Adt(Rc::new(AdtVal {
                    adt: *adt,
                    tag,
                    fields: vals,
                }))
            }
            TyKind::Map(v) => {
                let map = &*(p as *const Map);
                let entries: BTreeMap<String, Val> = map
                    .map
                    .iter()
                    .map(|(k, bits)| (k.clone(), to_val(m, *v, *bits)))
                    .collect();
                Val::Map(Rc::new(entries))
            }
            TyKind::Fn(..) => {
                let id = FuncId(*(slot_ptr(p, 0) as *const u64) as u32);
                let f = m.func(id);
                let captures = (0..f.ncaptures as usize)
                    .map(|i| {
                        to_val(
                            m,
                            f.reg_ty(f.params[i]),
                            *(slot_ptr(p, 1 + i) as *const u64),
                        )
                    })
                    .collect();
                Val::Closure(Rc::new(ClosureVal { func: id, captures }))
            }
        }
    }
}

fn float_bits(v: &Val) -> u64 {
    match v {
        Val::Float(f) => f.to_bits(),
        Val::Int(i) => *i as u64,
        Val::Bool(b) => *b as u64,
        _ => 0,
    }
}

/// A fresh object (one owned reference) or scalar bits for `v` of type `ty`.
pub fn from_val(m: &Module, ty: TyId, v: &Val) -> u64 {
    match (m.types.kind(ty), v) {
        (TyKind::Unit, _) => 0,
        (TyKind::Bool | TyKind::Num(_), v) => float_bits(v),
        (TyKind::Str, Val::Str(s)) => new_str(s.to_string()) as u64,
        (TyKind::List(e), Val::List(items)) => {
            let layout = elem_layout(m, *e);
            let mut p = list_new(items.len() as i64, layout.size, layout.heap as i64);
            for item in items.iter() {
                p = list_push(p, from_val(m, *e, item));
            }
            p as u64
        }
        (TyKind::Tuple(tys), Val::Tuple(items)) => {
            let mask = ptr_mask(m, tys, 0);
            let p = obj_new(tys.len() as i64, mask);
            for (i, (t, item)) in tys.iter().zip(items.iter()).enumerate() {
                unsafe { *(slot_ptr(p, i) as *mut u64) = from_val(m, *t, item) };
            }
            p as u64
        }
        (TyKind::Adt(adt), Val::Adt(a)) => {
            let def = m.types.adt(*adt);
            let (tys, base): (Vec<TyId>, usize) = match &def.body {
                AdtBody::Struct(fields) => (fields.iter().map(|f| f.ty).collect(), 0),
                AdtBody::Enum(variants) => (
                    variants[a.tag as usize]
                        .fields
                        .iter()
                        .map(|f| f.ty)
                        .collect(),
                    1,
                ),
            };
            let nslots = crate::adt_layout_slots(m, *adt);
            let p = obj_new(nslots as i64, ptr_mask(m, &tys, base));
            unsafe {
                if base == 1 {
                    *(slot_ptr(p, 0) as *mut u64) = a.tag as u64;
                }
                for (i, (t, item)) in tys.iter().zip(a.fields.iter()).enumerate() {
                    *(slot_ptr(p, base + i) as *mut u64) = from_val(m, *t, item);
                }
            }
            p as u64
        }
        (TyKind::Map(e), Val::Map(entries)) => {
            let mut p = map_new(is_heap(m, *e) as i64);
            for (k, item) in entries.iter() {
                let key = new_str(k.clone());
                p = map_set(p, key, from_val(m, *e, item));
                release(key);
            }
            p as u64
        }
        (TyKind::Fn(..), Val::Closure(c)) => {
            let f = m.func(c.func);
            let tys: Vec<TyId> = (0..f.ncaptures as usize)
                .map(|i| f.reg_ty(f.params[i]))
                .collect();
            let p = obj_new(1 + tys.len() as i64, ptr_mask(m, &tys, 1));
            unsafe {
                *(slot_ptr(p, 0) as *mut u64) = c.func.0 as u64;
                for (i, (t, item)) in tys.iter().zip(c.captures.iter()).enumerate() {
                    *(slot_ptr(p, 1 + i) as *mut u64) = from_val(m, *t, item);
                }
            }
            p as u64
        }
        (other, v) => fail(format!("internal error: cannot build {other:?} from {v:?}")),
    }
}
