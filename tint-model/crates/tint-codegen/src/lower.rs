//! Lowering of one typed-IR function to Cranelift.
//!
//! Every IR register is a Cranelift variable. A register of a heap type holds
//! null or exactly one owned reference; moving a value into a register
//! retains it (or hands the reference over when the source register is dead
//! afterwards), overwriting releases the old one, and `Return` hands the
//! result's reference to the caller and releases the rest.

use crate::{clif_ty, elem_layout, is_heap, is_scalar, ptr_mask, rt, unsupported, Unsupported};
use cranelift_codegen::ir::condcodes::{FloatCC, IntCC};
use cranelift_codegen::ir::{
    types, AbiParam, FuncRef, InstBuilder, MemFlagsData, Signature, StackSlotData, StackSlotKind,
    TrapCode, Type, Value,
};
use cranelift_frontend::{FunctionBuilder, Variable};
use std::collections::HashMap;
use tint_ir::typed::*;

pub(crate) type Imports = HashMap<&'static str, FuncRef>;

trait ImportExt {
    fn f(&self, name: &str) -> FuncRef;
}

impl ImportExt for Imports {
    fn f(&self, name: &str) -> FuncRef {
        self[name]
    }
}

fn slot_off(slot: usize) -> i32 {
    rt::OFF_SLOTS + 8 * slot as i32
}

fn flags() -> MemFlagsData {
    MemFlagsData::new().with_notrap()
}

/// Scalar register value -> 64-bit slot bits.
fn to_bits(b: &mut FunctionBuilder, v: Value) -> Value {
    match b.func.dfg.value_type(v) {
        t if t == types::F64 => b.ins().bitcast(types::I64, MemFlagsData::new(), v),
        t if t == types::I8 => b.ins().uextend(types::I64, v),
        _ => v,
    }
}

/// 64-bit slot bits -> register value of clif type `t`.
fn from_bits(b: &mut FunctionBuilder, bits: Value, t: Type) -> Value {
    if t == types::F64 {
        b.ins().bitcast(types::F64, MemFlagsData::new(), bits)
    } else if t == types::I8 {
        b.ins().ireduce(types::I8, bits)
    } else {
        bits
    }
}


/// The entry point closures are called through: `(closure, declared params...)`.
/// It unpacks the captured values from the closure object and calls the real
/// function, which takes them as its first parameters.
pub(crate) fn thunk_body(
    m: &Module,
    f: &Func,
    b: &mut FunctionBuilder,
    real: FuncRef,
    retain: FuncRef,
) {
    let entry = b.create_block();
    b.append_block_params_for_function_params(entry);
    b.switch_to_block(entry);
    let params = b.block_params(entry).to_vec();
    let closure = params[0];
    let mut args = Vec::new();
    for i in 0..f.ncaptures as usize {
        let ty = f.reg_ty(f.params[i]);
        let bits = b.ins().load(types::I64, flags(), closure, slot_off(1 + i));
        if is_heap(m, ty) {
            // The callee owns its parameters; the closure keeps its own reference.
            b.ins().call(retain, &[bits]);
            args.push(bits);
        } else {
            let t = clif_ty(m, ty).unwrap_or(types::I64);
            args.push(from_bits(b, bits, t));
        }
    }
    args.extend_from_slice(&params[1..]);
    let call = b.ins().call(real, &args);
    let r = b.inst_results(call)[0];
    b.ins().return_(&[r]);
}

struct L<'a, 'b> {
    m: &'a Module,
    f: &'a Func,
    b: &'a mut FunctionBuilder<'b>,
    imp: &'a Imports,
    callees: &'a [Option<FuncRef>],
    vars: Vec<Variable>,
    heap: Vec<bool>,
    msgs: &'a mut Vec<String>,
    /// Address of the table of closure entry points, indexed by function id.
    fn_table: i64,
    /// Address of the slots of the global variables.
    globals: i64,
    /// Heap registers the current instruction reads for the last time.
    dying: Vec<Reg>,
    /// Of those, the ones whose reference the instruction took over.
    moved: Vec<Reg>,
}

