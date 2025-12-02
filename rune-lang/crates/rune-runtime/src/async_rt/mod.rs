pub mod future;
pub mod scheduler;

use future::FutureHandle;
use scheduler::Scheduler;

impl Scheduler {
    pub fn poll(&mut self) {
        // TODO: poll all futures
    }
}
