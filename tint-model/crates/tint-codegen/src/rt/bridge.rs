// ---- bridge to the reference implementation ---------------------------------

pub const RT_FNS: &[RtFn] = &[
    RtFn::StrConcat,
    RtFn::StrLen,
    RtFn::StrIsEmpty,
    RtFn::StrTrim,
    RtFn::StrUpper,
    RtFn::StrLower,
    RtFn::StrContains,
    RtFn::StrStartsWith,
    RtFn::StrEndsWith,
    RtFn::StrSlice,
    RtFn::StrSplit,
    RtFn::StrReplace,
    RtFn::ListLen,
    RtFn::ListIsEmpty,
    RtFn::ListPush,
    RtFn::ListPop,
    RtFn::ListRemove,
    RtFn::ListReverse,
    RtFn::ListSort,
    RtFn::ListSlice,
    RtFn::ListContains,
    RtFn::ListJoin,
    RtFn::MapLen,
    RtFn::MapIsEmpty,
    RtFn::MapHas,
    RtFn::MapGet,
    RtFn::MapSet,
    RtFn::MapRemove,
    RtFn::MapKeys,
    RtFn::MapValues,
    RtFn::Sqrt,
    RtFn::Min,
    RtFn::Max,
    RtFn::Abs,
    RtFn::Sign,
    RtFn::Clamp,
    RtFn::ParseNumber,
    RtFn::Vec2Length,
    RtFn::Vec2Normalized,
];

pub fn rt_fn_index(f: RtFn) -> i64 {
    RT_FNS.iter().position(|x| *x == f).expect("listed") as i64
}

/// Runs a runtime call on the interpreter. `args` holds the operands (borrowed);
/// for a call that mutates its first operand, `args[0]` receives the new
/// object (one owned reference) that replaces it.
pub extern "C" fn rt_call(f: i64, n: i64, args: *mut u64, tys: *const u32, dst_ty: u32) -> u64 {
    let m = module();
    let n = n as usize;
    let f = RT_FNS[f as usize];
    let arg_tys: Vec<TyId> = (0..n).map(|i| TyId(unsafe { *tys.add(i) })).collect();
    let vals: Vec<Val> = (0..n)
        .map(|i| to_val(m, arg_tys[i], unsafe { *args.add(i) }))
        .collect();
    let dst_ty = TyId(dst_ty);
    let outcome = CTX.with(|c| {
        let mut c = c.borrow_mut();
        c.interp
            .as_mut()
            .expect("entered")
            .rt_values(f, &arg_tys, dst_ty, vals)
    });
    match outcome {
        Ok((result, after)) => {
            if f.mutates_first() {
                unsafe { *args = from_val(m, arg_tys[0], &after[0]) };
            }
            from_val(m, dst_ty, &result)
        }
        Err(trap) => fail(trap.msg),
    }
}

/// `a <op> b` on two values of type `ty`.
pub extern "C" fn rt_cmp(ty: u32, op: u32, a: u64, b: u64) -> u8 {
    let m = module();
    let ty = TyId(ty);
    let ops = [
        CmpOp::Eq,
        CmpOp::Ne,
        CmpOp::Lt,
        CmpOp::Le,
        CmpOp::Gt,
        CmpOp::Ge,
    ];
    let (va, vb) = (to_val(m, ty, a), to_val(m, ty, b));
    let outcome = CTX.with(|c| {
        c.borrow()
            .interp
            .as_ref()
            .expect("entered")
            .compare_values(ty, ops[op as usize], &va, &vb)
    });
    match outcome {
        Ok(r) => r as u8,
        Err(trap) => fail(trap.msg),
    }
}

pub const NUM_KINDS: [NumKind; 8] = [
    NumKind::Num,
    NumKind::I32,
    NumKind::I64,
    NumKind::U8,
    NumKind::U32,
    NumKind::U64,
    NumKind::F32,
    NumKind::F64,
];

pub fn num_kind_index(k: NumKind) -> i64 {
    NUM_KINDS.iter().position(|x| *x == k).unwrap() as i64
}

/// A checked numeric conversion (`as`).
pub extern "C" fn rt_cast(from: u32, to: u32, bits: u64) -> u64 {
    let (from, to) = (NUM_KINDS[from as usize], NUM_KINDS[to as usize]);
    match cast_num(from, to, &scalar_val(from, bits)) {
        Ok(v) => float_bits(&v),
        Err(trap) => fail(trap.msg),
    }
}

/// The text `"{value}"` produces.
pub extern "C" fn rt_to_str(ty: u32, bits: u64) -> Ptr {
    let m = module();
    let ty = TyId(ty);
    new_str(display(m, ty, &to_val(m, ty, bits)))
}

pub extern "C" fn print(ty: i64, bits: i64, newline: i64) {
    use std::io::Write;
    let m = module();
    let ty = TyId(ty as u32);
    let text = display(m, ty, &to_val(m, ty, bits as u64));
    let mut out = std::io::stdout().lock();
    let _ = if newline != 0 {
        writeln!(out, "{text}")
    } else {
        write!(out, "{text}")
    };
}

/// The result of `main`, rendered like the interpreter renders it.
pub fn render_bits(m: &Module, ty: TyId, bits: u64) -> String {
    let text = render(m, ty, &to_val(m, ty, bits));
    if is_heap(m, ty) {
        release(bits as Ptr);
    }
    text
}