pub(crate) fn lower_func(
    m: &Module,
    f: &Func,
    b: &mut FunctionBuilder,
    imp: &Imports,
    callees: &[Option<FuncRef>],
    msgs: &mut Vec<String>,
    fn_table: i64,
    globals: i64,
) -> Result<(), Unsupported> {
    let mut vars = Vec::new();
    for ty in &f.regs {
        let t = clif_ty(m, *ty).unwrap_or(types::I64);
        vars.push(b.declare_var(t));
    }
    // Reject the function up front if any register has an unsupported type.
    for ty in &f.regs {
        clif_ty(m, *ty)?;
    }
    let heap: Vec<bool> = f.regs.iter().map(|t| is_heap(m, *t)).collect();
    let blocks: Vec<_> = f.blocks.iter().map(|_| b.create_block()).collect();
    b.append_block_params_for_function_params(blocks[0]);
    b.switch_to_block(blocks[0]);
    for (i, h) in heap.iter().enumerate() {
        if *h && !f.params.iter().any(|p| p.0 as usize == i) {
            let null = b.ins().iconst(types::I64, 0);
            b.def_var(vars[i], null);
        }
    }
    for (i, p) in f.params.iter().enumerate() {
        let v = b.block_params(blocks[0])[i];
        b.def_var(vars[p.0 as usize], v);
    }
    let dies = dying_regs(f, &heap);
    let mut l = L {
        m,
        f,
        b,
        imp,
        callees,
        vars,
        heap,
        msgs,
        fn_table,
        globals,
        dying: Vec::new(),
        moved: Vec::new(),
    };

    for (bi, block) in f.blocks.iter().enumerate() {
        if bi != 0 {
            l.b.switch_to_block(blocks[bi]);
        }
        let mut skip_next = false;
        let mut skip_more = 0usize;
        for (ii, ins) in block.instrs.iter().enumerate() {
            if skip_more > 0 {
                skip_more -= 1;
                continue;
            }
            if std::mem::take(&mut skip_next) {
                continue;
            }
            l.dying = dies[bi][ii].clone();
            l.moved.clear();
            // `x = list[i]; y = x.k` with `x` dead afterwards and `y` a scalar:
            // read the field in place, without taking and giving back a reference.
            let fused = match (ins, block.instrs.get(ii + 1)) {
                (
                    Instr::Get { dst: t, base, proj: Proj::Index(i) },
                    Some(Instr::Get { dst: d, base: b2, proj: Proj::Field(k) | Proj::Tuple(k) }),
                ) if b2 == t
                    && t != d
                    && l.heap[t.0 as usize]
                    && !l.heap[d.0 as usize]
                    && dies[bi][ii + 1].contains(t)
                    && !dies[bi][ii].contains(t) =>
                {
                    Some((*base, *i, *d, *k))
                }
                _ => None,
            };
            // `t = take list[i]; t.k = v; list[i] = t` for a scalar `v`: write the
            // field in place (copying the element only when it is shared).
            let write = match (ins, block.instrs.get(ii + 1), block.instrs.get(ii + 2)) {
                (
                    Instr::Take { dst: t, base, proj: Proj::Index(i) },
                    Some(Instr::Set { base: b1, proj: Proj::Field(k), src: v }),
                    Some(Instr::Set { base: b2, proj: Proj::Index(i2), src: t2 }),
                ) if b1 == t
                    && t2 == t
                    && b2 == base
                    && i2 == i
                    && v != t
                    && v != base
                    && l.heap[t.0 as usize]
                    && !l.heap[v.0 as usize]
                    && dies[bi][ii].is_empty()
                    && dies[bi][ii + 1].is_empty()
                    && dies[bi][ii + 2].iter().all(|r| r == t) =>
                {
                    Some((*base, *i, *k as usize, *v))
                }
                _ => None,
            };
            if let Some((base, i, k, v)) = write {
                l.index_field_set(base, i, k, v)?;
                skip_more = 2;
                continue;
            }
            if let Some((base, i, d, k)) = fused {
                l.index_field(base, i, d, k as usize)?;
                skip_next = true;
                for r in std::mem::take(&mut l.dying) {
                    let old = l.get(r);
                    l.release(old);
                    l.clear(r);
                }
                continue;
            }
            l.instr(ins)?;
            // A heap register whose last use this was gives its reference back now.
            for r in std::mem::take(&mut l.dying) {
                if !l.moved.contains(&r) {
                    let old = l.get(r);
                    l.release(old);
                    l.clear(r);
                }
            }
        }
        match &block.term {
            Term::Jump(t) => {
                l.b.ins().jump(blocks[t.0 as usize], &[]);
            }
            Term::Branch { cond, then_, else_ } => {
                let c = l.get(*cond);
                l.b.ins().brif(c, blocks[then_.0 as usize], &[], blocks[else_.0 as usize], &[]);
            }
            Term::Switch { value, cases, default } => {
                let v = l.get(*value);
                let mut sw = cranelift_frontend::Switch::new();
                for (k, t) in cases {
                    sw.set_entry(*k as u128, blocks[t.0 as usize]);
                }
                sw.emit(l.b, v, blocks[default.0 as usize]);
            }
            Term::Return(r) => {
                let v = l.get(*r);
                if l.heap[r.0 as usize] {
                    // The reference moves to the caller.
                    l.clear(*r);
                }
                for i in 0..l.heap.len() {
                    if l.heap[i] {
                        let old = l.b.use_var(l.vars[i]);
                        l.release(old);
                    }
                }
                l.b.ins().return_(&[v]);
            }
            Term::Trap(msg) => {
                let id = l.msg(msg);
                let c = l.b.ins().iconst(types::I64, id);
                l.call("trap", &[c]);
                l.b.ins().trap(TrapCode::unwrap_user(1));
            }
        }
    }
    Ok(())
}

impl<'a, 'b> L<'a, 'b> {
    fn get(&mut self, r: Reg) -> Value {
        self.b.use_var(self.vars[r.0 as usize])
    }

    fn set(&mut self, r: Reg, v: Value) {
        self.b.def_var(self.vars[r.0 as usize], v);
    }

    /// Makes a heap register empty (its reference was handed on).
    fn clear(&mut self, r: Reg) {
        let null = self.b.ins().iconst(types::I64, 0);
        self.set(r, null);
    }

    /// `rc += 1`, inline.
    fn retain(&mut self, p: Value) {
        let (inc, done) = (self.b.create_block(), self.b.create_block());
        self.b.ins().brif(p, inc, &[], done, &[]);
        self.b.switch_to_block(inc);
        let rc = self.b.ins().load(types::I64, flags(), p, rt::OFF_RC);
        let next = self.b.ins().iadd_imm(rc, 1);
        self.b.ins().store(flags(), next, p, rt::OFF_RC);
        self.b.ins().jump(done, &[]);
        self.b.switch_to_block(done);
    }

    /// `rc -= 1`, inline; the runtime frees the object when it was the last reference.
    fn release(&mut self, p: Value) {
        let (check, last, dec, done) = (
            self.b.create_block(),
            self.b.create_block(),
            self.b.create_block(),
            self.b.create_block(),
        );
        self.b.set_cold_block(last);
        self.b.ins().brif(p, check, &[], done, &[]);
        self.b.switch_to_block(check);
        let rc = self.b.ins().load(types::I64, flags(), p, rt::OFF_RC);
        let shared = self.b.ins().icmp_imm(IntCC::UnsignedGreaterThan, rc, 1);
        self.b.ins().brif(shared, dec, &[], last, &[]);
        self.b.switch_to_block(dec);
        let next = self.b.ins().iadd_imm(rc, -1);
        self.b.ins().store(flags(), next, p, rt::OFF_RC);
        self.b.ins().jump(done, &[]);
        self.b.switch_to_block(last);
        let f = self.imp.f("release");
        self.b.ins().call(f, &[p]);
        self.b.ins().jump(done, &[]);
        self.b.switch_to_block(done);
    }

    fn call(&mut self, name: &str, args: &[Value]) -> Vec<Value> {
        let fr = self.imp.f(name);
        let c = self.b.ins().call(fr, args);
        self.b.inst_results(c).to_vec()
    }

    fn call1(&mut self, name: &str, args: &[Value]) -> Value {
        self.call(name, args)[0]
    }

    fn msg(&mut self, s: &str) -> i64 {
        if let Some(i) = self.msgs.iter().position(|x| x == s) {
            return i as i64;
        }
        self.msgs.push(s.to_string());
        (self.msgs.len() - 1) as i64
    }

