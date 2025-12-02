pub mod future;
pub mod scheduler;

use future::FutureHandle;
pub use scheduler::Scheduler;

impl Scheduler {
    pub fn poll(&mut self) {
        // TODO: poll all futures
    }
}
