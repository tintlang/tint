# Compiled wasm vs Node

Tint programs compiled by `tint-wasmgen` against the `tint-wasmrt` runtime, compared with the
same task written in JavaScript. Both run `main()` in V8, each measurement in a fresh process
(the engine shares optimized code between instances of identical modules, so repeats inside one
process are not representative).

```bash
node bench/wasm/all.js [runs] [task ...]   # from tint-model/; default 9 runs, every task
```

`DUMP=1 target/release/examples/emit bench/tasks/<task>/main.tn out.wasm` prints the typed IR.
Build output goes to `bench/wasm/out/` (ignored).