    fn iconst(&mut self, n: i64) -> Value {
        self.b.ins().iconst(types::I64, n)
    }

    fn clif(&self, r: Reg) -> Result<Type, Unsupported> {
        clif_ty(self.m, self.f.reg_ty(r))
    }

    fn ty(&self, r: Reg) -> TyId {
        self.f.reg_ty(r)
    }

    /// Stores a freshly owned heap value into `dst`, dropping what it held.
    fn assign(&mut self, dst: Reg, v: Value) {
        let old = self.get(dst);
        self.release(old);
        self.set(dst, v);
    }

    /// Writes a result that is either scalar bits or an owned pointer.
    fn put(&mut self, dst: Reg, bits: Value) -> Result<(), Unsupported> {
        let t = self.clif(dst)?;
        if self.heap[dst.0 as usize] {
            self.assign(dst, bits);
        } else {
            let v = from_bits(self.b, bits, t);
            self.set(dst, v);
        }
        Ok(())
    }

    /// The value of `reg` with one owned reference to give away. `rest` are
    /// the operands of the same instruction that come after this one.
    fn own(&mut self, reg: Reg, rest: &[Reg]) -> Value {
        let v = self.get(reg);
        if self.heap[reg.0 as usize] {
            if self.dying.contains(&reg) && !rest.contains(&reg) {
                self.moved.push(reg);
                self.clear(reg);
            } else {
                self.retain(v);
            }
        }
        v
    }

    /// `p` if the object is unshared, else a private copy; stored back into `reg`.
    fn unique(&mut self, reg: Reg, copy: &str) -> Value {
        let p = self.get(reg);
        let rc = self.b.ins().load(types::I64, flags(), p, rt::OFF_RC);
        let one = self.b.ins().icmp_imm(IntCC::Equal, rc, 1);
        let (fast, slow, done) = (self.b.create_block(), self.b.create_block(), self.b.create_block());
        self.b.set_cold_block(slow);
        self.b.append_block_param(done, types::I64);
        self.b.ins().brif(one, fast, &[], slow, &[]);
        self.b.switch_to_block(fast);
        self.b.ins().jump(done, &[p.into()]);
        self.b.switch_to_block(slow);
        let copied = self.call1(copy, &[p]);
        self.b.ins().jump(done, &[copied.into()]);
        self.b.switch_to_block(done);
        let u = self.b.block_params(done)[0];
        self.set(reg, u);
        u
    }

    fn load_slot(&mut self, obj: Value, slot: usize, t: Type) -> Value {
        let bits = self.b.ins().load(types::I64, flags(), obj, slot_off(slot));
        from_bits(self.b, bits, t)
    }

    fn store_slot(&mut self, obj: Value, slot: usize, v: Value) {
        let bits = to_bits(self.b, v);
        self.b.ins().store(flags(), bits, obj, slot_off(slot));
    }

