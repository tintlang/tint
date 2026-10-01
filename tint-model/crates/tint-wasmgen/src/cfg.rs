//! Control-flow analysis for turning a block graph into WebAssembly's
//! structured control flow ("Beyond Relooper", Ramsey 2022): dominator tree,
//! loop headers, merge nodes. Irreducible graphs are rejected.

use tint_ir::typed::{Func, Term};

pub struct Cfg {
    /// Reverse-postorder number of each reachable block.
    pub rpo: Vec<usize>,
    pub is_loop_header: Vec<bool>,
    pub is_merge: Vec<bool>,
    /// Dominator-tree children that are merge nodes, by ascending `rpo`.
    pub merge_children: Vec<Vec<usize>>,
}

impl Cfg {
    pub fn is_back_edge(&self, from: usize, to: usize) -> bool {
        self.rpo[to] <= self.rpo[from]
    }

    pub fn build(f: &Func) -> Result<Cfg, String> {
        let n = f.blocks.len();
        let succ: Vec<Vec<usize>> = f.blocks.iter().map(|b| term_succ(&b.term)).collect();
        // Postorder by iterative DFS from the entry.
        let mut reachable = vec![false; n];
        let mut post = Vec::new();
        let mut stack = vec![(0usize, 0usize)];
        reachable[0] = true;
        while let Some(&mut (b, ref mut i)) = stack.last_mut() {
            if *i < succ[b].len() {
                let s = succ[b][*i];
                *i += 1;
                if !reachable[s] {
                    reachable[s] = true;
                    stack.push((s, 0));
                }
            } else {
                post.push(b);
                stack.pop();
            }
        }
        let mut rpo = vec![usize::MAX; n];
        for (i, b) in post.iter().rev().enumerate() {
            rpo[*b] = i;
        }
        let order: Vec<usize> = post.iter().rev().copied().collect();

        // Predecessors among reachable blocks.
        let mut preds = vec![Vec::new(); n];
        for &b in &order {
            for &s in &succ[b] {
                preds[s].push(b);
            }
        }
        // Cooper-Harvey-Kennedy dominators.
        let mut idom = vec![usize::MAX; n];
        idom[0] = 0;
        let intersect = |idom: &Vec<usize>, mut a: usize, mut b: usize| {
            while a != b {
                while rpo[a] > rpo[b] {
                    a = idom[a];
                }
                while rpo[b] > rpo[a] {
                    b = idom[b];
                }
            }
            a
        };
        let mut changed = true;
        while changed {
            changed = false;
            for &b in order.iter().skip(1) {
                let mut new = usize::MAX;
                for &p in &preds[b] {
                    if idom[p] == usize::MAX {
                        continue;
                    }
                    new = if new == usize::MAX {
                        p
                    } else {
                        intersect(&idom, p, new)
                    };
                }
                if idom[b] != new {
                    idom[b] = new;
                    changed = true;
                }
            }
        }
        let dominates = |a: usize, mut b: usize| loop {
            if a == b {
                return true;
            }
            if b == 0 {
                return false;
            }
            b = idom[b];
        };

        let mut is_loop_header = vec![false; n];
        let mut fwd_in = vec![0usize; n];
        for &b in &order {
            for &s in &succ[b] {
                if rpo[s] <= rpo[b] {
                    if !dominates(s, b) {
                        return Err("irreducible control flow".into());
                    }
                    is_loop_header[s] = true;
                } else {
                    fwd_in[s] += 1;
                }
            }
        }
        let is_merge: Vec<bool> = fwd_in.iter().map(|c| *c >= 2).collect();
        let mut merge_children = vec![Vec::new(); n];
        for &b in order.iter().skip(1) {
            if is_merge[b] {
                merge_children[idom[b]].push(b);
            }
        }
        for kids in &mut merge_children {
            kids.sort_by_key(|b| rpo[*b]);
        }
        Ok(Cfg {
            rpo,
            is_loop_header,
            is_merge,
            merge_children,
        })
    }
}

pub fn term_succ(t: &Term) -> Vec<usize> {
    t.successors().iter().map(|b| b.0 as usize).collect()
}
