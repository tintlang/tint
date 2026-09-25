// IR module: data structures for SSA intermediate representation
pub mod ir;
pub mod opcode;
pub mod function;

pub use ir::{ProgramIR, BasicBlock, Instruction, Value, ValueId};
pub use opcode::Opcode;
pub use function::FunctionIR;
