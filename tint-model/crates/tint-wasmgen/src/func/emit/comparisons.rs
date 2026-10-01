use super::*;

impl<'a, 'b> Fc<'a, 'b> {
    // ---- comparisons ----------------------------------------------------------------------------------------

    pub(super) fn cmp(&mut self, op: CmpOp, a: Reg, b: Reg) -> Result<(), Unsupported> {
        let ty = self.ty(a);
        match self.m.types.kind(ty) {
            TyKind::Unit => {
                self.i32c((op == CmpOp::Eq) as i32);
            }
            TyKind::Bool => {
                self.get(a);
                self.get(b);
                match op {
                    CmpOp::Eq => self.ins(I::I32Eq),
                    CmpOp::Ne => self.ins(I::I32Ne),
                    _ => return unsupported("ordering of bools"),
                }
            }
            TyKind::Num(k) if k.is_float() => {
                self.get(a);
                self.get(b);
                self.ins(match op {
                    CmpOp::Eq => I::F64Eq,
                    CmpOp::Ne => I::F64Ne,
                    CmpOp::Lt => I::F64Lt,
                    CmpOp::Le => I::F64Le,
                    CmpOp::Gt => I::F64Gt,
                    CmpOp::Ge => I::F64Ge,
                });
            }
            TyKind::Num(k) => {
                self.get(a);
                self.get(b);
                let u = *k == NumKind::U64;
                self.ins(match (op, u) {
                    (CmpOp::Eq, _) => I::I64Eq,
                    (CmpOp::Ne, _) => I::I64Ne,
                    (CmpOp::Lt, false) => I::I64LtS,
                    (CmpOp::Le, false) => I::I64LeS,
                    (CmpOp::Gt, false) => I::I64GtS,
                    (CmpOp::Ge, false) => I::I64GeS,
                    (CmpOp::Lt, true) => I::I64LtU,
                    (CmpOp::Le, true) => I::I64LeU,
                    (CmpOp::Gt, true) => I::I64GtU,
                    (CmpOp::Ge, true) => I::I64GeU,
                });
            }
            TyKind::Str => {
                self.get(a);
                self.get(b);
                self.call(Imp::StrCmp);
                self.i32c(0);
                self.ins(match op {
                    CmpOp::Eq => I::I32Eq,
                    CmpOp::Ne => I::I32Ne,
                    CmpOp::Lt => I::I32LtS,
                    CmpOp::Le => I::I32LeS,
                    CmpOp::Gt => I::I32GtS,
                    CmpOp::Ge => I::I32GeS,
                });
            }
            _ => {
                let code = CMP_OPS.iter().position(|o| *o == op).unwrap() as i32;
                self.i32c(ty.0 as i32);
                self.i32c(code);
                self.get(a);
                self.ins(I::I64ExtendI32U);
                self.get(b);
                self.ins(I::I64ExtendI32U);
                self.call(Imp::Cmp);
            }
        }
        Ok(())
    }

    // ---- numbers ---------------------------------------------------------------------------------------------

    pub(super) fn round_f32(&mut self) {
        self.ins(I::F32DemoteF64);
        self.ins(I::F64PromoteF32);
    }

