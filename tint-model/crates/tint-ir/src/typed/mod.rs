//! Typed IR: what Tint programs are lowered to after type checking.
//!
//! - `ty`: interned types and monomorphized ADTs
//! - `ir`: registers, instructions, blocks, functions, modules
//! - `layout`: how values are laid out in 64-bit slots
//! - `verify`: structural and type checks over a module
//! - `lower`: typed AST + `SemanticModel` -> `Module`
//! - `interp`: the reference interpreter of the IR

pub mod display;
pub mod interp;
pub mod ir;
pub mod layout;
pub mod lower;
pub mod ty;
pub mod ui;
pub mod verify;

pub use interp::{Interp, Trap, Val};
pub use ir::*;
pub use lower::{lower_program, LowerError, Lowered};
pub use ty::*;
pub use ui::*;
