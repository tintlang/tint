fn check(b: &mut FunctionBuilder, imp: &Imports, ok: Value, msg: i64) {
    let cont = b.create_block();
    let fail = b.create_block();
    b.set_cold_block(fail);
    b.ins().brif(ok, cont, &[], fail, &[]);
    b.switch_to_block(fail);
    let c = b.ins().iconst(types::I64, msg);
    b.ins().call(imp.f("trap"), &[c]);
    b.ins().trap(TrapCode::unwrap_user(1));
    b.switch_to_block(cont);
}

fn arith(
    b: &mut FunctionBuilder,
    imp: &Imports,
    op: BinOp,
    kind: NumKind,
    x: Value,
    y: Value,
    msg: &mut dyn FnMut(String) -> i64,
) -> Value {
    if kind.is_float() {
        let r = match op {
            BinOp::Add => b.ins().fadd(x, y),
            BinOp::Sub => b.ins().fsub(x, y),
            BinOp::Mul => b.ins().fmul(x, y),
            BinOp::Div => b.ins().fdiv(x, y),
            BinOp::Rem => float_rem(b, imp, x, y),
        };
        if kind == NumKind::F32 {
            let n = b.ins().fdemote(types::F32, r);
            return b.ins().fpromote(types::F64, n);
        }
        return r;
    }
    let overflow_msg = msg(format!("{} arithmetic overflow", kind.name()));
    let (lo, hi) = kind.int_range();
    match kind {
        NumKind::I64 | NumKind::U64 => {
            let signed = kind == NumKind::I64;
            match op {
                BinOp::Add | BinOp::Sub | BinOp::Mul => {
                    let (r, of) = match (op, signed) {
                        (BinOp::Add, true) => b.ins().sadd_overflow(x, y),
                        (BinOp::Sub, true) => b.ins().ssub_overflow(x, y),
                        (BinOp::Mul, true) => b.ins().smul_overflow(x, y),
                        (BinOp::Add, false) => b.ins().uadd_overflow(x, y),
                        (BinOp::Sub, false) => b.ins().usub_overflow(x, y),
                        _ => b.ins().umul_overflow(x, y),
                    };
                    let ok = b.ins().bxor_imm(of, 1);
                    check(b, imp, ok, overflow_msg);
                    r
                }
                BinOp::Div | BinOp::Rem => {
                    let zero_msg = msg("division by zero".into());
                    let nz = b.ins().icmp_imm(IntCC::NotEqual, y, 0);
                    check(b, imp, nz, zero_msg);
                    if signed {
                        // i64::MIN / -1 overflows.
                        let is_min = b.ins().icmp_imm(IntCC::Equal, x, i64::MIN);
                        let is_m1 = b.ins().icmp_imm(IntCC::Equal, y, -1);
                        let bad = b.ins().band(is_min, is_m1);
                        let ok = b.ins().bxor_imm(bad, 1);
                        check(b, imp, ok, overflow_msg);
                        if op == BinOp::Div {
                            b.ins().sdiv(x, y)
                        } else {
                            b.ins().srem(x, y)
                        }
                    } else if op == BinOp::Div {
                        b.ins().udiv(x, y)
                    } else {
                        b.ins().urem(x, y)
                    }
                }
            }
        }
        _ => {
            // 32-bit and 8-bit kinds: compute in i64, then range-check.
            let r = match op {
                BinOp::Add => b.ins().iadd(x, y),
                BinOp::Sub => b.ins().isub(x, y),
                BinOp::Mul => b.ins().imul(x, y),
                BinOp::Div | BinOp::Rem => {
                    let zero_msg = msg("division by zero".into());
                    let nz = b.ins().icmp_imm(IntCC::NotEqual, y, 0);
                    check(b, imp, nz, zero_msg);
                    if op == BinOp::Div {
                        b.ins().sdiv(x, y)
                    } else {
                        b.ins().srem(x, y)
                    }
                }
            };
            // (r - lo) <= (hi - lo) as unsigned
            let shifted = b.ins().iadd_imm(r, -(lo as i64));
            let ok = b
                .ins()
                .icmp_imm(IntCC::UnsignedLessThanOrEqual, shifted, (hi - lo) as i64);
            check(b, imp, ok, overflow_msg);
            r
        }
    }
}

