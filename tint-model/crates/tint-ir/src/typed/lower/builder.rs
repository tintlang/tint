//! Function under construction: registers, blocks, scopes, loop targets and
//! the closure captures found so far.

use super::*;

pub(crate) struct Capture {
    /// Register in the enclosing function that supplies the value.
    pub outer: Reg,
    /// The lambda's own register that holds it.
    pub inner: Reg,
}

pub(crate) struct FnB {
    pub name: String,
    pub kind: FuncKind,
    pub params: Vec<Reg>,
    pub regs: Vec<TyId>,
    pub blocks: Vec<(Vec<Instr>, Option<Term>)>,
    pub cur: usize,
    pub scopes: Vec<HashMap<String, Reg>>,
    /// (break target, continue target) of the enclosing loops.
    pub loops: Vec<(BlockId, BlockId)>,
    /// What `return` and `?` return, as the caller sees it.
    pub ret: TyId,
    pub inout: bool,
    pub self_reg: Option<Reg>,
    pub captures: Vec<Capture>,
    pub is_lambda: bool,
}

impl FnB {
    pub fn new(name: String, kind: FuncKind, ret: TyId) -> FnB {
        FnB {
            name,
            kind,
            params: Vec::new(),
            regs: Vec::new(),
            blocks: vec![(Vec::new(), None)],
            cur: 0,
            scopes: vec![HashMap::new()],
            loops: Vec::new(),
            ret,
            inout: false,
            self_reg: None,
            captures: Vec::new(),
            is_lambda: false,
        }
    }

    pub fn new_reg(&mut self, ty: TyId) -> Reg {
        self.regs.push(ty);
        Reg(self.regs.len() as u32 - 1)
    }

    pub fn reg_ty(&self, reg: Reg) -> TyId {
        self.regs[reg.0 as usize]
    }

    pub fn define(&mut self, name: &str, reg: Reg) {
        self.scopes.last_mut().unwrap().insert(name.to_string(), reg);
    }

    pub fn lookup_local(&self, name: &str) -> Option<Reg> {
        self.scopes.iter().rev().find_map(|s| s.get(name).copied())
    }

    /// Assembles the finished function. `ret` is the type it returns.
    pub fn finish(mut self, ret: TyId) -> Func {
        prune(&mut self);
        let ncaptures = self.captures.len() as u32;
        let mut params: Vec<Reg> = self.captures.iter().map(|c| c.inner).collect();
        params.extend(self.params.iter().copied());
        Func {
            name: self.name,
            kind: self.kind,
            params,
            ncaptures,
            ret,
            regs: self.regs,
            blocks: self
                .blocks
                .into_iter()
                .map(|(instrs, term)| Block {
                    instrs,
                    term: term.unwrap_or_else(|| Term::Trap("fell off the end of a block".into())),
                })
                .collect(),
        }
    }
}

/// Drops blocks nothing jumps to and renumbers the rest.
fn prune(fb: &mut FnB) {
    let count = fb.blocks.len();
    let mut reachable = vec![false; count];
    let mut work = vec![0usize];
    while let Some(b) = work.pop() {
        if std::mem::replace(&mut reachable[b], true) {
            continue;
        }
        if let Some(term) = &fb.blocks[b].1 {
            for next in term.successors() {
                work.push(next.0 as usize);
            }
        }
    }
    let mut remap = vec![u32::MAX; count];
    let mut next = 0u32;
    for (i, r) in reachable.iter().enumerate() {
        if *r {
            remap[i] = next;
            next += 1;
        }
    }
    let fix = |b: &mut BlockId| b.0 = remap[b.0 as usize];
    let old = std::mem::take(&mut fb.blocks);
    for (i, (instrs, mut term)) in old.into_iter().enumerate() {
        if !reachable[i] {
            continue;
        }
        match &mut term {
            Some(Term::Jump(b)) => fix(b),
            Some(Term::Branch { then_, else_, .. }) => {
                fix(then_);
                fix(else_);
            }
            Some(Term::Switch { cases, default, .. }) => {
                for (_, b) in cases.iter_mut() {
                    fix(b);
                }
                fix(default);
            }
            _ => {}
        }
        fb.blocks.push((instrs, term));
    }
}

impl<'a> Lowerer<'a> {
    pub fn f(&mut self) -> &mut FnB {
        self.stack.last_mut().expect("no function under construction")
    }

