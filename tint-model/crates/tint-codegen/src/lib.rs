//! Native backend for the typed IR (Cranelift JIT).
//!
//! Supported: unit, bool, numbers, strings, lists, tuples, structs and enums,
//! direct calls, `print`. Calls into the runtime library that are not
//! implemented natively run on the reference interpreter (see `rt`). What the
//! backend cannot compile (closures, maps, globals, UI) is reported as
//! `Unsupported` at compile time, never miscompiled. Integer arithmetic is
//! checked like in the interpreter; a failed check prints `trap: <message>`
//! and exits the process.

mod lower;
pub mod rt;

include!("lib/core.rs");
include!("lib/compile.rs");
include!("lib/jit.rs");
