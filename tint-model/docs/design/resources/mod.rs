// tint-runtime/resources/mod.rs
//
// IDEA -- not wired into TintVM. An early sketch toward a generic
// resource table (handle-based ownership of arbitrary Rust values);
// nothing in the evaluator or VM inserts into or reads from it. Kept as
// reference for a possible future resource-handle feature, not as
// working code.
#![allow(dead_code)]

// Public submodules
mod table;
mod types;

// Public re-exports
pub use table::ResourceTable;
pub use types::Resource;
