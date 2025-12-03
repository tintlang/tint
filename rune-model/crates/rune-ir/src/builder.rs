// ============================================
// rune-ir/src/builder.rs — SSA Builder
// ============================================

use crate::ir::*;

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

    // ---------------------------------------
    // CONSTANTS
    // ---------------------------------------
    pub fn emit_const(&mut self, block: &mut Block, v: Value) -> ValueId {
        let dst = self.fresh_value();
        block.instrs.push(Instr::Const { dst, value: v });
        dst
    }

    // ---------------------------------------
    // BINARY
    // ---------------------------------------
    pub fn emit_binary(
        &mut self,
        block: &mut Block,
        op: String,
        lhs: ValueId,
        rhs: ValueId,
    ) -> ValueId {
        let dst = self.fresh_value();
        block.instrs.push(Instr::Binary { dst, op, lhs, rhs });
        dst
    }

    // ---------------------------------------
    // UNARY
    // ---------------------------------------
    pub fn emit_unary(
        &mut self,
        block: &mut Block,
        op: String,
        src: ValueId,
    ) -> ValueId {
        let dst = self.fresh_value();
        block.instrs.push(Instr::Unary { dst, op, src });
        dst
    }

    // ---------------------------------------
    // LOAD / STORE
    // ---------------------------------------
    pub fn emit_load(&mut self, block: &mut Block, name: String) -> ValueId {
        let dst = self.fresh_value();
        block.instrs.push(Instr::LoadLocal { dst, name });
        dst
    }

    pub fn emit_store(
        &mut self,
        block: &mut Block,
        name: String,
        src: ValueId,
    ) {
        block.instrs.push(Instr::StoreLocal { name, src });
    }

    // ---------------------------------------
    // CALL: foo(a,b)
    // NOTE: compiler gives (fnValueId, arg_ids)
    // ---------------------------------------
    pub fn emit_call(
        &mut self,
        block: &mut Block,
        func: ValueId,
        args: Vec<ValueId>,
    ) -> ValueId {
        let dst = self.fresh_value();
        block.instrs.push(Instr::Call { dst, func, args });
        dst
    }

    // ---------------------------------------
    // FIELD: obj.field
    // ---------------------------------------
    pub fn emit_field_access(
        &mut self,
        block: &mut Block,
        base: ValueId,
        field: String,
    ) -> ValueId {
        let dst = self.fresh_value();
        block.instrs.push(Instr::FieldAccess { dst, base, field });
        dst
    }

    // ---------------------------------------
    // NAMESPACE: A::B
    // ---------------------------------------
    pub fn emit_namespace_access(
        &mut self,
        block: &mut Block,
        base: ValueId,
        item: String,
    ) -> ValueId {
        let dst = self.fresh_value();
        block.instrs.push(Instr::NamespaceAccess { dst, base, item });
        dst
    }

    // ---------------------------------------
    // INDEX: arr[i]
    // ---------------------------------------
    pub fn emit_index(
        &mut self,
        block: &mut Block,
        arr: ValueId,
        index: ValueId,
    ) -> ValueId {
        let dst = self.fresh_value();
        block.instrs.push(Instr::Index { dst, arr, index });
        dst
    }

    // ---------------------------------------
    // STRUCT INIT
    // ---------------------------------------
    pub fn emit_struct_init(
        &mut self,
        block: &mut Block,
        name: String,
        fields: Vec<(String, ValueId)>,
    ) -> ValueId {
        let dst = self.fresh_value();
        block.instrs.push(Instr::StructInit { dst, name, fields });
        dst
    }

    // ---------------------------------------
    // RETURN
    // ---------------------------------------
    pub fn emit_return(&mut self, block: &mut Block, src: ValueId) {
        block.instrs.push(Instr::Return(src));
    }
}
