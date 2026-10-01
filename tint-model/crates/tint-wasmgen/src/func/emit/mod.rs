use super::*;

mod analysis;
mod basic;
mod bridge;
mod casts;
mod comparisons;
mod control;
mod core;
mod host;
mod instructions;
mod lists;
mod object;
mod objects;
mod runtime;

use analysis::{block_local_spans, dying_regs};

struct Fc<'a, 'b> {
    cx: &'b mut Cx<'a>,
    m: &'a Module,
    f: &'a Func,
    cfg: Cfg,
    heap: Vec<bool>,
    dies: Vec<Vec<Vec<Reg>>>,
    code: Vec<I<'static>>,
    /// Types of the locals after the parameters, in index order.
    local_types: Vec<ValType>,
    nparams: u32,
    local: Vec<u32>,
    t: [u32; 3],
    tf: u32,
    free: Vec<(ValType, u32)>,
    taken: Vec<(ValType, u32)>,
    dying: Vec<Reg>,
    moved: Vec<Reg>,
}

pub fn compile_func<'a>(cx: &mut Cx<'a>, f: &'a Func) -> Result<Function, Unsupported> {
    let m = cx.m;
    let cfg = Cfg::build(f).map_err(Unsupported)?;
    let heap = heap_regs(m, f);
    let dies = dying_regs(f, &heap);
    let mut fc = Fc {
        m,
        f,
        cx,
        cfg,
        heap,
        dies,
        code: Vec::new(),
        local_types: Vec::new(),
        nparams: f.params.len() as u32,
        local: vec![u32::MAX; f.regs.len()],
        t: [0; 3],
        tf: 0,
        free: Vec::new(),
        taken: Vec::new(),
        dying: Vec::new(),
        moved: Vec::new(),
    };
    for (i, p) in f.params.iter().enumerate() {
        fc.local[p.0 as usize] = i as u32;
    }
    // Registers that live inside one block share locals (huge straight-line
    // functions, e.g. list literals, otherwise get thousands of locals and take
    // V8 seconds to compile).
    let spans = block_local_spans(f, &fc.heap, &fc.dies);
    let mut per_block: Vec<Vec<(usize, usize, usize)>> = vec![Vec::new(); f.blocks.len()];
    for (r, sp) in spans.iter().enumerate() {
        if let Some((b, first, last)) = sp {
            if fc.local[r] == u32::MAX {
                per_block[*b].push((r, *first, *last));
            }
        }
    }
    let mut pool: Vec<(ValType, u32)> = Vec::new();
    for regs in &mut per_block {
        regs.sort_by_key(|(_, first, _)| *first);
        let mut free: Vec<(ValType, u32)> = pool.clone();
        let mut active: Vec<(usize, u32, ValType)> = Vec::new(); // (last, local, type)
        for &(r, first, last) in regs.iter() {
            let mut k = 0;
            while k < active.len() {
                if active[k].0 < first {
                    let (_, l, vt) = active.swap_remove(k);
                    free.push((vt, l));
                } else {
                    k += 1;
                }
            }
            let vt = val_type(m, f.regs[r]);
            let l = match free.iter().position(|(t, _)| *t == vt) {
                Some(pos) => free.swap_remove(pos).1,
                None => {
                    let l = fc.new_local(vt);
                    pool.push((vt, l));
                    l
                }
            };
            fc.local[r] = l;
            active.push((last, l, vt));
        }
    }
    for (i, ty) in f.regs.iter().enumerate() {
        if fc.local[i] == u32::MAX {
            fc.local[i] = fc.new_local(val_type(m, *ty));
        }
    }
    fc.t = [fc.new_local(I64), fc.new_local(I64), fc.new_local(I64)];
    fc.tf = fc.new_local(F64);
    let mut ctx = Vec::new();
    fc.do_tree(0, &mut ctx)?;
    fc.ins(I::Unreachable);
    fc.ins(I::End);
    Ok(fc.finish())
}