    pub fn new_reg(&mut self, ty: TyId) -> Reg {
        self.f().new_reg(ty)
    }

    pub fn reg_ty(&self, reg: Reg) -> TyId {
        self.stack.last().unwrap().reg_ty(reg)
    }

    pub fn new_block(&mut self) -> BlockId {
        let fb = self.f();
        fb.blocks.push((Vec::new(), None));
        BlockId(fb.blocks.len() as u32 - 1)
    }

    pub fn switch_to(&mut self, block: BlockId) {
        self.f().cur = block.0 as usize;
    }

    /// After a `return` or `break` the code that follows is unreachable;
    /// it gets a block of its own, which is pruned later.
    fn ensure_open(&mut self) {
        let closed = {
            let fb = self.f();
            fb.blocks[fb.cur].1.is_some()
        };
        if closed {
            let b = self.new_block();
            self.switch_to(b);
        }
    }

    pub fn emit(&mut self, instr: Instr) {
        self.ensure_open();
        let fb = self.f();
        let cur = fb.cur;
        fb.blocks[cur].0.push(instr);
    }

    pub fn terminate(&mut self, term: Term) {
        self.ensure_open();
        let fb = self.f();
        let cur = fb.cur;
        fb.blocks[cur].1 = Some(term);
    }

    pub fn push_scope(&mut self) {
        self.f().scopes.push(HashMap::new());
    }

    pub fn pop_scope(&mut self) {
        self.f().scopes.pop();
    }

    // ------------------------------------------------- small emitters

    pub fn unit_ty(&mut self) -> TyId {
        self.module.types.unit()
    }

    pub fn unit_reg(&mut self) -> Reg {
        let ty = self.unit_ty();
        let dst = self.new_reg(ty);
        self.emit(Instr::Const { dst, value: Const::Unit });
        dst
    }

    pub fn const_reg(&mut self, ty: TyId, value: Const) -> Reg {
        let dst = self.new_reg(ty);
        self.emit(Instr::Const { dst, value });
        dst
    }

    /// A register of type `ty` that nothing wrote to. Used as the value of
    /// code that never completes.
    pub fn undef(&mut self, ty: TyId) -> Reg {
        self.new_reg(ty)
    }

    pub fn mov(&mut self, src: Reg) -> Reg {
        let ty = self.reg_ty(src);
        let dst = self.new_reg(ty);
        self.emit(Instr::Mov { dst, src });
        dst
    }

    /// A block that stops the program; branch to it for impossible cases.
    pub fn trap_block(&mut self, message: &str) -> BlockId {
        let block = self.new_block();
        let fb = self.f();
        fb.blocks[block.0 as usize].1 = Some(Term::Trap(message.to_string()));
        block
    }

    pub fn tk(&self, ty: TyId) -> TyKind {
        self.module.types.kind(ty).clone()
    }

    pub fn show(&self, ty: TyId) -> String {
        self.module.types.show(ty)
    }

    pub fn bool_ty(&mut self) -> TyId {
        self.module.types.bool()
    }

    pub fn str_ty(&mut self) -> TyId {
        self.module.types.str()
    }

    pub fn num_ty(&mut self, kind: NumKind) -> TyId {
        self.module.types.num(kind)
    }

    /// Emits a runtime call producing `ty`.
    pub fn rt(&mut self, f: RtFn, args: Vec<Reg>, ty: TyId) -> Reg {
        let dst = self.new_reg(ty);
        self.emit(Instr::Rt { dst, f, args });
        dst
    }

    // ----------------------------------------------------- name lookup

    /// Finds a local, capturing it from enclosing functions when this is a
    /// lambda.
    pub fn resolve_local(&mut self, name: &str) -> Option<Reg> {
        let level = self.stack.len() - 1;
        self.resolve_at(level, name)
    }

    fn resolve_at(&mut self, level: usize, name: &str) -> Option<Reg> {
        if let Some(r) = self.stack[level].lookup_local(name) {
            return Some(r);
        }
        if level == 0 || !self.stack[level].is_lambda {
            return None;
        }
        let outer = self.resolve_at(level - 1, name)?;
        let ty = self.stack[level - 1].reg_ty(outer);
        let inner = self.stack[level].new_reg(ty);
        self.stack[level].captures.push(Capture { outer, inner });
        self.stack[level].scopes[0].insert(name.to_string(), inner);
        Some(inner)
    }
}
