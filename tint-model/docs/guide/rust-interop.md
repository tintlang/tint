# Rust interop

Two directions, one small crate: `tint` (`crates/tint`).

## Rust calls Tint

```rust
use tint::{Tint, IntoTint, FromTint};

#[derive(IntoTint, FromTint)]
struct Point { x: f64, y: f64 }

let mut tint = Tint::new(tint::tint_file!("geometry.tn"))?;   // parsed at compile time
let len: f64 = tint.call("length", (Point { x: 3.0, y: 4.0 },))?;
```

- `Tint::call::<R>(name, args)` converts arguments and the result; `call_value` takes raw `Value`s.
- `tint_file!("path")` embeds the file (path relative to the crate root) and fails `cargo build` on a syntax error.
- `Tint::ui(source, "App")` gives a stateful `UiSession` to render with your own backend.
- Conversions: numbers, `bool`, `String`, `()`, `Vec<T>`, `HashMap<String, T>`, `Option<T>`, tuples (2 to 4),
  `Result<T, E>`, `Value`, and structs with named fields via `#[derive(IntoTint, FromTint)]`.
  Everything is copied. Tint's `number` is an `f64`, so integers beyond 2^53 lose precision and
  `FromTint` for an integer type rejects fractions and out-of-range values.

## Tint calls Rust

```rust
// native.rs
#[tint::export]
fn slug(text: String) -> String { text.to_lowercase().replace(' ', "-") }

#[tint::export(name = "to_num")]
fn parse(text: String) -> Result<f64, String> { text.trim().parse::<f64>().map_err(|e| e.to_string()) }
```

```tn
app { rs::"./native.rs" }

fn main() {
    println(slug("Hello World"))     // hello-world
    println(to_num("41"))            // Result::Ok(41)
}
```

`tint run app.tn` builds a small cargo project in `.tint/native/` next to the entry file
(add `.tint/` to your ignores; it ships its own `.gitignore`), links your `.rs` files, `tint` and
the CLI, and runs the same command through it. The first build takes a while; later ones are
incremental.

- Parameters implement `FromTint`; the result implements `IntoTint`. A `Result<T, E: Display>` arrives
  in Tint as `Result::Ok(..)` / `Result::Err(message)`. A wrong argument count or type is a runtime error
  naming the function and argument.
- Exports shadow built-ins of the same name. Functions cannot be `async`, generic, or take `self`/`&str`
  (use `String`).
- The `.rs` files can use `std` and `tint` only (no extra crates yet) and are compiled with `rustc`,
  which does the borrow checking. Tint never parses Rust.
- From your own Rust program, no runner is needed: `Tint::new` picks up every `#[tint::export]` linked
  into the binary (`tint::registered_natives()`); use `tint::natives![area, shapes::perimeter]` to
  pass them explicitly (required on `wasm32`).
- Set `TINT_CRATES` to the folder with the `tint` and `tint-cli` crates if the runner cannot find them.

## Rust in the browser

`tint build` and `tint dev` compile the `rs::` files to wasm and embed the module in the page, so the
same `native.rs` works in the browser with no server:

```sh
cargo install wasm-pack      # once
tint build app.tn -o app.html
```

- The module is built in `.tint/wasm/` with `opt-level = "z"`, LTO and stripped symbols (no interpreter is
  linked into it: the `tint` crate is used with `default-features = false`) and embedded gzipped. The
  example in `examples/rust_interop` comes to about 88 KB, 46 KB gzipped.
- Calls are synchronous, except `async fn` exports, which return a Promise (see [Callbacks and async](async.md)). Values cross as plain data; structs, `Option` and `Result` keep their Tint shape.
- Only `std` and `tint` are available. `std::fs`, threads and sockets do not exist in the browser.
- `mount()` (the npm package) does not compile Rust; use `tint build`, or pass your own compiled functions
  with the `natives` option.
- `wasm-opt` is not run (some binaryen versions corrupt wasm-bindgen output); gzip does most of the work.
