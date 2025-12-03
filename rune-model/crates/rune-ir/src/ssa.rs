// rune-ir/ssa.rs
//
// Core SSA IR types:
// - FunctionIR
// - Instr
// - Value, ValueKind
// - helpers for analysing instructions

use std::collections::HashMap;

// -------------------------------------------
// SSA Values
// -------------------------------------------
#[derive(Debug, Clone)]
pub struct Value {
    pub id: u32,
    pub kind: ValueKind,
}

#[derive(Debug, Clone)]
pub enum ValueKind {
    ConstInt(i64),
    Temp,         // temporary SSA value
    Undefined,
}

// -------------------------------------------
// SSA Instructions
// -------------------------------------------
#[derive(Debug, Clone)]
pub enum Instr {
    Binary { op: String, lhs: u32, rhs: u32 },
    Copy { dst: u32, src: u32 },
    // TODO: Call, Load, Store, Jump, Phi, …
}

impl Instr {
    pub fn replace_value(&mut self, old: u32, new: u32) {
        match self {
            Instr::Binary { lhs, rhs, .. } => {
                if *lhs == old { *lhs = new }
                if *rhs == old { *rhs = new }
            }
            Instr::Copy { dst, src } => {
                if *dst == old { *dst = new }
                if *src == old { *src = new }
            }
        }
    }

    pub fn used_values(&self) -> Vec<u32> {
        match self {
            Instr::Binary { lhs, rhs, .. } => vec![*lhs, *rhs],
            Instr::Copy { src, .. } => vec![*src],
        }
    }
}

// -------------------------------------------
// SSA Function IR
// -------------------------------------------
#[derive(Debug, Clone)]
pub struct FunctionIR {
    pub instrs: HashMap<u32, Instr>,
    pub values: HashMap<u32, Value>,
    pub next_id: u32,
    pub ret_value: Option<u32>,
}

impl FunctionIR {
    pub fn new() -> Self {
        Self {
            instrs: HashMap::new(),
            values: HashMap::new(),
            next_id: 0,
            ret_value: None,
        }
    }

    pub fn new_value(&mut self, kind: ValueKind) -> u32 {
        let id = self.next_id;
        self.next_id += 1;

        self.values.insert(id, Value { id, kind });

        id
    }

    pub fn add_instr(&mut self, instr: Instr) -> u32 {
        let id = self.next_id;
        self.next_id += 1;

        self.instrs.insert(id, instr);
        id
    }
}
