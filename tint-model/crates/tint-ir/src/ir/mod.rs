mod instruction;
mod program;
mod value;

pub use instruction::Instr;
pub use program::{Block, BlockId, FunctionIR, ProgramIR, ValueId};
pub use value::Value;
