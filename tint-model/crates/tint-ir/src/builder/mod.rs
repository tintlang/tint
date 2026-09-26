use crate::ir::*;
mod emit;

/// SSA Builder: allocates ValueId, builds Ops with destinations (dst)
pub struct IrBuilder {
    next_value: ValueId,
    next_block: BlockId,
}

impl IrBuilder {
    pub fn new() -> Self {
        Self {
            next_value: 0,
            next_block: 0,
        }
    }

    fn fresh_value(&mut self) -> ValueId {
        let id = self.next_value;
        self.next_value += 1;
        id
    }

    pub fn new_block(&mut self) -> Block {
        let id = self.next_block;
        self.next_block += 1;
        Block { id, instrs: vec![] }
    }
}
