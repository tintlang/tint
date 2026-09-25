#[derive(Debug, Clone)]
pub enum Opcode {
    LoadConst(u32),       // push constant index
    LoadLocal(u32),       // load local var
    StoreLocal(u32),      // store local var

    Add,
    Sub,
    Mul,
    Div,

    Neg,                  // unary -
    Not,                  // logical !

    Jump(usize),          // goto
    JumpIfFalse(usize),   // conditional branch

    Return,
}
