use super::store::StateId;
use std::fmt;

pub type WatchCallback = Box<dyn Fn()>;

pub struct WatchEntry {
    pub target: StateId,
    pub callback: WatchCallback,
}

impl fmt::Debug for WatchEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WatchEntry")
            .field("target", &self.target)
            .field("callback", &"<fn>") // скрываем callback
            .finish()
    }
}
