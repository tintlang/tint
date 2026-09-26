pub mod scope;
pub mod value;
pub mod vm;

pub mod ui;

pub mod errors;
pub mod repl;
pub mod ui_session;

pub use errors::RuntimeError;

// IDEA / reserved for future work -- not wired into TintVM or any
// functional path (checked: nothing outside these modules' own files
// references them). Kept as design placeholders for future
// async/resource/state/borrow-checking work, not as working subsystems.
// Each module is `#[allow(dead_code)]`'d so its own internal warnings
// don't clutter a `cargo build` of the real, working parts of this crate.
pub mod async_rt;
pub mod borrow;
pub mod resources;
pub mod state;
