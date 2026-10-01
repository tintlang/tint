impl<'a, 'b> L<'a, 'b> {
    fn instr(&mut self, ins: &Instr) -> Result<(), Unsupported> {
        match ins {
            Instr::Const { dst, value } => {
                self.clif(*dst)?;
                let v = match value {
                    Const::Unit => self.b.ins().iconst(types::I8, 0),
                    Const::Bool(x) => self.b.ins().iconst(types::I8, *x as i64),
                    Const::Int(i) => {
                        if matches!(self.m.types.kind(self.ty(*dst)), TyKind::Num(k) if k.is_float())
                        {
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
            Instr::Bin {
                dst,
                op,
                kind,
                a,
                b: rb,
            } => {
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
                let (from, to) = match (
                    self.m.types.kind(self.ty(*src)),
                    self.m.types.kind(self.ty(*dst)),
                ) {
                    (TyKind::Num(a), TyKind::Num(b)) => (*a, *b),
                    _ => return unsupported("cast of a non-number"),
                };
                let x = self.get(*src);
                let bits = to_bits(self.b, x);
                let (f, t) = (rt::num_kind_index(from), rt::num_kind_index(to));
                let (f, t) = (
                    self.b.ins().iconst(types::I32, f),
                    self.b.ins().iconst(types::I32, t),
                );
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
                    let bits = if self.heap[src.0 as usize] {
                        x
                    } else {
                        to_bits(self.b, x)
                    };
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
            Instr::Variant {
                dst,
                adt,
                variant,
                fields,
            } if fields.is_empty() => {
                // A variant without fields is one shared object that is never freed.
                self.clif(*dst)?;
                let n = crate::adt_layout_slots(self.m, *adt);
                let p = rt::immortal_variant(n, *variant as u64);
                let v = self.iconst(p as i64);
                self.assign(*dst, v);
            }
            Instr::Variant {
                dst,
                adt,
                variant,
                fields,
            } => {
                let tys: Vec<TyId> = fields.iter().map(|r| self.ty(*r)).collect();
                let n = crate::adt_layout_slots(self.m, *adt);
                self.build_obj(
                    *dst,
                    fields,
                    n,
                    ptr_mask(self.m, &tys, 1),
                    1,
                    Some(*variant as i64),
                )?;
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
            Instr::Payload {
                dst, src, index, ..
            } => {
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
            Instr::Host {
                dst,
                f: f @ (HostFn::Print | HostFn::Println),
                args,
            } => {
                for a in args {
                    self.clif(*a)?;
                    let v = self.get(*a);
                    let bits = if self.heap[a.0 as usize] {
                        v
                    } else {
                        to_bits(self.b, v)
                    };
                    let tid = self.ty(*a).0 as i64;
                    let ty = self.iconst(tid);
                    let nl = self.iconst(i64::from(*f == HostFn::Println));
                    self.call("print", &[ty, bits, nl]);
                }
                let unit = self.b.ins().iconst(types::I8, 0);
                self.set(*dst, unit);
            }
            Instr::Closure {
                dst,
                func,
                captures,
            } => {
                let tys: Vec<TyId> = captures.iter().map(|r| self.ty(*r)).collect();
                let n = 1 + captures.len();
                self.build_obj(
                    *dst,
                    captures,
                    n,
                    ptr_mask(self.m, &tys, 1),
                    1,
                    Some(func.0 as i64),
                )?;
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

    // Allocates a struct, tuple or enum object and fills its slots.
}
