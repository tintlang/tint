//! Lowering of one typed-IR function to Cranelift.
//!
//! Every IR register is a Cranelift variable. A register of a heap type holds
//! null or exactly one owned reference; moving a value into a register
//! retains it (or hands the reference over when the source register is dead
//! afterwards), overwriting releases the old one, and `Return` hands the
//! result's reference to the caller and releases the rest.

include!("prelude.rs");
include!("methods1.rs");
include!("methods2.rs");
include!("methods3.rs");
include!("methods4.rs");
include!("free.rs");
