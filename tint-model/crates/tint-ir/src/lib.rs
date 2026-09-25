pub mod opcode;
pub mod function;
pub mod builder;
pub mod compiler;
pub mod ir;
pub mod ssa;
pub mod ssa_pass;
pub mod ir_vm;

pub use opcode::Opcode;
pub use function::{IRFunction, IRValue};
pub use compiler::SsaCompiler;
pub use ssa_pass::optimize;
pub use ir_vm::IrVM;
pub use ir::ProgramIR;
