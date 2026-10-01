use super::*;

impl<'a, 'b> Fc<'a, 'b> {
    // ---- runtime calls ------------------------------------------------------------------------------------------

    pub(super) fn rt(&mut self, dst: Reg, f: RtFn, args: &[Reg]) -> Result<(), Unsupported> {
        match f {
            RtFn::ListLen => {
                self.get(args[0]);
                self.ins(I::I32Load(ma(OFF_LEN, 2)));
                self.count_to(dst);
            }
            RtFn::ListPush => {
                let (list_reg, item) = (args[0], args[1]);
                let lay = self.list_elem(list_reg)?;
                let list = self.lr(list_reg);
                let bits = self.slot_bits(item, &[]);
                let len = self.tmp(I32);
                self.lget(list);
                self.ins(I::I32Load(ma(OFF_LEN, 2)));
                self.lset(len);
                // Fast path: room and unshared.
                self.lget(len);
                self.lget(list);
                self.ins(I::I32Load(ma(OFF_CAP, 2)));
                self.ins(I::I32LtU);
                self.lget(list);
                self.ins(I::I32Load(ma(OFF_RC, 2)));
                self.i32c(1);
                self.ins(I::I32Eq);
                self.ins(I::I32And);
                self.ins(I::If(BlockType::Empty));
                let addr = self.tmp(I32);
                self.lget(list);
                self.ins(I::I32Load(ma(OFF_PTR, 2)));
                self.lget(len);
                if lay.size != 1 {
                    self.i32c(lay.size as i32);
                    self.ins(I::I32Mul);
                }
                self.ins(I::I32Add);
                self.lset(addr);
                if lay.heap {
                    self.lget(addr);
                    self.lget(bits);
                    self.ins(I::I64Store(ma(0, 3)));
                } else {
                    let vt = self.vt(item);
                    let v = self.lr(item);
                    self.store_elem(addr, v, lay, vt);
                }
                self.lget(list);
                self.lget(len);
                self.i32c(1);
                self.ins(I::I32Add);
                self.ins(I::I32Store(ma(OFF_LEN, 2)));
                self.ins(I::Else);
                self.lget(list);
                self.lget(bits);
                self.call(Imp::ListPush);
                self.lset(list);
                self.ins(I::End);
                self.i32c(0);
                self.set(dst);
            }
            RtFn::MapLen | RtFn::MapIsEmpty => {
                self.get(args[0]);
                self.call(Imp::MapLen);
                if f == RtFn::MapLen {
                    self.count_to(dst);
                } else {
                    self.ins(I::I32Eqz);
                    self.set(dst);
                }
            }
            RtFn::MapGet | RtFn::ListPop => {
                let (some_tag, none_tag, nslots, payload) = self.option_layout(dst)?;
                let heap = is_heap(self.m, payload);
                let signed = matches!(self.m.types.kind(payload), TyKind::Num(NumKind::I32));
                if f == RtFn::MapGet {
                    self.get(args[0]);
                    self.get(args[1]);
                    self.i32c(some_tag);
                    self.i32c(none_tag);
                    self.i32c(nslots);
                    self.i32c(heap as i32);
                    self.call(Imp::MapGetOpt);
                } else {
                    let list = self.unique(args[0], Imp::ListUnique);
                    self.lget(list);
                    self.i32c(some_tag);
                    self.i32c(none_tag);
                    self.i32c(nslots);
                    self.i32c(heap as i32 | (signed as i32) << 1);
                    self.call(Imp::ListPopOpt);
                }
                let o = self.tmp(I32);
                self.lset(o);
                self.assign(dst, o);
            }
            RtFn::MapHas => {
                self.get(args[0]);
                self.get(args[1]);
                self.call(Imp::MapHas);
                self.set(dst);
            }
            RtFn::MapSet => {
                let (map_reg, key, val) = (args[0], args[1], args[2]);
                let map = self.lr(map_reg);
                let bits = self.slot_bits(val, &[]);
                self.lget(map);
                self.get(key);
                self.lget(bits);
                self.call(Imp::MapSet);
                self.lset(map);
                self.i32c(0);
                self.set(dst);
            }
            RtFn::StrConcat => {
                let n = args.len() as i32;
                self.i32c(4 * n);
                self.call(Imp::RtScratch);
                let buf = self.tmp(I32);
                self.lset(buf);
                for (i, a) in args.iter().enumerate() {
                    self.lget(buf);
                    self.get(*a);
                    self.ins(I::I32Store(ma(4 * i as u64, 2)));
                }
                self.i32c(n);
                self.lget(buf);
                self.call(Imp::StrConcat);
                let s = self.tmp(I32);
                self.lset(s);
                self.assign(dst, s);
            }
            RtFn::Sqrt | RtFn::Abs | RtFn::Sign | RtFn::Min | RtFn::Max | RtFn::Clamp
                if self.vt(dst) == ValType::F64
                    && args
                        .iter()
                        .all(|a| self.f.regs[a.0 as usize] == self.f.regs[dst.0 as usize]) =>
            {
                match f {
                    RtFn::Sqrt => {
                        self.get(args[0]);
                        self.ins(I::F64Sqrt);
                    }
                    RtFn::Abs => {
                        self.get(args[0]);
                        self.ins(I::F64Abs);
                    }
                    RtFn::Sign => {
                        // 1 if x > 0, -1 if x < 0, else 0 (also for NaN)
                        let x = self.lr(args[0]);
                        self.lget(x);
                        self.ins(I::F64Const(0.0.into()));
                        self.ins(I::F64Gt);
                        self.ins(I::If(BlockType::Result(F64)));
                        self.ins(I::F64Const(1.0.into()));
                        self.ins(I::Else);
                        self.lget(x);
                        self.ins(I::F64Const(0.0.into()));
                        self.ins(I::F64Lt);
                        self.ins(I::If(BlockType::Result(F64)));
                        self.ins(I::F64Const((-1.0).into()));
                        self.ins(I::Else);
                        self.ins(I::F64Const(0.0.into()));
                        self.ins(I::End);
                        self.ins(I::End);
                    }
                    RtFn::Min => {
                        self.get(args[0]);
                        self.get(args[1]);
                        self.f64_min();
                    }
                    RtFn::Max => {
                        self.get(args[0]);
                        self.get(args[1]);
                        self.f64_max();
                    }
                    _ => {
                        self.get(args[0]);
                        self.get(args[1]);
                        self.f64_max();
                        self.get(args[2]);
                        self.f64_min();
                    }
                }
                self.set(dst);
            }
            RtFn::StrLen => {
                self.get(args[0]);
                self.call(Imp::StrLen);
                self.count_to(dst);
            }
            _ => self.bridge(dst, f, args)?,
        }
        Ok(())
    }

