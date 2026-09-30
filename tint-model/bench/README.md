# Tint benchmarks

Compares Tint with Python, Node, Go and Rust on the same small programs.

```sh
cargo build --release -p tint-cli      # benchmark the release binary, not debug
python3 bench/run.py                   # all tasks, every language found on PATH
python3 bench/run.py --runs 10 --tasks fib,loop --langs tint,python,rust
```

Set `TINT=/path/to/tint` to benchmark another binary. Results are printed and
saved to `bench/results/latest.md` and `latest.json`.

## Tasks

| task | measures | size |
|---|---|---|
| `fib` | function calls | fib(30) |
| `loop` | integer loop + `%` | 3M iterations |
| `collatz` | integer loops + branches + calls | n < 20000 |
| `mandel` | float math | 200x200, 50 iterations |
| `sieve` | list indexing and writes | n = 3000 |
| `hello` | process startup (subtracted in the "net" table) | - |

All languages print the same answer; the runner fails the task if they differ.
Sizes are small on purpose so that Tint finishes in seconds. Go and Rust finish
in milliseconds, where process startup dominates, so read the "net" and "slower
than fastest" tables, and raise the sizes in `tasks/*/main.*` (in every language)
when you want steadier numbers for the compiled ones.

## Caveats

- Wall-clock time of the whole process, median of N runs after one warm-up.
- One machine, one run of the harness: use it for orders of magnitude, not for
  percent differences.
- Rust uses `black_box` on the input so LLVM cannot fold the loop to a constant.
  Go/Node/Python get no such treatment (Go's compiler does not fold these).
- Peak memory is sampled from `/proc` on Linux (approximate). On macOS it falls
  back to `ru_maxrss`, which includes this script's own memory (~10 MiB floor).
- Node's number is JIT-warm-up included: short tasks understate its peak speed.

## Adding a task

Create `tasks/<name>/main.{tn,py,js,go,rs}` printing the same output, and the
runner picks it up.
