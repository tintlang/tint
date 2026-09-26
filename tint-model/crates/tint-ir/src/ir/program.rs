use super::Instr;
use tint_ast::Pattern;

pub type ValueId = u32;
pub type BlockId = u32;

#[derive(Debug, Clone)]
pub struct Block {
    pub id: BlockId,
    pub instrs: Vec<Instr>,
}

#[derive(Debug, Clone)]
pub struct FunctionIR {
    pub name: String,
    pub blocks: Vec<Block>,
    pub locals: std::collections::HashMap<String, ValueId>,
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