    /// A stack array holding `vals`, returning its address.
    fn stack_array(&mut self, vals: &[Value]) -> Value {
        let size = (8 * vals.len()).max(8) as u32;
        let slot = self.b.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, size, 3));
        for (i, v) in vals.iter().enumerate() {
            self.b.ins().stack_store(types::I64, *v, slot, 8 * i as i32);
        }
        self.b.ins().stack_addr(types::I64, slot, 0)
    }

    /// Bounds-checks `idx` against the list and returns the address of the element.
    fn elem_addr(&mut self, list: Value, idx: Value, size: i64, check: bool) -> Value {
        if check {
            let len = self.b.ins().load(types::I64, flags(), list, rt::OFF_LEN);
            let in_range = self.b.ins().icmp(IntCC::UnsignedLessThan, idx, len);
            let (ok, bad) = (self.b.create_block(), self.b.create_block());
            self.b.set_cold_block(bad);
            self.b.ins().brif(in_range, ok, &[], bad, &[]);
            self.b.switch_to_block(bad);
            self.call("list_oob", &[list, idx]);
            self.b.ins().trap(TrapCode::unwrap_user(1));
            self.b.switch_to_block(ok);
        }
        let data = self.b.ins().load(types::I64, flags(), list, rt::OFF_PTR);
        let off = if size == 1 { idx } else { self.b.ins().imul_imm(idx, size) };
        self.b.ins().iadd(data, off)
    }

    fn load_elem(&mut self, addr: Value, lay: crate::ElemLayout) -> Value {
        let v = self.b.ins().load(lay.mem, flags(), addr, 0);
        if lay.mem == lay.reg {
            v
        } else if lay.signed {
            self.b.ins().sextend(lay.reg, v)
        } else {
            self.b.ins().uextend(lay.reg, v)
        }
    }

    fn store_elem(&mut self, addr: Value, v: Value, lay: crate::ElemLayout) {
        let v = if lay.mem == lay.reg { v } else { self.b.ins().ireduce(lay.mem, v) };
        self.b.ins().store(flags(), v, addr, 0);
    }

    fn list_elem(&self, list: Reg) -> Result<crate::ElemLayout, Unsupported> {
        match self.m.types.kind(self.ty(list)) {
            TyKind::List(e) => Ok(elem_layout(self.m, *e)),
            other => unsupported(format!("list operation on {other:?}")),
        }
    }

    fn index(&mut self, reg: Reg) -> Result<Value, Unsupported> {
        let (m, f) = (self.m, self.f);
        let vars = self.vars.clone();
        let imp = self.imp;
        let msgs = &mut *self.msgs;
        let mut intern = |s: String| -> i64 {
            if let Some(i) = msgs.iter().position(|x| *x == s) {
                return i as i64;
            }
            msgs.push(s);
            (msgs.len() - 1) as i64
        };
        index_value(m, f, self.b, imp, reg, &vars, &mut intern)
    }

    /// The stored bits of a scalar element/slot value, or the owned pointer
    /// of a heap one (taking `src`'s reference).
    fn slot_bits(&mut self, src: Reg, rest: &[Reg]) -> Value {
        if self.heap[src.0 as usize] {
            self.own(src, rest)
        } else {
            let v = self.get(src);
            to_bits(self.b, v)
        }
    }

    fn instr(&mut self, ins: &Instr) -> Result<(), Unsupported> {
        match ins {
            Instr::Const { dst, value } => {
                self.clif(*dst)?;
                let v = match value {
                    Const::Unit => self.b.ins().iconst(types::I8, 0),
                    Const::Bool(x) => self.b.ins().iconst(types::I8, *x as i64),
                    Const::Int(i) => {
                        if matches!(self.m.types.kind(self.ty(*dst)), TyKind::Num(k) if k.is_float()) {
                            self.b.ins().f64const(*i as f64)
                        } else {
                            self.b.ins().iconst(types::I64, *i)
                        }
                    }
                    Const::Float(x) => self.b.ins().f64const(*x),
                    Const::Str(s) => {
                        let p = rt::immortal_str(s);
                        let v = self.iconst(p as i64);
                        self.assign(*dst, v);
                        return Ok(());
                    }
                };
                self.set(*dst, v);
            }
            Instr::Mov { dst, src } => {
                self.clif(*dst)?;
                if self.heap[dst.0 as usize] {
                    let v = self.own(*src, &[]);
                    self.assign(*dst, v);
                } else {
                    let v = self.get(*src);
                    self.set(*dst, v);
                }
            }
            Instr::Bin { dst, op, kind, a, b: rb } => {
                let x = self.get(*a);
                let y = self.get(*rb);
                let imp = self.imp;
                let msgs = &mut *self.msgs;
                let mut intern = |s: String| -> i64 {
                    if let Some(i) = msgs.iter().position(|v| *v == s) {
                        return i as i64;
                    }
                    msgs.push(s);
                    (msgs.len() - 1) as i64
                };
                let r = arith(self.b, imp, *op, *kind, x, y, &mut intern);
                self.set(*dst, r);
            }
            Instr::Cmp { dst, op, a, b: rb } => self.cmp(*dst, *op, *a, *rb)?,
            Instr::Not { dst, src } => {
                let x = self.get(*src);
                let r = self.b.ins().bxor_imm(x, 1);
                self.set(*dst, r);
            }
            Instr::Neg { dst, src } => {
                let k = match self.m.types.kind(self.ty(*src)) {
                    TyKind::Num(k) => *k,
                    _ => return unsupported("neg"),
                };
                let x = self.get(*src);
                let r = if k.is_float() {
                    self.b.ins().fneg(x)
                } else {
                    let zero = self.iconst(0);
                    let imp = self.imp;
                    let msgs = &mut *self.msgs;
                    let mut intern = |s: String| -> i64 {
                        if let Some(i) = msgs.iter().position(|v| *v == s) {
                            return i as i64;
                        }
                        msgs.push(s);
                        (msgs.len() - 1) as i64
                    };
                    arith(self.b, imp, BinOp::Sub, k, zero, x, &mut intern)
                };
                self.set(*dst, r);
            }
            Instr::Cast { dst, src } => {
                let (from, to) = match (self.m.types.kind(self.ty(*src)), self.m.types.kind(self.ty(*dst))) {
                    (TyKind::Num(a), TyKind::Num(b)) => (*a, *b),
                    _ => return unsupported("cast of a non-number"),
                };
                let x = self.get(*src);
                let bits = to_bits(self.b, x);
                let (f, t) = (rt::num_kind_index(from), rt::num_kind_index(to));
                let (f, t) = (self.b.ins().iconst(types::I32, f), self.b.ins().iconst(types::I32, t));
                let out = self.call1("cast", &[f, t, bits]);
                self.put(*dst, out)?;
            }
            Instr::ToStr { dst, src } => {
                self.clif(*src)?;
                if matches!(self.m.types.kind(self.ty(*src)), TyKind::Str) {
                    let v = self.own(*src, &[]);
                    self.assign(*dst, v);
                } else {
                    let x = self.get(*src);
                    let bits = if self.heap[src.0 as usize] { x } else { to_bits(self.b, x) };
                    let tid = self.ty(*src).0 as i64;
                    let ty = self.b.ins().iconst(types::I32, tid);
                    let s = self.call1("to_str", &[ty, bits]);
                    self.assign(*dst, s);
                }
            }
            Instr::Tuple { dst, items } => {
                let tys: Vec<TyId> = items.iter().map(|r| self.ty(*r)).collect();
                self.build_obj(*dst, items, tys.len(), ptr_mask(self.m, &tys, 0), 0, None)?;
            }
            Instr::Struct { dst, adt, fields } => {
                let tys: Vec<TyId> = fields.iter().map(|r| self.ty(*r)).collect();
                let n = crate::adt_layout_slots(self.m, *adt);
                self.build_obj(*dst, fields, n, ptr_mask(self.m, &tys, 0), 0, None)?;
            }
            Instr::Variant { dst, adt, variant, fields } if fields.is_empty() => {
                // A variant without fields is one shared object that is never freed.
                self.clif(*dst)?;
                let n = crate::adt_layout_slots(self.m, *adt);
                let p = rt::immortal_variant(n, *variant as u64);
                let v = self.iconst(p as i64);
                self.assign(*dst, v);
            }
            Instr::Variant { dst, adt, variant, fields } => {
                let tys: Vec<TyId> = fields.iter().map(|r| self.ty(*r)).collect();
                let n = crate::adt_layout_slots(self.m, *adt);
                self.build_obj(*dst, fields, n, ptr_mask(self.m, &tys, 1), 1, Some(*variant as i64))?;
            }
            Instr::List { dst, items } => {
                self.clif(*dst)?;
                let lay = match self.m.types.kind(self.ty(*dst)) {
                    TyKind::List(e) => elem_layout(self.m, *e),
                    _ => return unsupported("list literal of a non-list"),
                };
                let n = self.iconst(items.len() as i64);
                let size = self.iconst(lay.size);
                let heap = self.iconst(lay.heap as i64);
                let list = self.call1("list_new", &[n, size, heap]);
                for (i, item) in items.iter().enumerate() {
                    let bits = self.slot_bits(*item, &items[i + 1..]);
                    self.call("list_push", &[list, bits]);
                }
                self.assign(*dst, list);
            }
            Instr::Get { dst, base, proj } => self.get_proj(*dst, *base, proj, false)?,
            Instr::Take { dst, base, proj } => self.get_proj(*dst, *base, proj, true)?,
            Instr::Set { base, proj, src } => self.set_proj(*base, proj, *src)?,
            Instr::Tag { dst, src } => {
                let p = self.get(*src);
                let v = self.b.ins().load(types::I64, flags(), p, slot_off(0));
                self.set(*dst, v);
            }
            Instr::Payload { dst, src, index, .. } => {
                let t = self.clif(*dst)?;
                let p = self.get(*src);
                let v = self.load_slot(p, 1 + *index as usize, t);
                if self.heap[dst.0 as usize] {
                    self.retain(v);
                    self.assign(*dst, v);
                } else {
                    self.set(*dst, v);
                }
            }
            Instr::Rt { dst, f, args } => self.rt(*dst, *f, args)?,
            Instr::Call { dst, func, args } => {
                let Some(callee) = self.callees[func.0 as usize] else {
                    return unsupported(format!("call to `{}`", self.m.func(*func).name));
                };
                let mut vals = Vec::new();
                for (k, a) in args.iter().enumerate() {
                    // The callee owns its parameters: hand it one reference each.
                    vals.push(self.own(*a, &args[k + 1..]));
                }
                let call = self.b.ins().call(callee, &vals);
                let r = self.b.inst_results(call)[0];
                if self.heap[dst.0 as usize] {
                    self.assign(*dst, r);
                } else {
                    self.set(*dst, r);
                }
            }
            Instr::Host { dst, f: f @ (HostFn::Print | HostFn::Println), args } => {
                for a in args {
                    self.clif(*a)?;
                    let v = self.get(*a);
                    let bits = if self.heap[a.0 as usize] { v } else { to_bits(self.b, v) };
                    let tid = self.ty(*a).0 as i64;
                    let ty = self.iconst(tid);
                    let nl = self.iconst(i64::from(*f == HostFn::Println));
                    self.call("print", &[ty, bits, nl]);
                }
                let unit = self.b.ins().iconst(types::I8, 0);
                self.set(*dst, unit);
            }
            Instr::Closure { dst, func, captures } => {
                let tys: Vec<TyId> = captures.iter().map(|r| self.ty(*r)).collect();
                let n = 1 + captures.len();
                self.build_obj(*dst, captures, n, ptr_mask(self.m, &tys, 1), 1, Some(func.0 as i64))?;
            }
            Instr::CallClosure { dst, callee, args } => {
                let ret = self.clif(*dst)?;
                let mut sig = Signature::new(self.b.func.signature.call_conv);
                sig.params.push(AbiParam::new(types::I64));
                for a in args {
                    sig.params.push(AbiParam::new(self.clif(*a)?));
                }
                sig.returns.push(AbiParam::new(ret));
                let sig = self.b.func.import_signature(sig);
                let closure = self.get(*callee);
                let id = self.b.ins().load(types::I64, flags(), closure, slot_off(0));
                let table = self.iconst(self.fn_table);
                let off = self.b.ins().imul_imm(id, 8);
                let at = self.b.ins().iadd(table, off);
                let code = self.b.ins().load(types::I64, flags(), at, 0);
                let mut vals = vec![closure];
                for (k, a) in args.iter().enumerate() {
                    vals.push(self.own(*a, &args[k + 1..]));
                }
                let call = self.b.ins().call_indirect(sig, code, &vals);
                let r = self.b.inst_results(call)[0];
                if self.heap[dst.0 as usize] {
                    self.assign(*dst, r);
                } else {
                    self.set(*dst, r);
                }
            }
            Instr::GlobalGet { dst, global } => {
                let t = self.clif(*dst)?;
                let at = self.iconst(self.globals + 8 * global.0 as i64);
                let bits = self.b.ins().load(types::I64, flags(), at, 0);
                if self.heap[dst.0 as usize] {
                    self.retain(bits);
                    self.assign(*dst, bits);
                } else {
                    let v = from_bits(self.b, bits, t);
                    self.set(*dst, v);
                }
            }
            Instr::GlobalSet { global, src } => {
                self.clif(*src)?;
                let at = self.iconst(self.globals + 8 * global.0 as i64);
                if self.heap[src.0 as usize] {
                    let old = self.b.ins().load(types::I64, flags(), at, 0);
                    let v = self.own(*src, &[]);
                    self.b.ins().store(flags(), v, at, 0);
                    self.release(old);
                } else {
                    let v = self.get(*src);
                    let bits = to_bits(self.b, v);
                    self.b.ins().store(flags(), bits, at, 0);
                }
            }
            Instr::Map { dst, entries } => {
                self.clif(*dst)?;
                let heap = match self.m.types.kind(self.ty(*dst)) {
                    TyKind::Map(v) => is_heap(self.m, *v),
                    _ => return unsupported("map literal of a non-map"),
                };
                let flag = self.iconst(heap as i64);
                let mut map = self.call1("map_new", &[flag]);
                for (i, (key, reg)) in entries.iter().enumerate() {
                    let k = self.iconst(rt::immortal_str(key) as i64);
                    let rest: Vec<Reg> = entries[i + 1..].iter().map(|(_, r)| *r).collect();
                    let bits = self.slot_bits(*reg, &rest);
                    map = self.call1("map_set", &[map, k, bits]);
                }
                self.assign(*dst, map);
            }
            other => return unsupported(format!("{other:?}").chars().take(60).collect::<String>()),
        }
        Ok(())
    }

    /// Allocates a struct, tuple or enum object and fills its slots.
    fn build_obj(
        &mut self,
        dst: Reg,
        fields: &[Reg],
        nslots: usize,
        mask: u64,
        base: usize,
        tag: Option<i64>,
    ) -> Result<(), Unsupported> {
        self.clif(dst)?;
        let n = self.iconst(nslots as i64);
        let mk = self.iconst(mask as i64);
        let obj = self.call1("obj_new", &[n, mk]);
        if let Some(tag) = tag {
            let t = self.iconst(tag);
            self.b.ins().store(flags(), t, obj, slot_off(0));
        }
        for (i, f) in fields.iter().enumerate() {
            self.clif(*f)?;
            let bits = self.slot_bits(*f, &fields[i + 1..]);
            self.b.ins().store(flags(), bits, obj, slot_off(base + i));
        }
        self.assign(dst, obj);
        Ok(())
    }

    /// `base[i].slot = v` for a scalar `v`, in place.
    fn index_field_set(&mut self, base: Reg, i: Reg, slot: usize, v: Reg) -> Result<(), Unsupported> {
        self.clif(v)?;
        let lay = self.list_elem(base)?;
        let idx = self.index(i)?;
        let list = self.unique(base, "list_unique");
        let addr = self.elem_addr(list, idx, lay.size, true);
        let obj = self.load_elem(addr, lay);
        let rc = self.b.ins().load(types::I64, flags(), obj, rt::OFF_RC);
        let one = self.b.ins().icmp_imm(IntCC::Equal, rc, 1);
        let (fast, slow, done) = (self.b.create_block(), self.b.create_block(), self.b.create_block());
        self.b.set_cold_block(slow);
        self.b.append_block_param(done, types::I64);
        self.b.ins().brif(one, fast, &[], slow, &[]);
        self.b.switch_to_block(fast);
        self.b.ins().jump(done, &[obj.into()]);
        self.b.switch_to_block(slow);
        let copy = self.call1("obj_unique", &[obj]);
        self.b.ins().store(flags(), copy, addr, 0);
        self.b.ins().jump(done, &[copy.into()]);
        self.b.switch_to_block(done);
        let obj = self.b.block_params(done)[0];
        let val = self.get(v);
        self.store_slot(obj, slot, val);
        Ok(())
    }

    /// `dst = base[i].slot` for a scalar field, reading the element in place.
    fn index_field(&mut self, base: Reg, i: Reg, dst: Reg, slot: usize) -> Result<(), Unsupported> {
        let t = self.clif(dst)?;
        let lay = self.list_elem(base)?;
        let idx = self.index(i)?;
        let list = self.get(base);
        let addr = self.elem_addr(list, idx, lay.size, true);
        let obj = self.load_elem(addr, lay);
        let v = self.load_slot(obj, slot, t);
        self.set(dst, v);
        Ok(())
    }

    fn get_proj(&mut self, dst: Reg, base: Reg, proj: &Proj, take: bool) -> Result<(), Unsupported> {
        let t = self.clif(dst)?;
        match proj {
            Proj::Field(i) | Proj::Tuple(i) => {
                let obj = if take { self.unique(base, "obj_unique") } else { self.get(base) };
                let slot = *i as usize;
                let v = self.load_slot(obj, slot, t);
                if self.heap[dst.0 as usize] {
                    if take {
                        // The slot gives its reference away and stays empty.
                        let null = self.iconst(0);
                        self.b.ins().store(flags(), null, obj, slot_off(slot));
                    } else {
                        self.retain(v);
                    }
                    self.assign(dst, v);
                } else {
                    self.set(dst, v);
                }
            }
            Proj::Index(i) => {
                let lay = self.list_elem(base)?;
                let idx = self.index(*i)?;
                if take && lay.heap {
                    let list = self.unique(base, "list_unique");
                    let v = self.call1("list_take", &[list, idx]);
                    self.assign(dst, v);
                } else {
                    let list = self.get(base);
                    let addr = self.elem_addr(list, idx, lay.size, true);
                    let v = self.load_elem(addr, lay);
                    if lay.heap {
                        self.retain(v);
                        self.assign(dst, v);
                    } else {
                        self.set(dst, v);
                    }
                }
            }
            Proj::Key(k) => {
                let key = self.get(*k);
                let map = if take { self.unique(base, "map_unique") } else { self.get(base) };
                let op = if take { "map_take" } else { "map_get" };
                let bits = self.call1(op, &[map, key]);
                if self.heap[dst.0 as usize] {
                    if !take {
                        self.retain(bits);
                    }
                    self.assign(dst, bits);
                } else {
                    let v = from_bits(self.b, bits, t);
                    self.set(dst, v);
                }
            }
        }
        Ok(())
    }

    fn set_proj(&mut self, base: Reg, proj: &Proj, src: Reg) -> Result<(), Unsupported> {
        self.clif(src)?;
        match proj {
            Proj::Field(i) | Proj::Tuple(i) => {
                let obj = self.unique(base, "obj_unique");
                let slot = *i as usize;
                if self.heap[src.0 as usize] {
                    let old = self.b.ins().load(types::I64, flags(), obj, slot_off(slot));
                    let v = self.own(src, &[]);
                    self.b.ins().store(flags(), v, obj, slot_off(slot));
                    self.release(old);
                } else {
                    let v = self.get(src);
                    self.store_slot(obj, slot, v);
                }
            }
            Proj::Index(i) => {
                let lay = self.list_elem(base)?;
                let idx = self.index(*i)?;
                let list = self.get(base);
                if lay.heap {
                    // The runtime copies a shared list and releases the old element.
                    let v = self.own(src, &[]);
                    let new = self.call1("list_set", &[list, idx, v]);
                    self.set(base, new);
                    return Ok(());
                }
                let v = self.get(src);
                // Fast path: unshared and in range. Otherwise the runtime
                // copies the list or reports the bad index.
                let len = self.b.ins().load(types::I64, flags(), list, rt::OFF_LEN);
                let rc = self.b.ins().load(types::I64, flags(), list, rt::OFF_RC);
                let in_range = self.b.ins().icmp(IntCC::UnsignedLessThan, idx, len);
                let unshared = self.b.ins().icmp_imm(IntCC::Equal, rc, 1);
                let fast_ok = self.b.ins().band(in_range, unshared);
                let (fast, slow, done) =
                    (self.b.create_block(), self.b.create_block(), self.b.create_block());
                self.b.set_cold_block(slow);
                self.b.append_block_param(done, types::I64);
                self.b.ins().brif(fast_ok, fast, &[], slow, &[]);
                self.b.switch_to_block(fast);
                let addr = self.elem_addr(list, idx, lay.size, false);
                self.store_elem(addr, v, lay);
                self.b.ins().jump(done, &[list.into()]);
                self.b.switch_to_block(slow);
                let bits = to_bits(self.b, v);
                let new = self.call1("list_set", &[list, idx, bits]);
                self.b.ins().jump(done, &[new.into()]);
                self.b.switch_to_block(done);
                let list = self.b.block_params(done)[0];
                self.set(base, list);
            }
            Proj::Key(k) => {
                let key = self.get(*k);
                let map = self.get(base);
                let bits = self.slot_bits(src, &[]);
                let new = self.call1("map_set", &[map, key, bits]);
                self.set(base, new);
            }
        }
        Ok(())
    }

    fn cmp(&mut self, dst: Reg, op: CmpOp, a: Reg, rb: Reg) -> Result<(), Unsupported> {
        let ty = self.ty(a);
        let x = self.get(a);
        let y = self.get(rb);
        let kind = match self.m.types.kind(ty) {
            TyKind::Num(k) => Some(*k),
            _ => None,
        };
        let int_cc = |op: CmpOp, unsigned: bool| match (op, unsigned) {
            (CmpOp::Eq, _) => IntCC::Equal,
            (CmpOp::Ne, _) => IntCC::NotEqual,
            (CmpOp::Lt, false) => IntCC::SignedLessThan,
            (CmpOp::Le, false) => IntCC::SignedLessThanOrEqual,
            (CmpOp::Gt, false) => IntCC::SignedGreaterThan,
            (CmpOp::Ge, false) => IntCC::SignedGreaterThanOrEqual,
            (CmpOp::Lt, true) => IntCC::UnsignedLessThan,
            (CmpOp::Le, true) => IntCC::UnsignedLessThanOrEqual,
            (CmpOp::Gt, true) => IntCC::UnsignedGreaterThan,
            (CmpOp::Ge, true) => IntCC::UnsignedGreaterThanOrEqual,
        };
        let r = if !is_scalar(self.m, ty) {
            self.clif(a)?;
            if matches!(self.m.types.kind(ty), TyKind::Str) {
                let c = self.call1("str_cmp", &[x, y]);
                self.b.ins().icmp_imm(int_cc(op, false), c, 0)
            } else {
                let ops = [CmpOp::Eq, CmpOp::Ne, CmpOp::Lt, CmpOp::Le, CmpOp::Gt, CmpOp::Ge];
                let code = ops.iter().position(|o| *o == op).unwrap() as i64;
                let (t, o) = (
                    self.b.ins().iconst(types::I32, ty.0 as i64),
                    self.b.ins().iconst(types::I32, code),
                );
                self.call1("cmp", &[t, o, x, y])
            }
        } else {
            match kind {
                Some(k) if k.is_float() => {
                    let cc = match op {
                        CmpOp::Eq => FloatCC::Equal,
                        CmpOp::Ne => FloatCC::NotEqual,
                        CmpOp::Lt => FloatCC::LessThan,
                        CmpOp::Le => FloatCC::LessThanOrEqual,
                        CmpOp::Gt => FloatCC::GreaterThan,
                        CmpOp::Ge => FloatCC::GreaterThanOrEqual,
                    };
                    self.b.ins().fcmp(cc, x, y)
                }
                other => {
                    let unsigned = matches!(other, Some(NumKind::U64));
                    self.b.ins().icmp(int_cc(op, unsigned), x, y)
                }
            }
        };
        self.set(dst, r);
        Ok(())
    }

    fn rt(&mut self, dst: Reg, f: RtFn, args: &[Reg]) -> Result<(), Unsupported> {
        match f {
            RtFn::ListLen => {
                let t = self.clif(dst)?;
                let list = self.get(args[0]);
                let n = self.b.ins().load(types::I64, flags(), list, rt::OFF_LEN);
                let v = if t == types::F64 { self.b.ins().fcvt_from_sint(types::F64, n) } else { n };
                self.set(dst, v);
            }
            RtFn::ListPush => {
                let (list_reg, item) = (args[0], args[1]);
                let lay = self.list_elem(list_reg)?;
                let list = self.get(list_reg);
                let bits = self.slot_bits(item, &[]);
                let len = self.b.ins().load(types::I64, flags(), list, rt::OFF_LEN);
                let cap = self.b.ins().load(types::I64, flags(), list, rt::OFF_CAP);
                let rc = self.b.ins().load(types::I64, flags(), list, rt::OFF_RC);
                let room = self.b.ins().icmp(IntCC::UnsignedLessThan, len, cap);
                let unshared = self.b.ins().icmp_imm(IntCC::Equal, rc, 1);
                let fast_ok = self.b.ins().band(room, unshared);
                let (fast, slow, done) =
                    (self.b.create_block(), self.b.create_block(), self.b.create_block());
                self.b.set_cold_block(slow);
                self.b.append_block_param(done, types::I64);
                self.b.ins().brif(fast_ok, fast, &[], slow, &[]);
                self.b.switch_to_block(fast);
                let addr = self.elem_addr(list, len, lay.size, false);
                if lay.heap {
                    self.b.ins().store(flags(), bits, addr, 0);
                } else {
                    let v = self.get(item);
                    self.store_elem(addr, v, lay);
                }
                let next = self.b.ins().iadd_imm(len, 1);
                self.b.ins().store(flags(), next, list, rt::OFF_LEN);
                self.b.ins().jump(done, &[list.into()]);
                self.b.switch_to_block(slow);
                let new = self.call1("list_push", &[list, bits]);
                self.b.ins().jump(done, &[new.into()]);
                self.b.switch_to_block(done);
                let list = self.b.block_params(done)[0];
                self.set(list_reg, list);
                let unit = self.b.ins().iconst(types::I8, 0);
                self.set(dst, unit);
            }
            RtFn::MapLen | RtFn::MapIsEmpty => {
                let t = self.clif(dst)?;
                let map = self.get(args[0]);
                let n = self.call1("map_len", &[map]);
                let v = if f == RtFn::MapLen {
                    if t == types::F64 { self.b.ins().fcvt_from_sint(types::F64, n) } else { n }
                } else {
                    self.b.ins().icmp_imm(IntCC::Equal, n, 0)
                };
                self.set(dst, v);
            }
            RtFn::MapGet | RtFn::ListPop => {
                self.clif(dst)?;
                let (some_tag, none_tag, nslots, payload) = self.option_layout(dst)?;
                let mask = if is_heap(self.m, payload) && f == RtFn::MapGet { 1 << 1 } else { 0 };
                let mask = if f == RtFn::ListPop && is_heap(self.m, payload) { 1 << 1 } else { mask };
                let signed = matches!(self.m.types.kind(payload), TyKind::Num(NumKind::I32));
                let layout: &'static [i64; 5] =
                    Box::leak(Box::new([some_tag, none_tag, nslots, mask, signed as i64]));
                let lp = self.iconst(layout.as_ptr() as i64);
                let opt = if f == RtFn::MapGet {
                    let (map, key) = (self.get(args[0]), self.get(args[1]));
                    self.call1("map_get_opt", &[map, key, lp])
                } else {
                    let list = self.unique(args[0], "list_unique");
                    self.call1("list_pop_opt", &[list, lp])
                };
                self.assign(dst, opt);
            }
            RtFn::MapHas => {
                let (map, key) = (self.get(args[0]), self.get(args[1]));
                let v = self.call1("map_has", &[map, key]);
                self.set(dst, v);
            }
            RtFn::MapSet => {
                let (map_reg, key, val) = (args[0], args[1], args[2]);
                let map = self.get(map_reg);
                let key = self.get(key);
                let bits = self.slot_bits(val, &[]);
                let new = self.call1("map_set", &[map, key, bits]);
                self.set(map_reg, new);
                let unit = self.b.ins().iconst(types::I8, 0);
                self.set(dst, unit);
            }
            RtFn::StrConcat => {
                let ptrs: Vec<Value> = args.iter().map(|a| self.get(*a)).collect();
                let arr = self.stack_array(&ptrs);
                let n = self.iconst(args.len() as i64);
                let s = self.call1("str_concat", &[n, arr]);
                self.assign(dst, s);
            }
            RtFn::Sqrt | RtFn::Abs | RtFn::Sign | RtFn::Min | RtFn::Max | RtFn::Clamp
                if self.clif(dst)? == types::F64 && args.iter().all(|a| self.f.regs[a.0 as usize] == self.f.regs[dst.0 as usize]) =>
            {
                let a: Vec<Value> = args.iter().map(|r| self.get(*r)).collect();
                let v = match f {
                    RtFn::Sqrt => self.b.ins().sqrt(a[0]),
                    RtFn::Abs => self.b.ins().fabs(a[0]),
                    RtFn::Sign => {
                        let zero = self.b.ins().f64const(0.0);
                        let one = self.b.ins().f64const(1.0);
                        let neg = self.b.ins().f64const(-1.0);
                        let gt = self.b.ins().fcmp(FloatCC::GreaterThan, a[0], zero);
                        let lt = self.b.ins().fcmp(FloatCC::LessThan, a[0], zero);
                        let r = self.b.ins().select(lt, neg, zero);
                        self.b.ins().select(gt, one, r)
                    }
                    RtFn::Min => self.f64_min(a[0], a[1]),
                    RtFn::Max => self.f64_max(a[0], a[1]),
                    _ => {
                        let lo = self.f64_max(a[0], a[1]);
                        self.f64_min(lo, a[2])
                    }
                };
                self.set(dst, v);
            }
            RtFn::StrLen => {
                let t = self.clif(dst)?;
                let s = self.get(args[0]);
                let n = self.call1("str_len", &[s]);
                let v = if t == types::F64 { self.b.ins().fcvt_from_sint(types::F64, n) } else { n };
                self.set(dst, v);
            }
            _ => self.bridge(dst, f, args)?,
        }
        Ok(())
    }

    /// Rust's `f64::min`: a NaN operand is ignored.
    fn f64_min(&mut self, a: Value, b: Value) -> Value {
        let lt = self.b.ins().fcmp(FloatCC::LessThan, a, b);
        let t = self.b.ins().select(lt, a, b);
        let bn = self.b.ins().fcmp(FloatCC::Unordered, b, b);
        self.b.ins().select(bn, a, t)
    }

    fn f64_max(&mut self, a: Value, b: Value) -> Value {
        let gt = self.b.ins().fcmp(FloatCC::GreaterThan, a, b);
        let t = self.b.ins().select(gt, a, b);
        let bn = self.b.ins().fcmp(FloatCC::Unordered, b, b);
        self.b.ins().select(bn, a, t)
    }

    /// `(Some tag, None tag, object slots, payload type)` of the `Option`
    /// type of register `dst`.
    fn option_layout(&self, dst: Reg) -> Result<(i64, i64, i64, TyId), Unsupported> {
        let Some(adt) = self.m.types.as_adt(self.ty(dst)) else {
            return unsupported("Option result of a runtime call is not an enum");
        };
        let def = self.m.types.adt(adt);
        let (Some(some), Some(none)) = (def.variant_index("Some"), def.variant_index("None")) else {
            return unsupported("Option without Some/None");
        };
        let payload = match &def.body {
            AdtBody::Enum(variants) => variants[some as usize].fields[0].ty,
            _ => return unsupported("Option is not an enum"),
        };
        let nslots = crate::adt_layout_slots(self.m, adt) as i64;
        Ok((some as i64, none as i64, nslots, payload))
    }

    /// Runs the call on the reference interpreter (see `rt::rt_call`).
    fn bridge(&mut self, dst: Reg, f: RtFn, args: &[Reg]) -> Result<(), Unsupported> {
        for a in args {
            self.clif(*a)?;
        }
        self.clif(dst)?;
        let tys: Vec<u32> = args.iter().map(|a| self.ty(*a).0).collect();
        let tys: &'static [u32] = Box::leak(tys.into_boxed_slice());
        let mut bits = Vec::new();
        for a in args {
            let v = self.get(*a);
            bits.push(if self.heap[a.0 as usize] { v } else { to_bits(self.b, v) });
        }
        let arr = self.stack_array(&bits);
        let fi = self.iconst(rt::rt_fn_index(f));
        let n = self.iconst(args.len() as i64);
        let tp = self.iconst(tys.as_ptr() as i64);
        let dtid = self.ty(dst).0 as i64;
        let dt = self.b.ins().iconst(types::I32, dtid);
        let out = self.call1("call", &[fi, n, arr, tp, dt]);
        if f.mutates_first() {
            let slot_val = self.b.ins().load(types::I64, flags(), arr, 0);
            self.assign(args[0], slot_val);
        }
        self.put(dst, out)
    }
}

/// Emits `if !ok { trap(msg) }`.
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
                        if op == BinOp::Div { b.ins().sdiv(x, y) } else { b.ins().srem(x, y) }
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
                    if op == BinOp::Div { b.ins().sdiv(x, y) } else { b.ins().srem(x, y) }
                }
            };
            // (r - lo) <= (hi - lo) as unsigned
            let shifted = b.ins().iadd_imm(r, -(lo as i64));
            let ok = b.ins().icmp_imm(IntCC::UnsignedLessThanOrEqual, shifted, (hi - lo) as i64);
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
                if heap[r.0 as usize] && !live[r.0 as usize] && Some(r) != dst && !dead.contains(&r) {
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
