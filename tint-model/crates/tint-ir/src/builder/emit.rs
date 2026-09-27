use super::IrBuilder;
use crate::ir::*;
use tint_ast::Pattern;

impl IrBuilder {
    pub fn emit_const(&mut self, block: &mut Block, v: Value) -> ValueId {
        let dst = self.fresh_value();
        block.instrs.push(Instr::Const { dst, value: v });
        dst
    }

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

    pub fn emit_unary(&mut self, block: &mut Block, op: String, src: ValueId) -> ValueId {
        let dst = self.fresh_value();
        block.instrs.push(Instr::Unary { dst, op, src });
        dst
    }

    pub fn emit_load(&mut self, block: &mut Block, name: String) -> ValueId {
        let dst = self.fresh_value();
        block.instrs.push(Instr::LoadLocal { dst, name });
        dst
    }

    pub fn emit_store(&mut self, block: &mut Block, name: String, src: ValueId) {
        block.instrs.push(Instr::StoreLocal { name, src });
    }

    pub fn emit_call(&mut self, block: &mut Block, func: ValueId, args: Vec<ValueId>) -> ValueId {
        let dst = self.fresh_value();
        block.instrs.push(Instr::Call {
            dst,
            func,
            args,
            method: None,
        });
        dst
    }

    pub fn emit_method_call(
        &mut self,
        block: &mut Block,
        receiver: ValueId,
        method: String,
        args: Vec<ValueId>,
    ) -> ValueId {
        let dst = self.fresh_value();
        block.instrs.push(Instr::Call {
            dst,
            func: receiver,
            args,
            method: Some((receiver, method)),
        });
        dst
    }

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

    pub fn emit_namespace_access(
        &mut self,
        block: &mut Block,
        base: ValueId,
        item: String,
    ) -> ValueId {
        let dst = self.fresh_value();
        block
            .instrs
            .push(Instr::NamespaceAccess { dst, base, item });
        dst
    }

    pub fn emit_index(&mut self, block: &mut Block, arr: ValueId, index: ValueId) -> ValueId {
        let dst = self.fresh_value();
        block.instrs.push(Instr::Index { dst, arr, index });
        dst
    }

    // STRUCT INIT
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

    pub fn emit_variant_init(
        &mut self,
        block: &mut Block,
        enum_name: String,
        variant: String,
        fields: Vec<(String, ValueId)>,
    ) -> ValueId {
        let dst = self.fresh_value();
        block.instrs.push(Instr::VariantInit {
            dst,
            enum_name,
            variant,
            fields,
        });
        dst
    }

    pub fn emit_map_init(&mut self, block: &mut Block, entries: Vec<(String, ValueId)>) -> ValueId {
        let dst = self.fresh_value();
        block.instrs.push(Instr::MapInit { dst, entries });
        dst
    }

    pub fn emit_map_access(&mut self, block: &mut Block, map: ValueId, key: String) -> ValueId {
        let dst = self.fresh_value();
        block.instrs.push(Instr::MapAccess { dst, map, key });
        dst
    }

    pub fn emit_field_store(
        &mut self,
        block: &mut Block,
        base: ValueId,
        field: String,
        src: ValueId,
    ) {
        block.instrs.push(Instr::FieldStore { base, field, src });
    }

    pub fn emit_index_store(
        &mut self,
        block: &mut Block,
        arr: ValueId,
        index: ValueId,
        src: ValueId,
    ) {
        block.instrs.push(Instr::IndexStore { arr, index, src });
    }

    pub fn emit_tuple(&mut self, block: &mut Block, items: Vec<ValueId>) -> ValueId {
        let dst = self.fresh_value();
        block.instrs.push(Instr::Tuple { dst, items });
        dst
    }

    pub fn emit_tuple_extract(
        &mut self,
        block: &mut Block,
        tuple: ValueId,
        index: usize,
    ) -> ValueId {
        let dst = self.fresh_value();
        block.instrs.push(Instr::TupleExtract { dst, tuple, index });
        dst
    }

    pub fn emit_struct_update(
        &mut self,
        block: &mut Block,
        base: ValueId,
        updates: Vec<(String, ValueId)>,
    ) -> ValueId {
        let dst = self.fresh_value();
        block
            .instrs
            .push(Instr::StructUpdate { dst, base, updates });
        dst
    }

    pub fn emit_array(&mut self, block: &mut Block, items: Vec<ValueId>) -> ValueId {
        let dst = self.fresh_value();
        block.instrs.push(Instr::Array { dst, items });
        dst
    }

    pub fn emit_match(
        &mut self,
        block: &mut Block,
        scrutinee: ValueId,
        arms: Vec<(Pattern, Option<ValueId>, ValueId)>,
    ) -> ValueId {
        let dst = self.fresh_value();
        block.instrs.push(Instr::Match {
            dst,
            scrutinee,
            arms,
        });
        dst
    }

    pub fn emit_return(&mut self, block: &mut Block, src: ValueId) {
        block.instrs.push(Instr::Return(src));
    }
}
