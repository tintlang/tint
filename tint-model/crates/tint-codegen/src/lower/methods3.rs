impl<'a, 'b> L<'a, 'b> {
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

    fn index_field_set(
        &mut self,
        base: Reg,
        i: Reg,
        slot: usize,
        v: Reg,
    ) -> Result<(), Unsupported> {
        self.clif(v)?;
        let lay = self.list_elem(base)?;
        let idx = self.index(i)?;
        let list = self.unique(base, "list_unique");
        let addr = self.elem_addr(list, idx, lay.size, true);
        let obj = self.load_elem(addr, lay);
        let rc = self.b.ins().load(types::I64, flags(), obj, rt::OFF_RC);
        let one = self.b.ins().icmp_imm(IntCC::Equal, rc, 1);
        let (fast, slow, done) = (
            self.b.create_block(),
            self.b.create_block(),
            self.b.create_block(),
        );
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

    fn get_proj(
        &mut self,
        dst: Reg,
        base: Reg,
        proj: &Proj,
        take: bool,
    ) -> Result<(), Unsupported> {
        let t = self.clif(dst)?;
        match proj {
            Proj::Field(i) | Proj::Tuple(i) => {
                let obj = if take {
                    self.unique(base, "obj_unique")
                } else {
                    self.get(base)
                };
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
                let map = if take {
                    self.unique(base, "map_unique")
                } else {
                    self.get(base)
                };
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
                let (fast, slow, done) = (
                    self.b.create_block(),
                    self.b.create_block(),
                    self.b.create_block(),
                );
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
                let ops = [
                    CmpOp::Eq,
                    CmpOp::Ne,
                    CmpOp::Lt,
                    CmpOp::Le,
                    CmpOp::Gt,
                    CmpOp::Ge,
                ];
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
}
