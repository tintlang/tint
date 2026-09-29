# Conformance suite

Engine-agnostic `.tn` cases. Any execution engine (tree-walker, a future
bytecode VM, wasm/native backends) must render every case identically.

A case is a directive comment directly above a zero-argument `fn`:

```
// expect: 42            result renders to exactly `42`
// expect-error: text    run fails and the message contains `text`
// xfail: reason         known gap: must currently FAIL (a passing xfail fails the suite)
fn answer() { 40 + 2 }
```

Rendering: numbers as `f64` (`3`, `2.5`), exact types with suffix (`3i32`, `255u8`),
strings quoted, lists `[1, 2]`, tuples `(1, 2)`, structs `Name { a: 1 }`,
enum variants `Enum::Variant(args)`, maps `{k: v}` sorted by key.

Each case runs on a fresh VM with the whole file loaded. A parse error takes
down every case in its file, so keep parse-level gaps in their own `gaps_*.tn`.

Run: `cargo test -p tint-runtime --test conformance -- --nocapture`
(`--nocapture` also prints what each xfail currently returns).

To add an engine, implement `Engine` in `tint-runtime/tests/conformance.rs`
and list it in `engines()`.
