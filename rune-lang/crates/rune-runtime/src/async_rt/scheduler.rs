// async_rt/scheduler.rs

use super::future::FutureHandle;
use std::task::{Context, Poll, Waker, RawWaker, RawWakerVTable};
use std::pin::Pin;
use std::collections::VecDeque;

pub struct Scheduler {
    queue: VecDeque<FutureHandle>,
}

impl Scheduler {
    pub fn new() -> Self {
        Self { queue: VecDeque::new() }
    }

    pub fn spawn(&mut self, f: impl std::future::Future<Output = ()> + 'static) {
        let id = self.queue.len();
        self.queue.push_back(FutureHandle::new(id, f));
    }
    
    pub fn poll_all(&mut self) {
        let mut n = self.queue.len();

        while n > 0 {
            let mut handle = self.queue.pop_front().unwrap();

            let waker = dummy_waker();
            let mut cx = Context::from_waker(&waker);

            match handle.poll(&mut cx) {
                Poll::Pending => self.queue.push_back(handle),
                Poll::Ready(_) => {}
            }

            n -= 1;
        }
    }
}

// dummy no-op waker
fn dummy_waker() -> Waker {
    unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) }
}

static VTABLE: std::task::RawWakerVTable = RawWakerVTable::new(
    |_| RawWaker::new(std::ptr::null(), &VTABLE),
    |_| {},
    |_| {},
    |_| {},
);
