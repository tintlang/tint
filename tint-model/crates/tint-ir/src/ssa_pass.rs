// =============================================================
// SSA Optimization Passes (compatible with current TintIR)
// =============================================================

use crate::ir::{ProgramIR, FunctionIR, Block, Instr, Value};

// =============================================================
// PUBLIC ENTRY POINT
// =============================================================
pub fn optimize(prog: &mut ProgramIR) {
    for f in &mut prog.functions {
        optimize_function(f);
    }
}

// =============================================================
// OPTIMIZE ONE FUNCTION
// =============================================================
fn optimize_function(func: &mut FunctionIR) {
    let mut changed = true;

    while changed {
        changed = false;

        changed |= constant_folding(func);
        changed |= dead_code_elimination(func);
    }
}

// =============================================================
// CONSTANT FOLDING
// =============================================================
fn constant_folding(func: &mut FunctionIR) -> bool {
    let mut changed = false;

    // 1) Collect const values
    use std::collections::HashMap;
    let mut const_map: HashMap<u32, Value> = HashMap::new();

    for block in &func.blocks {
        for instr in &block.instrs {
            if let Instr::Const { dst, value } = instr {
                const_map.insert(*dst, value.clone());
            }
        }
    }

    // 2) Fold binary ops
    for block in &mut func.blocks {
        for instr in &mut block.instrs {
            let original = std::mem::replace(instr, Instr::Return(0));

            *instr = match original {
                Instr::Binary { dst, op, lhs, rhs } => {
                    let lv = const_map.get(&lhs);
                    let rv = const_map.get(&rhs);

                    if let (Some(Value::Number(a)), Some(Value::Number(b))) = (lv, rv) {
                        let folded = match op.as_str() {
                            "+" => Some(a + b),
                            "-" => Some(a - b),
                            "*" => Some(a * b),
                            "/" => Some(a / b),
                            _ => None,
                        };

                        if let Some(result) = folded {
                            const_map.insert(dst, Value::Number(result));
                            changed = true;

                            Instr::Const {
                                dst,
                                value: Value::Number(result),
                            }
                        } else {
                            // preserve original if unknown op
                            Instr::Binary { dst, op, lhs, rhs }
                        }
                    } else {
                        // cannot fold (not both consts)
                        Instr::Binary { dst, op, lhs, rhs }
                    }
                }

                other => other,
            };
        }
    }

    changed
}

// =============================================================
// DEAD CODE ELIMINATION (DCE)
// =============================================================
fn dead_code_elimination(func: &mut FunctionIR) -> bool {
    use std::collections::HashSet;

    let mut live: HashSet<u32> = HashSet::new();

    // Mark return values as live
    for block in &func.blocks {
        for instr in &block.instrs {
            if let Instr::Return(id) = instr {
                live.insert(*id);
            }
        }
    }

    // propagate liveness backwards
    let mut changed = true;
    while changed {
        changed = false;

        for block in &func.blocks {
            for instr in &block.instrs {
                for used in used_values(instr) {
                    if live.insert(used) {
                        changed = true;
                    }
                }
            }
        }
    }

    // remove dead instructions
    let mut removed = false;

    for block in &mut func.blocks {
        let before = block.instrs.len();

        block.instrs.retain(|instr| {
            match instr {
                Instr::Binary { dst, .. }
                | Instr::Unary { dst, .. }
                | Instr::Const { dst, .. } => live.contains(dst),

                _ => true, // keep load/store/return
            }
        });

        if block.instrs.len() != before {
            removed = true;
        }
    }

    removed
}

// collect used ValueIds from instruction
fn used_values(instr: &Instr) -> Vec<u32> {
    match instr {
        Instr::Binary { lhs, rhs, .. } => vec![*lhs, *rhs],
        Instr::Unary { src, .. } => vec![*src],
        Instr::StoreLocal { src, .. } => vec![*src],
        Instr::Return(id) => vec![*id],
        Instr::Tuple { items, .. } => items.clone(),
        Instr::Array { items, .. } => items.clone(),
        Instr::StructInit { fields, .. } => fields.iter().map(|(_, v)| *v).collect(),
        Instr::StructUpdate { base, updates, .. } => {
            let mut result = vec![*base];
            result.extend(updates.iter().map(|(_, v)| *v));
            result
        }
        Instr::VariantInit { fields, .. } => fields.iter().map(|(_, v)| *v).collect(),
        Instr::Index { arr, index, .. } => vec![*arr, *index],
        Instr::FieldAccess { base, .. } => vec![*base],
        Instr::NamespaceAccess { base, .. } => vec![*base],
        Instr::Call { func, args, .. } => {
            let mut result = vec![*func];
            result.extend(args.clone());
            result
        }
        Instr::Match { scrutinee, arms, .. } => {
            let mut used = vec![*scrutinee];
            for (_, guard, result_id) in arms {
                if let Some(g) = guard {
                    used.push(*g);
                }
                used.push(*result_id);
            }
            used
        }
        Instr::MapInit { entries, .. } => entries.iter().map(|(_, v)| *v).collect(),
        Instr::MapAccess { map, .. } => vec![*map],
        Instr::FieldStore { base, src, .. } => vec![*base, *src],
        Instr::IndexStore { arr, index, src, .. } => vec![*arr, *index, *src],
        Instr::TupleExtract { tuple, .. } => vec![*tuple],
        Instr::LoadLocal { .. } => vec![],
        Instr::Const { .. } => vec![],
    }
}