/// `x % y` on floats. When both operands are integral and fit an i64 (the
/// common case: `number` holds counters and indices) use the integer unit,
/// otherwise the libm `fmod`. The sign of the result follows `x`, like fmod.
fn float_rem(b: &mut FunctionBuilder, imp: &Imports, x: Value, y: Value) -> Value {
    let slow = b.create_block();
    let fast = b.create_block();
    let done = b.create_block();
    b.append_block_param(done, types::F64);

    let xi = b.ins().fcvt_to_sint_sat(types::I64, x);
    let yi = b.ins().fcvt_to_sint_sat(types::I64, y);
    let xf = b.ins().fcvt_from_sint(types::F64, xi);
    let yf = b.ins().fcvt_from_sint(types::F64, yi);
    let x_int = b.ins().fcmp(FloatCC::Equal, xf, x);
    let y_int = b.ins().fcmp(FloatCC::Equal, yf, y);
    // |y| in 1..2^62 keeps srem defined (no MIN / -1) and rules out y == 0.
    let y_lo = b.ins().icmp_imm(IntCC::SignedGreaterThan, yi, 0);
    let y_neg = b.ins().icmp_imm(IntCC::SignedLessThan, yi, 0);
    let y_ok = b.ins().bor(y_lo, y_neg);
    let x_ok = b.ins().icmp_imm(IntCC::NotEqual, xi, i64::MIN);
    let y_ok2 = b.ins().icmp_imm(IntCC::NotEqual, yi, i64::MIN);
    let a = b.ins().band(x_int, y_int);
    let c = b.ins().band(y_ok, x_ok);
    let d = b.ins().band(a, c);
    let all = b.ins().band(d, y_ok2);
    b.ins().brif(all, fast, &[], slow, &[]);

    b.switch_to_block(fast);
    let r = b.ins().srem(xi, yi);
    let rf = b.ins().fcvt_from_sint(types::F64, r);
    let signed = b.ins().fcopysign(rf, x);
    b.ins().jump(done, &[signed.into()]);

    b.switch_to_block(slow);
    let call = b.ins().call(imp.f("fmod"), &[x, y]);
    let v = b.inst_results(call)[0];
    b.ins().jump(done, &[v.into()]);

    b.switch_to_block(done);
    b.block_params(done)[0]
}

/// For every instruction, the heap registers it reads for the last time: they
/// are not read again on any path before being written. Their reference is
/// given back right after the instruction (or moved into its result).
fn dying_regs(f: &Func, heap: &[bool]) -> Vec<Vec<Vec<Reg>>> {
    let n = f.regs.len();
    let term_uses = |t: &Term| -> Vec<Reg> {
        match t {
            Term::Branch { cond, .. } => vec![*cond],
            Term::Switch { value, .. } => vec![*value],
            Term::Return(r) => vec![*r],
            _ => vec![],
        }
    };
    // live_in per block by backward dataflow to a fixed point.
    let mut live_in = vec![vec![false; n]; f.blocks.len()];
    let live_out_of = |live_in: &Vec<Vec<bool>>, bi: usize| -> Vec<bool> {
        let mut out = vec![false; n];
        for succ in f.blocks[bi].term.successors() {
            for (o, i) in out.iter_mut().zip(&live_in[succ.0 as usize]) {
                *o |= *i;
            }
        }
        out
    };
    let mut changed = true;
    while changed {
        changed = false;
        for bi in (0..f.blocks.len()).rev() {
            let mut live = live_out_of(&live_in, bi);
            for r in term_uses(&f.blocks[bi].term) {
                live[r.0 as usize] = true;
            }
            for ins in f.blocks[bi].instrs.iter().rev() {
                if let Some(d) = ins.dst() {
                    live[d.0 as usize] = false;
                }
                for r in ins.uses() {
                    live[r.0 as usize] = true;
                }
            }
            if live != live_in[bi] {
                live_in[bi] = live;
                changed = true;
            }
        }
    }
    let mut out = Vec::new();
    for bi in 0..f.blocks.len() {
        let mut live = live_out_of(&live_in, bi);
        for r in term_uses(&f.blocks[bi].term) {
            live[r.0 as usize] = true;
        }
        let mut per_instr = vec![Vec::new(); f.blocks[bi].instrs.len()];
        for (ii, ins) in f.blocks[bi].instrs.iter().enumerate().rev() {
            // `live` is what is live after this instruction.
            let dst = ins.dst();
            let mut dead: Vec<Reg> = Vec::new();
            for r in ins.uses() {
                if heap[r.0 as usize] && !live[r.0 as usize] && Some(r) != dst && !dead.contains(&r)
                {
                    dead.push(r);
                }
            }
            per_instr[ii] = dead;
            if let Some(d) = dst {
                live[d.0 as usize] = false;
            }
            for r in ins.uses() {
                live[r.0 as usize] = true;
            }
        }
        out.push(per_instr);
    }
    out
}

/// A list index as an i64: integers pass through, floats must be integral.
fn index_value(
    m: &Module,
    f: &Func,
    b: &mut FunctionBuilder,
    imp: &Imports,
    reg: Reg,
    vars: &[Variable],
    msg: &mut dyn FnMut(String) -> i64,
) -> Result<Value, Unsupported> {
    let kind = match m.types.kind(f.reg_ty(reg)) {
        TyKind::Num(k) => *k,
        other => return unsupported(format!("index of type {other:?}")),
    };
    let v = b.use_var(vars[reg.0 as usize]);
    if !kind.is_float() {
        return Ok(v);
    }
    let i = b.ins().fcvt_to_sint_sat(types::I64, v);
    let back = b.ins().fcvt_from_sint(types::F64, i);
    let ok = b.ins().fcmp(FloatCC::Equal, back, v);
    let not_int = msg("list index is not an integer".into());
    check(b, imp, ok, not_int);
    Ok(i)
}
