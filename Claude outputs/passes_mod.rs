// Passes module: IR transformation and optimization passes
pub mod ssa;
pub mod ssa_pass;

pub use ssa::{SSA};
pub use ssa_pass::{run_ssa_pass};
