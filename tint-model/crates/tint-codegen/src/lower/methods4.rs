impl<'a, 'b> L<'a, 'b> {
    fn rt(&mut self, dst: Reg, f: RtFn, args: &[Reg]) -> Result<(), Unsupported> {
        match f {
            RtFn::ListLen => {
                let t = self.clif(dst)?;
                let list = self.get(args[0]);
                let n = self.b.ins().load(types::I64, flags(), list, rt::OFF_LEN);
                let v = if t == types::F64 {
                    self.b.ins().fcvt_from_sint(types::F64, n)
                } else {
                    n
                };
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
                let (fast, slow, done) = (
                    self.b.create_block(),
                    self.b.create_block(),
                    self.b.create_block(),
                );
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
                    if t == types::F64 {
                        self.b.ins().fcvt_from_sint(types::F64, n)
                    } else {
                        n
                    }
                } else {
                    self.b.ins().icmp_imm(IntCC::Equal, n, 0)
                };
                self.set(dst, v);
            }
            RtFn::MapGet | RtFn::ListPop => {
                self.clif(dst)?;
                let (some_tag, none_tag, nslots, payload) = self.option_layout(dst)?;
                let mask = if is_heap(self.m, payload) && f == RtFn::MapGet {
                    1 << 1
                } else {
                    0
                };
                let mask = if f == RtFn::ListPop && is_heap(self.m, payload) {
                    1 << 1
                } else {
                    mask
                };
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
                if self.clif(dst)? == types::F64
                    && args
                        .iter()
                        .all(|a| self.f.regs[a.0 as usize] == self.f.regs[dst.0 as usize]) =>
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
                let v = if t == types::F64 {
                    self.b.ins().fcvt_from_sint(types::F64, n)
                } else {
                    n
                };
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
        let (Some(some), Some(none)) = (def.variant_index("Some"), def.variant_index("None"))
        else {
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
            bits.push(if self.heap[a.0 as usize] {
                v
            } else {
                to_bits(self.b, v)
            });
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

// Emits `if !ok { trap(msg) }`.
