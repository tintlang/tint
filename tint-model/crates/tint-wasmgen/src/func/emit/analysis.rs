use super::*;

pub(super) fn depth(ctx: &[Frame], want: Frame) -> u32 {
    let pos = ctx
        .iter()
        .rposition(|f| *f == want)
        .expect("branch target is in scope");
    (ctx.len() - 1 - pos) as u32
}

/// For every instruction, the heap registers it reads for the last time: they
/// are not read again on any path before being written. Their reference is
/// given back right after the instruction (or moved into its result).
/// For each register that is defined, used and dead inside a single block: the
/// block and the instruction indices of its first and last occurrence (a use
/// by the terminator counts as the instruction after the last one).
pub(super) fn block_local_spans(
    f: &Func,
    heap: &[bool],
    dies: &[Vec<Vec<Reg>>],
) -> Vec<Option<(usize, usize, usize)>> {
    let n = f.regs.len();
    // (block, first, last, first_is_def, single_block)
    let mut info: Vec<Option<(usize, usize, usize, bool, bool)>> = vec![None; n];
    let note = |info: &mut Vec<Option<(usize, usize, usize, bool, bool)>>,
                r: Reg,
                b: usize,
                i: usize,
                is_def: bool| {
        let e = &mut info[r.0 as usize];
        match e {
            None => *e = Some((b, i, i, is_def, true)),
            Some((eb, _, last, _, single)) => {
                if *eb != b {
                    *single = false;
                } else {
                    *last = (*last).max(i);
                }
            }
        }
    };
    for (b, block) in f.blocks.iter().enumerate() {
        for (i, ins) in block.instrs.iter().enumerate() {
            // Uses come first: an instruction reads before it writes.
            for r in ins.uses() {
                note(&mut info, r, b, i, false);
            }
            if let Some(d) = ins.dst() {
                note(&mut info, d, b, i, true);
            }
        }
        let len = block.instrs.len();
        match &block.term {
            Term::Branch { cond, .. } => note(&mut info, *cond, b, len, false),
            Term::Switch { value, .. } => note(&mut info, *value, b, len, false),
            Term::Return(r) => note(&mut info, *r, b, len, false),
            _ => {}
        }
    }
    let mut out = vec![None; n];
    for (r, e) in info.iter().enumerate() {
        let Some((b, first, last, first_is_def, single)) = *e else {
            continue;
        };
        if !single || !first_is_def {
            continue;
        }
        // First occurrence must be a pure def (not also a use of the same instruction).
        if f.blocks[b]
            .instrs
            .get(first)
            .is_some_and(|ins| ins.uses().iter().any(|u| u.0 as usize == r))
        {
            continue;
        }
        if f.params.iter().any(|p| p.0 as usize == r) {
            continue;
        }
        if heap[r] {
            // Must give its reference back inside the block, so the local is empty afterwards.
            let ends_at_term = last == f.blocks[b].instrs.len();
            if ends_at_term || !dies[b][last].iter().any(|d| d.0 as usize == r) {
                continue;
            }
        }
        out[r] = Some((b, first, last));
    }
    out
}

pub(super) fn dying_regs(f: &Func, heap: &[bool]) -> Vec<Vec<Vec<Reg>>> {
    let n = f.regs.len();
    let term_uses = |t: &Term| -> Vec<Reg> {
        match t {
            Term::Branch { cond, .. } => vec![*cond],
            Term::Switch { value, .. } => vec![*value],
            Term::Return(r) => vec![*r],
            _ => vec![],
        }
    };
    let mut live_in = vec![vec![false; n]; f.blocks.len()];
    let live_out_of = |live_in: &Vec<Vec<bool>>, bi: usize| -> Vec<bool> {
        let mut out = vec![false; n];
        for succ in f.blocks[bi].term.successors() {
            for (o, i) in out.iter_mut().zip(&live_in[succ.0 as usize]) {
                *o |= *i;
            }
        }
        out
    };
    let mut changed = true;
    while changed {
        changed = false;
        for bi in (0..f.blocks.len()).rev() {
            let mut live = live_out_of(&live_in, bi);
            for r in term_uses(&f.blocks[bi].term) {
                live[r.0 as usize] = true;
            }
            for ins in f.blocks[bi].instrs.iter().rev() {
                if let Some(d) = ins.dst() {
                    live[d.0 as usize] = false;
                }
                for r in ins.uses() {
                    live[r.0 as usize] = true;
                }
            }
            if live != live_in[bi] {
                live_in[bi] = live;
                changed = true;
            }
        }
    }
    let mut out = Vec::new();
    for bi in 0..f.blocks.len() {
        let mut live = live_out_of(&live_in, bi);
        for r in term_uses(&f.blocks[bi].term) {
            live[r.0 as usize] = true;
        }
        let mut per_instr = vec![Vec::new(); f.blocks[bi].instrs.len()];
        for (ii, ins) in f.blocks[bi].instrs.iter().enumerate().rev() {
            let dst = ins.dst();
            let mut dead: Vec<Reg> = Vec::new();
            for r in ins.uses() {
                if heap[r.0 as usize] && !live[r.0 as usize] && Some(r) != dst && !dead.contains(&r)
                {
                    dead.push(r);
                }
            }
            per_instr[ii] = dead;
            if let Some(d) = dst {
                live[d.0 as usize] = false;
            }
            for r in ins.uses() {
                live[r.0 as usize] = true;
            }
        }
        out.push(per_instr);
    }
    out
}
