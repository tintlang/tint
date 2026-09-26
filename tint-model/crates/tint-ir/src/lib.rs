pub mod builder;
pub mod compiler;
pub mod function;
pub mod ir;
pub mod opcode;
pub mod passes;
pub mod ssa;
pub mod vm;

pub use compiler::SsaCompiler;
pub use function::{IRFunction, IRValue};
pub use ir::ProgramIR;
pub use opcode::Opcode;
pub use passes::optimize;
pub use vm::IrVM;
