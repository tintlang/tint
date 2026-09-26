use crate::ir::{FunctionIR, Instr, Value};

pub(super) fn constant_folding(func: &mut FunctionIR) -> bool {
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
