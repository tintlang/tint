# Callbacks and async

Tint has no futures. Anything that finishes later (a timer, a `fetch`, a Rust `async fn`) is a function
that takes a **callback as its last argument**, and `await` is sugar for exactly that.

## Callbacks

A lambda or `fn` can be passed to a host function (JS or Rust). While the host function runs, the
callback runs at once; after it has returned (a timer, a settled Promise) the callback runs against the
live app, and the UI re-renders. Callbacks can assign state:

```tn
ui fn App() {
    state label = "waiting"
    Button { click||start "go" }
}

fn start() { later(|text| { label = text }) }
```

Rust receives it as `tint::Callback` (`cb.call::<R>((arg,))`); in JS it is a normal function.

## Async functions

A **JS function that returns a Promise**, or a **Rust `async fn`** (returning `Result<T, E: Display>`),
takes a trailing callback. The callback receives `Result::Ok(value)` or `Result::Err(message)`:

```tn
fn load() {
    fetch_json("/api/items", |r| {
        match r {
            Ok { value } => items = value,
            Err { error } => status = "failed: {error}"
        }
    })
}
```

```js
export function fetch_json(url) { return fetch(url).then((r) => r.json()); }
```

```rust
#[tint::export]
pub async fn slow_square(n: f64) -> Result<f64, String> { Ok(n * n) }
```

Without a trailing callback a Promise-returning function is an error (Tint has no way to wait). A Rust
`async fn` called natively (`tint run`) is run to completion on the calling thread: futures that need a
reactor (tokio timers, sockets) do not work there; in the browser it is a Promise.

### Types of what the host returns

`tint build` compiles the program to WebAssembly, so the type of a host function's result must be
known when the program is compiled. Write it where the value is used: `let x: number = f(..)` for a
call, `|r: Result<number, string>|` for a callback's parameter, `f(..) as number` (also `as string`, `as Stats`, `as Vec<number>`, `as Map<string, number>`) for a call inside an expression, `let r: number = await f(..)` in an
`async fn` (the callback then receives `Result<number, string>`). A callback that does not read its
parameter needs nothing. Without the type the build stops with an error that names the call.

## `await`

Inside an `async fn`, `await call(args)` rewrites the rest of the body into the callback of the call:

```tn
async fn run() {
    status = "loading"
    let squared = await slow_square(7)      // slow_square(7, |squared| { ...rest... })
    await wait(300)
    status = "done: {squared}"
}
```

Rules: `await` is only allowed on a call and only at the top level of the `async fn` body (not inside
`if`, loops or lambdas; use a callback there), and `let` takes a single name. The bound value is the
`Result`. An `async fn` itself returns nothing; to hand a result on, give it a callback parameter.

See `examples/async`.
