# Conformance suite

Engine-agnostic `.tn` cases. Any execution engine (tree-walker, a future
bytecode VM, wasm/native backends) must render every case identically.

Layout: `core/` (arithmetic, numeric types, control flow, functions), `data/`
(structs/enums, lists, maps, strings), `option_result/`, and `errors/` (known
gaps `gaps_*.tn` and cases only the dynamic tree-walker accepts). Subfolders
are found recursively; case names are printed as `folder/file.tn::fn`.

A case is a directive comment directly above a zero-argument `fn`:

```
// expect: 42            result renders to exactly `42`
// expect-error: text    run fails and the message contains `text`
// xfail: reason         known gap on every engine: must currently FAIL (a passing xfail fails the suite)
// xfail-tree-walker: r  known gap of the tree-walker only; other engines must pass
// ir-expect: 42f32      what the typed IR renders instead (it is statically typed)
// ir-expect-error: t    the typed IR rejects the program (type/lowering error) or traps with `t`
fn answer() { 40 + 2 }
```

Rendering: numbers as `f64` (`3`, `2.5`), exact types with suffix (`3i32`, `255u8`),
strings quoted, lists `[1, 2]`, tuples `(1, 2)`, structs `Name { a: 1 }`,
enum variants `Enum::Variant(args)`, maps `{k: v}` sorted by key.

Each case runs on a fresh VM with the whole file loaded. A parse error takes
down every case in its file, so keep parse-level gaps in their own `errors/gaps_*.tn`.

Engines: `tree-walker` and `typed-ir` (strict check, lower to `tint-ir::typed`,
run on the reference interpreter). Statically typed results keep their suffix,
so use `ir-expect` where it differs. `dynamic.tn` holds cases only the dynamic
tree-walker accepts (`ir-expect-error`).

Run: `cargo test -p tint-runtime --test conformance -- --nocapture`
(`--nocapture` also prints what each xfail currently returns).

To add an engine, implement `Engine` in `tint-runtime/tests/conformance.rs`
and list it in `engines()`.
