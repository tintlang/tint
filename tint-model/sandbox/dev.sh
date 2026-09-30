#!/bin/sh
# Dev server that also restarts when the Rust side (compiler, runtime, embedded
# wasm) changes. `.tn` edits are already live-reloaded by `tint dev` itself.
cd "$(dirname "$0")/.." || exit 1
stamp() { find crates -type f \( -name '*.rs' -o -name '*.js' -o -name '*.wasm' -o -name Cargo.toml \) -not -path '*/target/*' -exec stat -f '%m%N' {} + 2>/dev/null || find crates -type f \( -name '*.rs' -o -name '*.js' -o -name '*.wasm' -o -name Cargo.toml \) -not -path '*/target/*' -exec stat -c '%Y%n' {} +; }
sig() { stamp | sort | cksum; }
trap 'kill $pid 2>/dev/null; exit' INT TERM
while :; do
    cargo build -q -p tint-cli || echo "build failed, waiting for changes..."
    ./target/debug/tint dev sandbox/src/site.tn "$@" & pid=$!
    last=$(sig)
    while kill -0 $pid 2>/dev/null && [ "$(sig)" = "$last" ]; do sleep 1; done
    kill $pid 2>/dev/null; wait $pid 2>/dev/null
done
