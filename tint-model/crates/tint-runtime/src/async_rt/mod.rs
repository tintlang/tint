// runtime/async_rt/mod.rs
//
// IDEA -- not wired into TintVM. An early sketch toward an async
// runtime/future scheduler for Tint; `Scheduler::poll` is an empty stub
// and nothing registers a future with it or calls `poll`. Kept as
// reference for a possible future async-language-feature, not as
// working code.
#![allow(dead_code)]

pub mod future;
pub mod scheduler;

pub use scheduler::Scheduler;

impl Scheduler {
    pub fn poll(&mut self) {
        // TODO: poll all futures
    }
}
