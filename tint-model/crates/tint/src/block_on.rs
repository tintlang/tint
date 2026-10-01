//! Native side of `async` exports: the future is run to completion on the
//! calling thread (there is no reactor, so futures that need one -- tokio
//! timers, sockets -- do not work here), then handed to Tint as a `Result`.

use std::future::Future;
use std::pin::pin;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
use std::thread::{self, Thread};

use crate::__private::NativeFuture;
use crate::{EvalResult, Value};

struct ThreadWaker(Thread);

impl Wake for ThreadWaker {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}

fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    let waker = Waker::from(Arc::new(ThreadWaker(thread::current())));
    let mut context = Context::from_waker(&waker);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(value) => return value,
            Poll::Pending => thread::park(),
        }
    }
}

fn result_value(outcome: Result<Value, String>) -> Value {
    let (variant, payload) = match outcome {
        Ok(value) => ("Ok", value),
        Err(message) => ("Err", Value::String(message)),
    };
    Value::EnumInstance { enum_name: "Result".into(), variant: variant.into(), args: vec![payload] }
}

/// Runs an `async` export. With a trailing Tint callback (`f(a, b, |r| ..)`)
/// the callback receives `Result::Ok/Err`; without one the call returns it.
pub fn run_async_native(
    _name: &str,
    declared: usize,
    args: &[Value],
    make: fn(Vec<Value>) -> NativeFuture,
) -> EvalResult<Value> {
    let (callback, values) = match args.split_last() {
        Some((Value::Callback(callback), rest)) if args.len() == declared + 1 => (Some(callback.clone()), rest.to_vec()),
        _ => (None, args.to_vec()),
    };
    let result = result_value(block_on(make(values)));
    match callback {
        Some(callback) => {
            (callback.0)(&[result])?;
            Ok(Value::Unit)
        }
        None => Ok(result),
    }
}
