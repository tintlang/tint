use super::*;

impl<'a, 'b> Fc<'a, 'b> {
    pub(super) fn instr_host(&mut self, ins: &Instr) -> Result<(), Unsupported> {
        match ins {
            Instr::Host {
                dst,
                f: f @ (HostFn::Print | HostFn::Println),
                args,
            } => {
                for a in args {
                    self.i32c(self.ty(*a).0 as i32);
                    self.get(*a);
                    let vt = self.vt(*a);
                    self.to_bits(vt);
                    self.i32c(i32::from(*f == HostFn::Println));
                    self.call(Imp::Print);
                }
                self.i32c(0);
                self.set(*dst);
            }
            Instr::Host {
                dst,
                f: HostFn::StorageGetOr,
                args,
            } => {
                self.get(args[0]);
                self.get(args[1]);
                self.call(Imp::StorageGetOr);
                self.put_result(*dst);
            }
            Instr::Host {
                dst,
                f: HostFn::StorageSet,
                args,
            } => {
                self.get(args[0]);
                self.get(args[1]);
                self.call(Imp::StorageSet);
                self.i32c(0);
                self.set(*dst);
            }
            Instr::Host {
                dst,
                f: HostFn::StorageRemove,
                args,
            } => {
                self.get(args[0]);
                self.call(Imp::StorageRemove);
                self.i32c(0);
                self.set(*dst);
            }
            Instr::Host {
                dst,
                f: HostFn::NowMs,
                ..
            } => {
                self.call(Imp::NowMs);
                self.set(*dst);
            }
            Instr::Host {
                dst,
                f: HostFn::HttpGet,
                args,
            } => {
                self.get(args[0]);
                self.call(Imp::HttpGet);
                self.set(*dst);
            }
            Instr::Host {
                dst,
                f: HostFn::Native(index),
                args,
            } => {
                let callback = match args.last() {
                    Some(last) if matches!(self.m.types.kind(self.ty(*last)), TyKind::Fn(..)) => {
                        Some(*last)
                    }
                    _ => None,
                };
                let plain = &args[..args.len() - callback.is_some() as usize];
                let n = plain.len() as i32;
                self.i32c(12 * n.max(1));
                self.call(Imp::RtScratch);
                let buf = self.tmp(I32);
                self.lset(buf);
                for (i, a) in plain.iter().enumerate() {
                    self.lget(buf);
                    self.get(*a);
                    let vt = self.vt(*a);
                    self.to_bits(vt);
                    self.ins(I::I64Store(ma(8 * i as u64, 3)));
                    self.lget(buf);
                    self.i32c(self.ty(*a).0 as i32);
                    self.ins(I::I32Store(ma(8 * n as u64 + 4 * i as u64, 2)));
                }
                self.i32c(*index as i32);
                self.i32c(n);
                self.lget(buf);
                self.lget(buf);
                self.i32c(8 * n);
                self.ins(I::I32Add);
                self.i32c(self.ty(*dst).0 as i32);
                match callback {
                    Some(cb) => {
                        self.get(cb);
                        let TyKind::Fn(params, _) = self.m.types.kind(self.ty(cb)).clone() else {
                            unreachable!()
                        };
                        self.i32c(params[0].0 as i32);
                    }
                    None => {
                        self.i32c(0);
                        self.i32c(0);
                    }
                }
                self.call(Imp::HostNative);
                match callback {
                    None => self.put_bits(*dst),
                    Some(cb) => {
                        // An answer that is already there is delivered now.
                        self.ins(I::I32WrapI64);
                        let answer = self.tmp(I32);
                        self.lset(answer);
                        self.lget(answer);
                        self.ins(I::If(BlockType::Empty));
                        let ty = self.cx.func_type(vec![I32, I32], vec![I32]);
                        let closure = self.lr(cb);
                        self.lget(closure);
                        self.lget(answer);
                        self.lget(closure);
                        self.ins(I::I64Load(ma(slot_off(0), 3)));
                        self.ins(I::I32WrapI64);
                        self.ins(I::CallIndirect {
                            type_index: ty,
                            table_index: 0,
                        });
                        self.ins(I::Drop);
                        self.ins(I::End);
                        self.i32c(0);
                        self.set(*dst);
                    }
                }
            }
            Instr::UiOpen { template, values } => {
                self.i32c(*template as i32);
                self.call(Imp::UiOpen);
                for v in values {
                    match self.m.types.kind(self.ty(*v)) {
                        TyKind::Num(k) if k.is_float() => {
                            self.get(*v);
                            self.call(Imp::UiNum);
                        }
                        TyKind::Num(_) => {
                            self.get(*v);
                            self.ins(I::F64ConvertI64S);
                            self.call(Imp::UiNum);
                        }
                        TyKind::Str => {
                            self.get(*v);
                            self.call(Imp::UiStr);
                        }
                        TyKind::Bool => {
                            self.get(*v);
                            self.call(Imp::UiBool);
                        }
                        _ => return unsupported("non-scalar UI value"),
                    }
                }
            }
            Instr::UiClose => self.call(Imp::UiClose),
            Instr::UiTokens { template } => {
                self.i32c(*template as i32);
                self.call(Imp::UiTokens);
            }
            Instr::UiText { src } => {
                if matches!(self.m.types.kind(self.ty(*src)), TyKind::Str) {
                    self.get(*src);
                    self.call(Imp::UiText);
                } else {
                    let tid = self.ty(*src).0 as i32;
                    self.i32c(tid);
                    self.get(*src);
                    let vt = self.vt(*src);
                    self.to_bits(vt);
                    self.call(Imp::ToStr);
                    let t = self.tmp(I32);
                    self.lset(t);
                    self.lget(t);
                    self.call(Imp::UiText);
                    self.release(t);
                }
            }
            _ => unreachable!(),
        }
        Ok(())
    }
}
