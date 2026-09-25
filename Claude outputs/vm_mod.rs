// VM module: IR virtual machine execution
pub mod ir_vm;
pub mod regalloc;

pub use ir_vm::{IrVM};
pub use regalloc::RegAlloc;
