pub mod builder;
pub mod compiler;
pub mod function;
pub mod ir;
pub mod ir_vm;
pub mod opcode;
pub mod ssa;
pub mod ssa_pass;

pub use compiler::SsaCompiler;
pub use function::{IRFunction, IRValue};
pub use ir::ProgramIR;
pub use ir_vm::IrVM;
pub use opcode::Opcode;
pub use ssa_pass::optimize;
