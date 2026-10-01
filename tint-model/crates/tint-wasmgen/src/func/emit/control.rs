use super::analysis::depth;
use super::*;

impl<'a, 'b> Fc<'a, 'b> {
    pub(super) fn do_tree(&mut self, x: usize, ctx: &mut Vec<Frame>) -> Result<(), Unsupported> {
        let looped = self.cfg.is_loop_header[x];
        if looped {
            self.ins(I::Loop(BlockType::Empty));
            ctx.push(Frame::Loop(x));
        }
        let ys = self.cfg.merge_children[x].clone();
        self.node_within(x, ys, ctx)?;
        if looped {
            ctx.pop();
            self.ins(I::End);
        }
        Ok(())
    }

    pub(super) fn node_within(
        &mut self,
        x: usize,
        mut ys: Vec<usize>,
        ctx: &mut Vec<Frame>,
    ) -> Result<(), Unsupported> {
        match ys.pop() {
            Some(y) => {
                self.ins(I::Block(BlockType::Empty));
                ctx.push(Frame::Block(y));
                self.node_within(x, ys, ctx)?;
                ctx.pop();
                self.ins(I::End);
                self.do_tree(y, ctx)
            }
            None => self.block_code(x, ctx),
        }
    }

    pub(super) fn do_branch(
        &mut self,
        from: usize,
        to: usize,
        ctx: &mut Vec<Frame>,
    ) -> Result<(), Unsupported> {
        if self.cfg.is_back_edge(from, to) {
            let d = depth(ctx, Frame::Loop(to));
            self.ins(I::Br(d));
            Ok(())
        } else if self.cfg.is_merge[to] {
            let d = depth(ctx, Frame::Block(to));
            self.ins(I::Br(d));
            Ok(())
        } else {
            self.do_tree(to, ctx)
        }
    }

    pub(super) fn block_code(&mut self, x: usize, ctx: &mut Vec<Frame>) -> Result<(), Unsupported> {
        let f = self.f;
        let block = &f.blocks[x];
        for (ii, ins) in block.instrs.iter().enumerate() {
            self.dying = self.dies[x][ii].clone();
            self.moved.clear();
            self.instr(ins)?;
            // A heap register whose last use this was gives its reference back now.
            for r in std::mem::take(&mut self.dying) {
                if !self.moved.contains(&r) {
                    let l = self.lr(r);
                    self.release(l);
                    self.clear(r);
                }
            }
            self.end_instr();
        }
        match &block.term {
            Term::Jump(t) => self.do_branch(x, t.0 as usize, ctx),
            Term::Branch { cond, then_, else_ } => {
                self.get(*cond);
                self.ins(I::If(BlockType::Empty));
                ctx.push(Frame::If);
                self.do_branch(x, then_.0 as usize, ctx)?;
                ctx.pop();
                self.ins(I::Else);
                ctx.push(Frame::If);
                self.do_branch(x, else_.0 as usize, ctx)?;
                ctx.pop();
                self.ins(I::End);
                Ok(())
            }
            Term::Switch {
                value,
                cases,
                default,
            } => {
                for (c, target) in cases {
                    self.get(*value);
                    self.i64c(*c);
                    self.ins(I::I64Eq);
                    self.ins(I::If(BlockType::Empty));
                    ctx.push(Frame::If);
                    self.do_branch(x, target.0 as usize, ctx)?;
                    ctx.pop();
                    self.ins(I::End);
                }
                self.do_branch(x, default.0 as usize, ctx)
            }
            Term::Return(r) => {
                let rt = self.vt(*r);
                let tv = self.tmp(rt);
                self.get(*r);
                self.lset(tv);
                if self.heap[r.0 as usize] {
                    // The reference moves to the caller.
                    self.clear(*r);
                }
                for i in 0..self.heap.len() {
                    if self.heap[i] {
                        let l = self.local[i];
                        self.release(l);
                    }
                }
                self.lget(tv);
                self.ins(I::Return);
                self.end_instr();
                Ok(())
            }
            Term::Trap(msg) => {
                let id = self.cx.msg(msg);
                self.i32c(id);
                self.call(Imp::Trap);
                self.ins(I::Unreachable);
                Ok(())
            }
        }
    }
}
