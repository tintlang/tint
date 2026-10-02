use super::*;

impl<'a, 'b> Fc<'a, 'b> {
    // ---- lists ---------------------------------------------------------------------------------------

    pub(super) fn list_elem(&self, list: Reg) -> Result<ElemLayout, Unsupported> {
        match self.m.types.kind(self.ty(list)) {
            TyKind::List(e) => Ok(elem_layout(self.m, *e)),
            other => unsupported(format!("list operation on {other:?}")),
        }
    }

    /// A list index as an i64 local: integers pass through, floats must be integral.
    pub(super) fn index(&mut self, reg: Reg) -> Result<u32, Unsupported> {
        let kind = match self.m.types.kind(self.ty(reg)) {
            TyKind::Num(k) => *k,
            other => return unsupported(format!("index of type {other:?}")),
        };
        if !kind.is_float() {
            return Ok(self.lr(reg));
        }
        let i = self.tmp(I64);
        self.get(reg);
        self.ins(I::I64TruncF64S);
        self.lset(i);
        self.lget(i);
        self.ins(I::F64ConvertI64S);
        self.get(reg);
        self.ins(I::F64Ne);
        self.trap_if_bad("list index is not an integer");
        Ok(i)
    }

    /// Bounds-checks `idx` against the list and returns a local with the element address.
    pub(super) fn elem_addr(&mut self, list: u32, idx: u32, size: u32, check: bool) -> u32 {
        if check {
            self.lget(idx);
            self.lget(list);
            self.ins(I::I32Load(ma(OFF_LEN, 2)));
            self.ins(I::I64ExtendI32U);
            self.ins(I::I64GeU);
            self.ins(I::If(BlockType::Empty));
            self.lget(list);
            self.lget(idx);
            self.call(Imp::ListOob);
            self.ins(I::Unreachable);
            self.ins(I::End);
        }
        let addr = self.tmp(I32);
        self.lget(list);
        self.ins(I::I32Load(ma(OFF_PTR, 2)));
        self.lget(idx);
        self.ins(I::I32WrapI64);
        if size != 1 {
            self.i32c(size as i32);
            self.ins(I::I32Mul);
        }
        self.ins(I::I32Add);
        self.lset(addr);
        addr
    }

    /// Pushes the element at `addr` as a register value of type `vt`
    /// (a pointer for a heap element).
    pub(super) fn load_elem(&mut self, addr: u32, lay: ElemLayout, vt: ValType) {
        self.lget(addr);
        if lay.heap {
            self.ins(I::I32Load(ma(0, 2)));
            return;
        }
        match (lay.size, vt) {
            (1, ValType::I32) => self.ins(I::I32Load8U(ma(0, 0))),
            (1, _) => self.ins(I::I64Load8U(ma(0, 0))),
            (4, _) if lay.signed => self.ins(I::I64Load32S(ma(0, 2))),
            (4, _) => self.ins(I::I64Load32U(ma(0, 2))),
            (_, ValType::F64) => self.ins(I::F64Load(ma(0, 3))),
            _ => self.ins(I::I64Load(ma(0, 3))),
        }
    }

    /// Stores the value in `v` (a register value of type `vt`) as an element.
    pub(super) fn store_elem(&mut self, addr: u32, v: u32, lay: ElemLayout, vt: ValType) {
        self.lget(addr);
        self.lget(v);
        if lay.heap {
            self.ins(I::I64ExtendI32U);
            self.ins(I::I64Store(ma(0, 3)));
            return;
        }
        match (lay.size, vt) {
            (1, ValType::I32) => self.ins(I::I32Store8(ma(0, 0))),
            (1, _) => self.ins(I::I64Store8(ma(0, 0))),
            (4, _) => self.ins(I::I64Store32(ma(0, 2))),
            (_, ValType::F64) => self.ins(I::F64Store(ma(0, 3))),
            _ => self.ins(I::I64Store(ma(0, 3))),
        }
    }

