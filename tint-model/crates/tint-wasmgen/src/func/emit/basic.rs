use super::*;

impl<'a, 'b> Fc<'a, 'b> {
    pub(super) fn instr_basic(&mut self, ins: &Instr) -> Result<(), Unsupported> {
        match ins {
            Instr::Const { dst, value } => {
                let vt = self.vt(*dst);
                match (value, vt) {
                    (Const::Unit, _) => self.i32c(0),
                    (Const::Bool(b), _) => self.i32c(*b as i32),
                    (Const::Int(i), ValType::F64) => self.ins(I::F64Const((*i as f64).into())),
                    (Const::Int(i), _) => self.i64c(*i),
                    (Const::Float(x), _) => self.ins(I::F64Const((*x).into())),
                    (Const::Str(s), _) => {
                        let g = self.cx.extra(ExtraKey::Lit(s.clone()));
                        let t = self.tmp(I32);
                        self.ins(I::GlobalGet(g));
                        self.lset(t);
                        self.retain(t);
                        self.assign(*dst, t);
                        return Ok(());
                    }
                }
                self.set(*dst);
            }
            Instr::Mov { dst, src } => {
                if self.heap[dst.0 as usize] {
                    let v = self.own(*src, &[]);
                    self.assign(*dst, v);
                } else {
                    self.get(*src);
                    self.set(*dst);
                }
            }
            Instr::Bin {
                dst,
                op,
                kind,
                a,
                b,
            } => {
                if kind.is_float() {
                    self.get(*a);
                    self.get(*b);
                    match op {
                        BinOp::Add => self.ins(I::F64Add),
                        BinOp::Sub => self.ins(I::F64Sub),
                        BinOp::Mul => self.ins(I::F64Mul),
                        BinOp::Div => self.ins(I::F64Div),
                        BinOp::Rem => self.call(Imp::Fmod),
                    }
                    if *kind == NumKind::F32 {
                        self.round_f32();
                    }
                } else {
                    self.get(*a);
                    self.ins(I::LocalSet(self.t[0]));
                    self.get(*b);
                    self.ins(I::LocalSet(self.t[1]));
                    self.int_arith(*op, *kind);
                }
                self.set(*dst);
            }
            Instr::Neg { dst, src } => {
                let kind = match self.m.types.kind(self.ty(*src)) {
                    TyKind::Num(k) => *k,
                    _ => return unsupported("negation of a non-number"),
                };
                if kind.is_float() {
                    self.get(*src);
                    self.ins(I::F64Neg);
                } else {
                    let (t0, t1) = (self.t[0], self.t[1]);
                    self.i64c(0);
                    self.lset(t0);
                    self.get(*src);
                    self.lset(t1);
                    self.int_arith(BinOp::Sub, kind);
                }
                self.set(*dst);
            }
            Instr::Not { dst, src } => {
                self.get(*src);
                self.ins(I::I32Eqz);
                self.set(*dst);
            }
            Instr::Cmp { dst, op, a, b } => {
                self.cmp(*op, *a, *b)?;
                self.set(*dst);
            }
            Instr::Cast { dst, src } => {
                let from = self.m.types.as_num(self.ty(*src));
                let to = self.m.types.as_num(self.ty(*dst));
                let (Some(from), Some(to)) = (from, to) else {
                    return unsupported("cast of a non-number");
                };
                self.cast(*src, from, to);
                self.set(*dst);
            }
            Instr::ToStr { dst, src } => {
                if matches!(self.m.types.kind(self.ty(*src)), TyKind::Str) {
                    let v = self.own(*src, &[]);
                    self.assign(*dst, v);
                } else {
                    let tid = self.ty(*src).0 as i32;
                    self.i32c(tid);
                    self.get(*src);
                    let vt = self.vt(*src);
                    self.to_bits(vt);
                    self.call(Imp::ToStr);
                    let t = self.tmp(I32);
                    self.lset(t);
                    self.assign(*dst, t);
                }
            }
            Instr::Tuple { dst, items } => {
                let tys: Vec<TyId> = items.iter().map(|r| self.ty(*r)).collect();
                self.build_obj(
                    *dst,
                    items,
                    tys.len() as u32,
                    ptr_mask(self.m, &tys, 0),
                    0,
                    None,
                );
            }
            Instr::Struct { dst, adt, fields } => {
                let tys: Vec<TyId> = fields.iter().map(|r| self.ty(*r)).collect();
                let n = adt_slots(self.m, *adt);
                self.build_obj(*dst, fields, n, ptr_mask(self.m, &tys, 0), 0, None);
            }
            Instr::Variant {
                dst,
                adt,
                variant,
                fields,
            } if fields.is_empty() => {
                let n = adt_slots(self.m, *adt);
                let g = self.cx.extra(ExtraKey::Variant(n, *variant));
                let t = self.tmp(I32);
                self.ins(I::GlobalGet(g));
                self.lset(t);
                self.retain(t);
                self.assign(*dst, t);
            }
            Instr::Variant {
                dst,
                adt,
                variant,
                fields,
            } => {
                let tys: Vec<TyId> = fields.iter().map(|r| self.ty(*r)).collect();
                let n = adt_slots(self.m, *adt);
                self.build_obj(
                    *dst,
                    fields,
                    n,
                    ptr_mask(self.m, &tys, 1),
                    1,
                    Some(*variant as i64),
                );
            }
            Instr::List { dst, items } => {
                let lay = match self.m.types.kind(self.ty(*dst)) {
                    TyKind::List(e) => elem_layout(self.m, *e),
                    _ => return unsupported("list literal of a non-list"),
                };
                self.i32c(items.len() as i32);
                self.i32c(lay.size as i32);
                self.i32c(lay.heap as i32);
                self.call(Imp::ListNew);
                let list = self.tmp(I32);
                self.lset(list);
                for (i, item) in items.iter().enumerate() {
                    let bits = self.slot_bits(*item, &items[i + 1..]);
                    self.lget(list);
                    self.lget(bits);
                    self.call(Imp::ListPush);
                    self.ins(I::Drop);
                }
                self.assign(*dst, list);
            }
            _ => unreachable!(),
        }
        Ok(())
    }
}
