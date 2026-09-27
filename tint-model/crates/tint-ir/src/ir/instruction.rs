use super::ValueId;
use tint_ast::Pattern;

#[derive(Debug, Clone)]
pub enum Instr {
    Const {
        dst: ValueId,
        value: super::Value,
    },
    LoadLocal {
        dst: ValueId,
        name: String,
    },
    StoreLocal {
        name: String,
        src: ValueId,
    },
    Unary {
        dst: ValueId,
        op: String,
        src: ValueId,
    },
    Binary {
        dst: ValueId,
        op: String,
        lhs: ValueId,
        rhs: ValueId,
    },
    Call {
        dst: ValueId,
        func: ValueId,
        args: Vec<ValueId>,
        method: Option<(ValueId, String)>,
    },
    FieldAccess {
        dst: ValueId,
        base: ValueId,
        field: String,
    },
    NamespaceAccess {
        dst: ValueId,
        base: ValueId,
        item: String,
    },
    Index {
        dst: ValueId,
        arr: ValueId,
        index: ValueId,
    },
    StructInit {
        dst: ValueId,
        name: String,
        fields: Vec<(String, ValueId)>,
    },
    StructUpdate {
        dst: ValueId,
        base: ValueId,
        updates: Vec<(String, ValueId)>,
    },
    VariantInit {
        dst: ValueId,
        enum_name: String,
        variant: String,
        fields: Vec<(String, ValueId)>,
    },
    Array {
        dst: ValueId,
        items: Vec<ValueId>,
    },
    Match {
        dst: ValueId,
        scrutinee: ValueId,
        arms: Vec<(Pattern, Option<ValueId>, ValueId)>,
    },
    Tuple {
        dst: ValueId,
        items: Vec<ValueId>,
    },
    TupleExtract {
        dst: ValueId,
        tuple: ValueId,
        index: usize,
    },
    MapInit {
        dst: ValueId,
        entries: Vec<(String, ValueId)>,
    },
    MapAccess {
        dst: ValueId,
        map: ValueId,
        key: String,
    },
    FieldStore {
        base: ValueId,
        field: String,
        src: ValueId,
    },
    IndexStore {
        arr: ValueId,
        index: ValueId,
        src: ValueId,
    },
    Return(ValueId),
}