    /// `rx = get list[i]; ry = get rx.f` where `rx` is dead afterwards and `ry` a
    /// scalar: the field is read straight out of the element, without taking a
    /// reference to it and giving it back. Returns whether it emitted both.
    pub(super) fn fuse_index_field(
        &mut self,
        block: usize,
        at: usize,
        first: &Instr,
        second: &Instr,
    ) -> Result<bool, Unsupported> {
        let (Instr::Get { dst: rx, base: list_reg, proj: Proj::Index(i) }, Instr::Get { dst: ry, base: from, proj }) =
            (first, second)
        else {
            return Ok(false);
        };
        let (Proj::Field(f) | Proj::Tuple(f)) = proj else { return Ok(false) };
        if from != rx
            || ry == rx
            || ry == list_reg
            || self.heap[ry.0 as usize]
            || !self.heap[rx.0 as usize]
            || !self.dies[block][at + 1].contains(rx)
            || self.dies[block][at].contains(rx)
        {
            return Ok(false);
        }
        let lay = self.list_elem(*list_reg)?;
        if !lay.heap {
            return Ok(false);
        }
        // Registers whose last use is the first instruction (the list, say) are given back after both.
        let mut dying = self.dies[block][at].clone();
        for r in &self.dies[block][at + 1] {
            if r != rx && !dying.contains(r) {
                dying.push(*r);
            }
        }
        self.moved.clear();
        // `rx` is empty on entry in every flow (it dies at the second instruction), nothing to give back.
        self.clear(*rx);
        let idx = self.index(*i)?;
        let list = self.lr(*list_reg);
        let addr = self.elem_addr(list, idx, 8, true);
        let obj = self.tmp(I32);
        self.lget(addr);
        self.ins(I::I32Load(ma(0, 2)));
        self.lset(obj);
        self.load_slot(obj, *f as usize, *ry);
        self.set(*ry);
        for r in dying {
            let l = self.lr(r);
            self.release(l);
            self.clear(r);
        }
        self.end_instr();
        Ok(true)
    }

    // ---- projections ---------------------------------------------------------------------------------------

    pub(super) fn get_proj(
        &mut self,
        dst: Reg,
        base: Reg,
        proj: &Proj,
        take: bool,
    ) -> Result<(), Unsupported> {
        match proj {
            Proj::Field(i) | Proj::Tuple(i) => {
                let obj = if take {
                    self.unique(base, Imp::ObjUnique)
                } else {
                    self.lr(base)
                };
                let slot = *i as usize;
                self.load_slot(obj, slot, dst);
                if self.heap[dst.0 as usize] {
                    let v = self.tmp(I32);
                    self.lset(v);
                    if take {
                        // The slot gives its reference away and stays empty.
                        self.lget(obj);
                        self.i64c(0);
                        self.ins(I::I64Store(ma(slot_off(slot), 3)));
                    } else {
                        self.retain(v);
                    }
                    self.assign(dst, v);
                } else {
                    self.set(dst);
                }
            }
            Proj::Index(i) => {
                let lay = self.list_elem(base)?;
                let idx = self.index(*i)?;
                if take && lay.heap {
                    let list = self.unique(base, Imp::ListUnique);
                    // Bounds-checked, then the element's reference moves out (the slot is emptied).
                    let addr = self.elem_addr(list, idx, 8, true);
                    let v = self.tmp(I32);
                    self.lget(addr);
                    self.ins(I::I32Load(ma(0, 2)));
                    self.lset(v);
                    self.lget(addr);
                    self.i64c(0);
                    self.ins(I::I64Store(ma(0, 3)));
                    self.assign(dst, v);
                } else {
                    let list = self.lr(base);
                    let addr = self.elem_addr(list, idx, lay.size, true);
                    let vt = self.vt(dst);
                    self.load_elem(addr, lay, vt);
                    if lay.heap {
                        let v = self.tmp(I32);
                        self.lset(v);
                        self.retain(v);
                        self.assign(dst, v);
                    } else {
                        self.set(dst);
                    }
                }
            }
            Proj::Key(k) => {
                let map = if take {
                    self.unique(base, Imp::MapUnique)
                } else {
                    self.lr(base)
                };
                let key = self.lr(*k);
                self.lget(map);
                self.lget(key);
                self.call(if take { Imp::MapTake } else { Imp::MapGet });
                let vt = self.vt(dst);
                self.from_bits(vt);
                if self.heap[dst.0 as usize] {
                    let v = self.tmp(I32);
                    self.lset(v);
                    if !take {
                        self.retain(v);
                    }
                    self.assign(dst, v);
                } else {
                    self.set(dst);
                }
            }
        }
        Ok(())
    }

