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
        // Named, not positional (unlike this used to be) -- so that a
        // struct-style variant (`enum E { A { x } }`, constructed as
        // `E::A { x { 50 } }`) can be pattern-matched by field NAME
        // (`A { x } => ...`), the same way `StructInstance`'s `fields`
        // already works. See `ir_vm.rs`'s `Pattern::Struct` match arm,
        // which now accepts this variant too. `Instr::VariantInit`
        // already carried these names at compile time (`fields: Vec<(String,
        // ValueId)>`) -- only this runtime value was throwing them away.
        // A purely positional variant pattern (`A(x) => ...`,
        // `Pattern::Variant`) still works fine against this: it just
        // zips against `fields` in declaration order and ignores the
        // names.
        fields: Vec<(String, Value)>,
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
    // The function's parameter patterns, in declaration order. Needed so
    // `Instr::Call` (see ir_vm.rs) can bind a callee's arguments by
    // itself when one IR-compiled function calls another -- previously
    // only the top-level entry point had its params supplied externally
    // (by `TintVM::call_fn`, which reads them off the original `FnDecl`),
    // so a nested call had no way to bind them at all.
    pub params: Vec<Pattern>,
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
