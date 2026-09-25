// tint-ir/regalloc.rs
//
// Skeleton for future register allocator.

use crate::ssa::FunctionIR;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reg(u8);

#[derive(Debug)]
pub struct AllocationResult {
    pub value_to_reg: std::collections::HashMap<u32, Reg>,
}

pub struct RegAlloc;

impl RegAlloc {
    pub fn allocate(func: &FunctionIR) -> AllocationResult {
        // Placeholder: naive allocation
        let mut map = std::collections::HashMap::new();

        let mut next = 0;
        for (id, _) in func.values.iter() {
            map.insert(*id, Reg(next));
            next += 1;
        }

        AllocationResult {
            value_to_reg: map,
        }
    }
}