    pub(super) fn set_proj(&mut self, base: Reg, proj: &Proj, src: Reg) -> Result<(), Unsupported> {
        match proj {
            Proj::Field(i) | Proj::Tuple(i) => {
                let obj = self.unique(base, Imp::ObjUnique);
                let slot = *i as usize;
                if self.heap[src.0 as usize] {
                    let old = self.tmp(I32);
                    self.lget(obj);
                    self.ins(I::I32Load(ma(slot_off(slot), 2)));
                    self.lset(old);
                    let v = self.own(src, &[]);
                    self.lget(obj);
                    self.lget(v);
                    self.ins(I::I64ExtendI32U);
                    self.ins(I::I64Store(ma(slot_off(slot), 3)));
                    self.release(old);
                } else {
                    self.store_slot_scalar(obj, slot, src);
                }
            }
            Proj::Index(i) => {
                let lay = self.list_elem(base)?;
                let idx = self.index(*i)?;
                let list = self.lr(base);
                if lay.heap {
                    // The runtime copies a shared list and releases the old element.
                    let v = self.own(src, &[]);
                    // Fast path: unshared and in range, the slot takes the reference.
                    self.lget(idx);
                    self.lget(list);
                    self.ins(I::I32Load(ma(OFF_LEN, 2)));
                    self.ins(I::I64ExtendI32U);
                    self.ins(I::I64LtU);
                    self.lget(list);
                    self.ins(I::I32Load(ma(OFF_RC, 2)));
                    self.i32c(1);
                    self.ins(I::I32Eq);
                    self.ins(I::I32And);
                    self.ins(I::If(BlockType::Empty));
                    let addr = self.elem_addr(list, idx, 8, false);
                    let old = self.tmp(I32);
                    self.lget(addr);
                    self.ins(I::I32Load(ma(0, 2)));
                    self.lset(old);
                    self.lget(addr);
                    self.lget(v);
                    self.ins(I::I64ExtendI32U);
                    self.ins(I::I64Store(ma(0, 3)));
                    self.release(old);
                    self.ins(I::Else);
                    self.lget(list);
                    self.lget(idx);
                    self.lget(v);
                    self.ins(I::I64ExtendI32U);
                    self.call(Imp::ListSet);
                    self.lset(list);
                    self.ins(I::End);
                    return Ok(());
                }
                let vt = self.vt(src);
                let v = self.lr(src);
                // Fast path: unshared and in range. Otherwise the runtime copies
                // the list or reports the bad index.
                self.lget(idx);
                self.lget(list);
                self.ins(I::I32Load(ma(OFF_LEN, 2)));
                self.ins(I::I64ExtendI32U);
                self.ins(I::I64LtU);
                self.lget(list);
                self.ins(I::I32Load(ma(OFF_RC, 2)));
                self.i32c(1);
                self.ins(I::I32Eq);
                self.ins(I::I32And);
                self.ins(I::If(BlockType::Empty));
                let addr = self.elem_addr(list, idx, lay.size, false);
                self.store_elem(addr, v, lay, vt);
                self.ins(I::Else);
                self.lget(list);
                self.lget(idx);
                self.lget(v);
                self.to_bits(vt);
                self.call(Imp::ListSet);
                self.lset(list);
                self.ins(I::End);
            }
            Proj::Key(k) => {
                let key = self.lr(*k);
                let map = self.lr(base);
                let bits = self.slot_bits(src, &[]);
                self.lget(map);
                self.lget(key);
                self.lget(bits);
                self.call(Imp::MapSet);
                self.lset(map);
            }
        }
        Ok(())
    }
}
