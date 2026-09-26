// ============================================
// tint-ir/src/ir.rs — Minimal SSA IR
// ============================================
use tint_ast::Pattern;

pub type ValueId = u32;
pub type BlockId = u32;

#[derive(Debug, Clone)]
pub enum Value {
    Number(f64),
    String(String),
    Bool(bool),
    Unit,

    Tuple(Vec<Value>),
    List(Vec<Value>),

    StructInstance {
        name: String,
        fields: Vec<(String, Value)>,
    },

    EnumInstance {
        enum_name: String,
        variant: String,
        args: Vec<Value>,
    },

    Map(std::collections::HashMap<String, Value>),
}

#[derive(Debug, Clone)]
pub struct Block {
    pub id: BlockId,
    pub instrs: Vec<Instr>,
}

#[derive(Debug, Clone)]
pub enum Instr {
    Const {
        dst: ValueId,
        value: Value,
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

#[derive(Debug, Clone)]
pub struct FunctionIR {
    pub name: String,
    pub blocks: Vec<Block>,
    pub locals: std::collections::HashMap<String, ValueId>,
}

#[derive(Debug, Clone)]
pub struct ProgramIR {
    pub functions: Vec<FunctionIR>,
}

impl ProgramIR {
    pub fn new() -> Self {
        Self { functions: vec![] }
    }
}

impl Value {
    pub fn unwrap_number(&self) -> f64 {
        match self {
            Value::Number(n) => *n,
            other => panic!("Expected number, got {:?}", other),
        }
    }

    pub fn unwrap_bool(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            other => panic!("Expected bool, got {:?}", other),
        }
    }

    pub fn unwrap_string(&self) -> &str {
        match self {
            Value::String(s) => s,
            other => panic!("Expected string, got {:?}", other),
        }
    }
}