    /// Checked integer arithmetic on `t0 op t1`; leaves the result on the stack.
    pub(super) fn int_arith(&mut self, op: BinOp, kind: NumKind) {
        let [t0, t1, t2] = self.t;
        let overflow = format!("{} arithmetic overflow", kind.name());
        match kind {
            NumKind::I64 | NumKind::U64 => {
                let signed = kind == NumKind::I64;
                match op {
                    BinOp::Add | BinOp::Sub => {
                        self.lget(t0);
                        self.lget(t1);
                        self.ins(if op == BinOp::Add {
                            I::I64Add
                        } else {
                            I::I64Sub
                        });
                        self.lset(t2);
                        if signed {
                            if op == BinOp::Add {
                                // ((x ^ r) & (y ^ r)) < 0
                                self.lget(t0);
                                self.lget(t2);
                                self.ins(I::I64Xor);
                                self.lget(t1);
                                self.lget(t2);
                                self.ins(I::I64Xor);
                            } else {
                                // ((x ^ y) & (x ^ r)) < 0
                                self.lget(t0);
                                self.lget(t1);
                                self.ins(I::I64Xor);
                                self.lget(t0);
                                self.lget(t2);
                                self.ins(I::I64Xor);
                            }
                            self.ins(I::I64And);
                            self.i64c(0);
                            self.ins(I::I64LtS);
                        } else if op == BinOp::Add {
                            self.lget(t2);
                            self.lget(t0);
                            self.ins(I::I64LtU);
                        } else {
                            self.lget(t0);
                            self.lget(t1);
                            self.ins(I::I64LtU);
                        }
                        self.trap_if_bad(&overflow);
                        self.lget(t2);
                    }
                    BinOp::Mul => {
                        self.lget(t0);
                        self.lget(t1);
                        self.ins(I::I64Mul);
                        self.lset(t2);
                        // Fast path: operands small enough that the product cannot overflow.
                        if signed {
                            for t in [t0, t1] {
                                self.lget(t);
                                self.i64c(1 << 31);
                                self.ins(I::I64Add);
                                self.i64c(1 << 32);
                                self.ins(I::I64LtU);
                            }
                            self.ins(I::I32And);
                        } else {
                            self.lget(t0);
                            self.lget(t1);
                            self.ins(I::I64Or);
                            self.i64c(32);
                            self.ins(I::I64ShrU);
                            self.ins(I::I64Eqz);
                        }
                        self.ins(I::I32Eqz);
                        self.ins(I::If(BlockType::Empty));
                        self.lget(t0);
                        self.ins(I::I64Eqz);
                        self.ins(I::If(BlockType::Result(ValType::I32)));
                        self.i32c(0);
                        self.ins(I::Else);
                        if signed {
                            self.lget(t0);
                            self.i64c(-1);
                            self.ins(I::I64Eq);
                            self.ins(I::If(BlockType::Result(ValType::I32)));
                            self.lget(t1);
                            self.i64c(i64::MIN);
                            self.ins(I::I64Eq);
                            self.ins(I::Else);
                            self.lget(t2);
                            self.lget(t0);
                            self.ins(I::I64DivS);
                            self.lget(t1);
                            self.ins(I::I64Ne);
                            self.ins(I::End);
                        } else {
                            self.lget(t2);
                            self.lget(t0);
                            self.ins(I::I64DivU);
                            self.lget(t1);
                            self.ins(I::I64Ne);
                        }
                        self.ins(I::End);
                        self.trap_if_bad(&overflow);
                        self.ins(I::End);
                        self.lget(t2);
                    }
                    BinOp::Div | BinOp::Rem => {
                        self.lget(t1);
                        self.ins(I::I64Eqz);
                        self.trap_if_bad("division by zero");
                        if signed {
                            self.lget(t0);
                            self.i64c(i64::MIN);
                            self.ins(I::I64Eq);
                            self.lget(t1);
                            self.i64c(-1);
                            self.ins(I::I64Eq);
                            self.ins(I::I32And);
                            self.trap_if_bad(&overflow);
                        }
                        self.lget(t0);
                        self.lget(t1);
                        self.ins(match (op, signed) {
                            (BinOp::Div, true) => I::I64DivS,
                            (BinOp::Div, false) => I::I64DivU,
                            (_, true) => I::I64RemS,
                            (_, false) => I::I64RemU,
                        });
                    }
                }
            }
            _ => {
                // 32-bit and 8-bit kinds compute in i64, then range-check.
                let (lo, hi) = kind.int_range();
                if matches!(op, BinOp::Div | BinOp::Rem) {
                    self.lget(t1);
                    self.ins(I::I64Eqz);
                    self.trap_if_bad("division by zero");
                }
                self.lget(t0);
                self.lget(t1);
                self.ins(match op {
                    BinOp::Add => I::I64Add,
                    BinOp::Sub => I::I64Sub,
                    BinOp::Mul => I::I64Mul,
                    BinOp::Div => I::I64DivS,
                    BinOp::Rem => I::I64RemS,
                });
                self.lset(t2);
                self.lget(t2);
                self.i64c((lo as i64).wrapping_neg());
                self.ins(I::I64Add);
                self.i64c((hi - lo) as i64);
                self.ins(I::I64GtU);
                self.trap_if_bad(&overflow);
                self.lget(t2);
            }
        }
    }
}