    /// A `u32` count on the stack as the number `dst` holds.
    pub(super) fn count_to(&mut self, dst: Reg) {
        match self.vt(dst) {
            ValType::F64 => self.ins(I::F64ConvertI32U),
            _ => self.ins(I::I64ExtendI32U),
        }
        self.set(dst);
    }

    /// Rust's `f64::min` on the two f64 on the stack: a NaN operand is ignored.
    pub(super) fn f64_min(&mut self) {
        let (b, a) = (self.tf_b(), self.tf_a());
        self.lset(b);
        self.lset(a);
        // NaN in b: take a
        self.lget(a);
        self.lget(a);
        self.lget(b);
        self.ins(I::F64Lt);
        self.ins(I::If(BlockType::Result(F64)));
        self.lget(a);
        self.ins(I::Else);
        self.lget(b);
        self.ins(I::End);
        self.lget(b);
        self.lget(b);
        self.ins(I::F64Ne);
        self.ins(I::Select);
    }

    pub(super) fn f64_max(&mut self) {
        let (b, a) = (self.tf_b(), self.tf_a());
        self.lset(b);
        self.lset(a);
        self.lget(a);
        self.lget(a);
        self.lget(b);
        self.ins(I::F64Gt);
        self.ins(I::If(BlockType::Result(F64)));
        self.lget(a);
        self.ins(I::Else);
        self.lget(b);
        self.ins(I::End);
        self.lget(b);
        self.lget(b);
        self.ins(I::F64Ne);
        self.ins(I::Select);
    }

    pub(super) fn tf_a(&mut self) -> u32 {
        self.tmp(F64)
    }

    pub(super) fn tf_b(&mut self) -> u32 {
        self.tmp(F64)
    }

    /// `(Some tag, None tag, object slots, payload type)` of the `Option`
    /// type of register `dst`.
    pub(super) fn option_layout(&self, dst: Reg) -> Result<(i32, i32, i32, TyId), Unsupported> {
        let Some(adt) = self.m.types.as_adt(self.ty(dst)) else {
            return unsupported("Option result of a runtime call is not an enum");
        };
        let def = self.m.types.adt(adt);
        let (Some(some), Some(none)) = (def.variant_index("Some"), def.variant_index("None"))
        else {
            return unsupported("Option without Some/None");
        };
        let payload = match &def.body {
            AdtBody::Enum(variants) => variants[some as usize].fields[0].ty,
            _ => return unsupported("Option is not an enum"),
        };
        Ok((
            some as i32,
            none as i32,
            adt_slots(self.m, adt) as i32,
            payload,
        ))
    }
}
