// runtime/state/mod.rs
//
// IDEA -- not wired into TintVM. A typed (`create_state::<T>()`) reactive
// state store, explored early on but never connected to anything real:
// `TintVM`'s actual `state` (the `state name = expr` declarations a `ui
// fn` can have) lives directly in the VM's own scope stack instead (see
// `ui_session.rs`), which fits a dynamically-typed language much more
// naturally than this generic-over-T design does. Kept as reference for
// a possible future typed-state feature, not as working code.
#![allow(dead_code)]

pub mod cell;
pub mod computed;
pub mod signal;
pub mod store;
pub mod watch;

pub use cell::StateCell;
pub use computed::ComputedCell;
pub use signal::SignalCell;
pub use store::{StateId, StateStore};
pub use watch::{WatchCallback, WatchEntry};
