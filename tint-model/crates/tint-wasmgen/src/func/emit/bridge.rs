use super::*;

impl<'a, 'b> Fc<'a, 'b> {
    pub(super) fn bridge(&mut self, dst: Reg, f: RtFn, args: &[Reg]) -> Result<(), Unsupported> {
        let n = args.len() as i32;
        self.i32c(12 * n.max(1));
        self.call(Imp::RtScratch);
        let buf = self.tmp(I32);
        self.lset(buf);
        for (i, a) in args.iter().enumerate() {
            self.lget(buf);
            self.get(*a);
            let vt = self.vt(*a);
            self.to_bits(vt);
            self.ins(I::I64Store(ma(8 * i as u64, 3)));
            self.lget(buf);
            self.i32c(self.ty(*a).0 as i32);
            self.ins(I::I32Store(ma(8 * n as u64 + 4 * i as u64, 2)));
        }
        self.i32c(rt_fn_index(f) as i32);
        self.i32c(n);
        self.lget(buf);
        self.lget(buf);
        self.i32c(8 * n);
        self.ins(I::I32Add);
        self.i32c(self.ty(dst).0 as i32);
        self.call(Imp::Call);
        let out = self.tmp(I64);
        self.lset(out);
        if f.mutates_first() {
            let nb = self.tmp(I32);
            self.lget(buf);
            self.ins(I::I32Load(ma(0, 2)));
            self.lset(nb);
            self.assign(args[0], nb);
        }
        self.lget(out);
        self.put_bits(dst);
        Ok(())
    }
}
