use super::*;

impl<'a, 'b> Fc<'a, 'b> {
    pub(super) fn cast_fail_if(&mut self, from: NumKind, to: NumKind, src: Reg) {
        self.ins(I::If(BlockType::Empty));
        self.i32c(kind_index(from) as i32);
        self.i32c(kind_index(to) as i32);
        self.get(src);
        if from.is_float() {
            self.ins(I::I64ReinterpretF64);
        }
        self.call(Imp::CastFail);
        self.ins(I::Unreachable);
        self.ins(I::End);
    }

    /// Converts `src` and leaves the result on the stack.
    pub(super) fn cast(&mut self, src: Reg, from: NumKind, to: NumKind) {
        if to.is_float() {
            if from.is_float() {
                if to == NumKind::F32 {
                    self.get(src);
                    self.ins(I::F64Abs);
                    self.ins(I::F64Const((f32::MAX as f64).into()));
                    self.ins(I::F64Le);
                    self.ins(I::I32Eqz);
                    self.cast_fail_if(from, to, src);
                    self.get(src);
                    self.round_f32();
                } else {
                    self.get(src);
                }
            } else {
                self.get(src);
                self.ins(if from == NumKind::U64 {
                    I::F64ConvertI64U
                } else {
                    I::F64ConvertI64S
                });
                if to == NumKind::F32 {
                    self.round_f32();
                }
            }
            return;
        }
        let (tlo, thi) = to.int_range();
        if from.is_float() {
            // Integral and within [lo, hi + 1).
            let tf = self.tf;
            self.get(src);
            self.lset(tf);
            self.lget(tf);
            self.ins(I::F64Trunc);
            self.lget(tf);
            self.ins(I::F64Eq);
            self.lget(tf);
            self.ins(I::F64Const((tlo as f64).into()));
            self.ins(I::F64Ge);
            self.ins(I::I32And);
            self.lget(tf);
            self.ins(I::F64Const(((thi + 1) as f64).into()));
            self.ins(I::F64Lt);
            self.ins(I::I32And);
            self.ins(I::I32Eqz);
            self.cast_fail_if(from, to, src);
            self.lget(tf);
            self.ins(if to == NumKind::U64 {
                I::I64TruncF64U
            } else {
                I::I64TruncF64S
            });
            return;
        }
        let (flo, fhi) = from.int_range();
        if !(tlo <= flo && thi >= fhi) {
            if from == NumKind::U64 {
                self.get(src);
                self.i64c(thi as i64);
                self.ins(I::I64GtU);
            } else if to == NumKind::U64 {
                self.get(src);
                self.i64c(0);
                self.ins(I::I64LtS);
            } else {
                self.get(src);
                self.i64c((tlo as i64).wrapping_neg());
                self.ins(I::I64Add);
                self.i64c((thi - tlo) as i64);
                self.ins(I::I64GtU);
            }
            self.cast_fail_if(from, to, src);
        }
        self.get(src);
    }
}
