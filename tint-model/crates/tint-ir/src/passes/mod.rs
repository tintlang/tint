use crate::ir::{FunctionIR, ProgramIR};

mod dead_code;
mod folding;

pub fn optimize(prog: &mut ProgramIR) {
    for f in &mut prog.functions {
        optimize_function(f);
    }
}

fn optimize_function(func: &mut FunctionIR) {
    let mut changed = true;

    while changed {
        changed = false;

        changed |= folding::constant_folding(func);
        changed |= dead_code::dead_code_elimination(func);
    }
}
