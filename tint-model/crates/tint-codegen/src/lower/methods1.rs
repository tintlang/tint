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
        let (fast, slow, done) = (
            self.b.create_block(),
            self.b.create_block(),
            self.b.create_block(),
        );
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
        let slot = self.b.create_sized_stack_slot(StackSlotData::new(
            StackSlotKind::ExplicitSlot,
            size,
            3,
        ));
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
        let off = if size == 1 {
            idx
        } else {
            self.b.ins().imul_imm(idx, size)
        };
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
        let v = if lay.mem == lay.reg {
            v
        } else {
            self.b.ins().ireduce(lay.mem, v)
        };
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
}
