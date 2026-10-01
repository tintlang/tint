use super::*;

impl<'a, 'b> Fc<'a, 'b> {
    pub(super) fn instr_objects(&mut self, ins: &Instr) -> Result<(), Unsupported> {
        match ins {
            Instr::Get { dst, base, proj } => self.get_proj(*dst, *base, proj, false)?,
            Instr::Take { dst, base, proj } => self.get_proj(*dst, *base, proj, true)?,
            Instr::Set { base, proj, src } => self.set_proj(*base, proj, *src)?,
            Instr::Tag { dst, src } => {
                self.get(*src);
                self.ins(I::I64Load(ma(slot_off(0), 3)));
                self.set(*dst);
            }
            Instr::Payload {
                dst, src, index, ..
            } => {
                let obj = self.lr(*src);
                self.load_slot(obj, 1 + *index as usize, *dst);
                if self.heap[dst.0 as usize] {
                    let t = self.tmp(I32);
                    self.lset(t);
                    self.retain(t);
                    self.assign(*dst, t);
                } else {
                    self.set(*dst);
                }
            }
            Instr::Rt { dst, f, args } => self.rt(*dst, *f, args)?,
            Instr::Call { dst, func, args } => {
                let Some(callee) = self.cx.index.get(func).copied() else {
                    return unsupported(format!("call to `{}`", self.m.func(*func).name));
                };
                let mut locals = Vec::new();
                for (k, a) in args.iter().enumerate() {
                    // The callee owns its parameters: hand it one reference each.
                    locals.push(self.own(*a, &args[k + 1..]));
                }
                for l in locals {
                    self.lget(l);
                }
                self.ins(I::Call(callee));
                self.put_result(*dst);
            }
            Instr::Closure {
                dst,
                func,
                captures,
            } => {
                let tys: Vec<TyId> = captures.iter().map(|r| self.ty(*r)).collect();
                let n = 1 + captures.len() as u32;
                self.build_obj(
                    *dst,
                    captures,
                    n,
                    ptr_mask(self.m, &tys, 1),
                    1,
                    Some(func.0 as i64),
                );
            }
            Instr::CallClosure { dst, callee, args } => {
                let mut params = vec![I32];
                params.extend(args.iter().map(|a| self.vt(*a)));
                let ret = self.vt(*dst);
                let ty = self.cx.func_type(params, vec![ret]);
                let mut locals = Vec::new();
                for (k, a) in args.iter().enumerate() {
                    locals.push(self.own(*a, &args[k + 1..]));
                }
                let closure = self.lr(*callee);
                self.lget(closure);
                for l in locals {
                    self.lget(l);
                }
                self.lget(closure);
                self.ins(I::I64Load(ma(slot_off(0), 3)));
                self.ins(I::I32WrapI64);
                self.ins(I::CallIndirect {
                    type_index: ty,
                    table_index: 0,
                });
                self.put_result(*dst);
            }
            Instr::GlobalGet { dst, global } => {
                self.ins(I::GlobalGet(global.0));
                if self.heap[dst.0 as usize] {
                    let t = self.tmp(I32);
                    self.lset(t);
                    self.retain(t);
                    self.assign(*dst, t);
                } else {
                    self.set(*dst);
                }
            }
            Instr::GlobalSet { global, src } => {
                if self.heap[src.0 as usize] {
                    let old = self.tmp(I32);
                    self.ins(I::GlobalGet(global.0));
                    self.lset(old);
                    let v = self.own(*src, &[]);
                    self.lget(v);
                    self.ins(I::GlobalSet(global.0));
                    self.release(old);
                } else {
                    self.get(*src);
                    self.ins(I::GlobalSet(global.0));
                }
            }
            Instr::Map { dst, entries } => {
                let heap = match self.m.types.kind(self.ty(*dst)) {
                    TyKind::Map(v) => is_heap(self.m, *v),
                    _ => return unsupported("map literal of a non-map"),
                };
                self.i32c(heap as i32);
                self.call(Imp::MapNew);
                let map = self.tmp(I32);
                self.lset(map);
                for (i, (key, reg)) in entries.iter().enumerate() {
                    let g = self.cx.extra(ExtraKey::Lit(key.clone()));
                    let rest: Vec<Reg> = entries[i + 1..].iter().map(|(_, r)| *r).collect();
                    let bits = self.slot_bits(*reg, &rest);
                    self.lget(map);
                    self.ins(I::GlobalGet(g));
                    self.lget(bits);
                    self.call(Imp::MapSet);
                    self.lset(map);
                }
                self.assign(*dst, map);
            }
            _ => unreachable!(),
        }
        Ok(())
    }
}
