// async_rt/future.rs

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

pub struct FutureHandle {
    pub id: usize,
    pub inner: Pin<Box<dyn Future<Output = ()>>>,
    pub waker: Option<Waker>,
}

impl FutureHandle {
    pub fn new(id: usize, f: impl Future<Output = ()> + 'static) -> Self {
        Self {
            id,
            inner: Box::pin(f),
            waker: None,
        }
    }

    pub fn poll(&mut self, cx: &mut Context<'_>) -> Poll<()> {
        self.inner.as_mut().poll(cx)
    }
}
