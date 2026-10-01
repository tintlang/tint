use super::*;

impl<'a, 'b> Fc<'a, 'b> {
    pub(super) fn new_local(&mut self, vt: ValType) -> u32 {
        self.local_types.push(vt);
        self.nparams + self.local_types.len() as u32 - 1
    }

    pub(super) fn finish(self) -> Function {
        let mut decls: Vec<(u32, ValType)> = Vec::new();
        for vt in &self.local_types {
            match decls.last_mut() {
                Some((n, t)) if t == vt => *n += 1,
                _ => decls.push((1, *vt)),
            }
        }
        let mut f = Function::new(decls);
        for i in &self.code {
            f.instruction(i);
        }
        f
    }

    pub(super) fn ins(&mut self, i: I<'static>) {
        self.code.push(i);
    }

    pub(super) fn tmp(&mut self, vt: ValType) -> u32 {
        let l = match self.free.iter().position(|(t, _)| *t == vt) {
            Some(pos) => self.free.remove(pos).1,
            None => self.new_local(vt),
        };
        self.taken.push((vt, l));
        l
    }

    pub(super) fn end_instr(&mut self) {
        self.free.append(&mut self.taken);
    }

    pub(super) fn lget(&mut self, l: u32) {
        self.ins(I::LocalGet(l));
    }

    pub(super) fn lset(&mut self, l: u32) {
        self.ins(I::LocalSet(l));
    }

    pub(super) fn get(&mut self, r: Reg) {
        let l = self.local[r.0 as usize];
        self.lget(l);
    }

    pub(super) fn set(&mut self, r: Reg) {
        let l = self.local[r.0 as usize];
        self.lset(l);
    }

    pub(super) fn lr(&self, r: Reg) -> u32 {
        self.local[r.0 as usize]
    }

    pub(super) fn vt(&self, r: Reg) -> ValType {
        val_type(self.m, self.f.reg_ty(r))
    }

    pub(super) fn ty(&self, r: Reg) -> TyId {
        self.f.reg_ty(r)
    }

    pub(super) fn call(&mut self, i: Imp) {
        let idx = self.cx.imp(i);
        self.ins(I::Call(idx));
    }

    pub(super) fn i32c(&mut self, v: i32) {
        self.ins(I::I32Const(v));
    }

    pub(super) fn i64c(&mut self, v: i64) {
        self.ins(I::I64Const(v));
    }

    // ---- bit conversions ---------------------------------------------------------

    /// Register value on the stack -> 64-bit bits.
    pub(super) fn to_bits(&mut self, vt: ValType) {
        match vt {
            ValType::F64 => self.ins(I::I64ReinterpretF64),
            ValType::I32 => self.ins(I::I64ExtendI32U),
            _ => {}
        }
    }

    /// 64-bit bits on the stack -> register value of type `vt`.
    pub(super) fn from_bits(&mut self, vt: ValType) {
        match vt {
            ValType::F64 => self.ins(I::F64ReinterpretI64),
            ValType::I32 => self.ins(I::I32WrapI64),
            _ => {}
        }
    }

    // ---- reference counting -------------------------------------------------------

    pub(super) fn retain(&mut self, p: u32) {
        self.lget(p);
        self.ins(I::If(BlockType::Empty));
        self.lget(p);
        self.lget(p);
        self.ins(I::I32Load(ma(OFF_RC, 2)));
        self.i32c(1);
        self.ins(I::I32Add);
        self.ins(I::I32Store(ma(OFF_RC, 2)));
        self.ins(I::End);
    }

    pub(super) fn release(&mut self, p: u32) {
        self.lget(p);
        self.ins(I::If(BlockType::Empty));
        self.lget(p);
        self.ins(I::I32Load(ma(OFF_RC, 2)));
        self.i32c(1);
        self.ins(I::I32GtU);
        self.ins(I::If(BlockType::Empty));
        self.lget(p);
        self.lget(p);
        self.ins(I::I32Load(ma(OFF_RC, 2)));
        self.i32c(1);
        self.ins(I::I32Sub);
        self.ins(I::I32Store(ma(OFF_RC, 2)));
        self.ins(I::Else);
        self.lget(p);
        self.call(Imp::Release);
        self.ins(I::End);
        self.ins(I::End);
    }

    /// Empties a heap register whose reference was handed on.
    pub(super) fn clear(&mut self, r: Reg) {
        self.i32c(0);
        self.set(r);
    }

    /// Stores a freshly owned heap value (in local `v`) into `dst`, dropping what it held.
    pub(super) fn assign(&mut self, dst: Reg, v: u32) {
        let old = self.lr(dst);
        self.release(old);
        self.lget(v);
        self.lset(old);
    }

    /// The value of `reg` with one owned reference to give away, in a local.
    /// `rest` are the operands of the same instruction that come after this one.
    pub(super) fn own(&mut self, reg: Reg, rest: &[Reg]) -> u32 {
        let l = self.lr(reg);
        if !self.heap[reg.0 as usize] {
            return l;
        }
        let t = self.tmp(I32);
        self.lget(l);
        self.lset(t);
        if self.dying.contains(&reg) && !rest.contains(&reg) {
            self.moved.push(reg);
            self.clear(reg);
        } else {
            self.retain(t);
        }
        t
    }

    /// Bits of a scalar value or the owned pointer (extended) of a heap one, in an i64 local.
    pub(super) fn slot_bits(&mut self, src: Reg, rest: &[Reg]) -> u32 {
        let t = self.tmp(I64);
        if self.heap[src.0 as usize] {
            let o = self.own(src, rest);
            self.lget(o);
            self.ins(I::I64ExtendI32U);
        } else {
            self.get(src);
            let vt = self.vt(src);
            self.to_bits(vt);
        }
        self.lset(t);
        t
    }

    /// Makes `reg` unique (copying with `copy` when shared); returns its local.
    pub(super) fn unique(&mut self, reg: Reg, copy: Imp) -> u32 {
        let l = self.lr(reg);
        self.lget(l);
        self.ins(I::I32Load(ma(OFF_RC, 2)));
        self.i32c(1);
        self.ins(I::I32Ne);
        self.ins(I::If(BlockType::Empty));
        self.lget(l);
        self.call(copy);
        self.lset(l);
        self.ins(I::End);
        l
    }

    pub(super) fn trap_if_bad(&mut self, msg: &str) {
        let id = self.cx.msg(msg);
        self.ins(I::If(BlockType::Empty));
        self.i32c(id);
        self.call(Imp::Trap);
        self.ins(I::Unreachable);
        self.ins(I::End);
    }

    // ---- structured control flow ------------------------------------------------------------
}
