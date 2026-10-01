use super::*;

impl<'a, 'b> Fc<'a, 'b> {
    /// Stores the call result on the stack into `dst`: a heap result is an
    /// owned reference.
    pub(super) fn put_result(&mut self, dst: Reg) {
        if self.heap[dst.0 as usize] {
            let t = self.tmp(I32);
            self.lset(t);
            self.assign(dst, t);
        } else {
            self.set(dst);
        }
    }

    /// Writes the i64 on the stack as the value of `dst` (scalar bits or an owned pointer).
    pub(super) fn put_bits(&mut self, dst: Reg) {
        let vt = self.vt(dst);
        self.from_bits(vt);
        self.put_result(dst);
    }

    /// Allocates a struct, tuple, enum or closure object and fills its slots.
    pub(super) fn build_obj(
        &mut self,
        dst: Reg,
        fields: &[Reg],
        nslots: u32,
        mask: u64,
        base: usize,
        tag: Option<i64>,
    ) {
        self.i32c(nslots as i32);
        self.i64c(mask as i64);
        self.call(Imp::ObjNew);
        let obj = self.tmp(I32);
        self.lset(obj);
        if let Some(tag) = tag {
            self.lget(obj);
            self.i64c(tag);
            self.ins(I::I64Store(ma(slot_off(0), 3)));
        }
        for (i, f) in fields.iter().enumerate() {
            let bits = self.slot_bits(*f, &fields[i + 1..]);
            self.lget(obj);
            self.lget(bits);
            self.ins(I::I64Store(ma(slot_off(base + i), 3)));
        }
        self.assign(dst, obj);
    }

    /// Pushes slot `slot` of the object in local `obj`, typed like register `dst`.
    pub(super) fn load_slot(&mut self, obj: u32, slot: usize, dst: Reg) {
        self.lget(obj);
        match self.vt(dst) {
            ValType::F64 => self.ins(I::F64Load(ma(slot_off(slot), 3))),
            ValType::I64 => self.ins(I::I64Load(ma(slot_off(slot), 3))),
            _ => self.ins(I::I32Load(ma(slot_off(slot), 2))),
        }
    }

    /// Stores register `src` (scalar) into slot `slot` of the object in `obj`.
    pub(super) fn store_slot_scalar(&mut self, obj: u32, slot: usize, src: Reg) {
        self.lget(obj);
        self.get(src);
        let vt = self.vt(src);
        self.to_bits(vt);
        self.ins(I::I64Store(ma(slot_off(slot), 3)));
    }
}
