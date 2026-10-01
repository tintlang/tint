#!/bin/sh
# Dev server that also rebuilds the CLI and restarts when the Rust side
# (compiler, runtime, embedded wasm) changes, e.g. after `git pull`.
# `.tn` edits are already live-reloaded by `tint dev` itself.
cd "$(dirname "$0")/.." || exit 1
FILES='-name *.rs -o -name *.js -o -name *.wasm -o -name *.gz -o -name Cargo.toml'
stamp() { find crates Cargo.lock -type f \( $FILES -o -name Cargo.lock \) -not -path '*/target/*' -exec stat -f '%m%N' {} + 2>/dev/null || find crates Cargo.lock -type f \( $FILES -o -name Cargo.lock \) -not -path '*/target/*' -exec stat -c '%Y%n' {} +; }
sig() { stamp | sort | cksum; }
trap 'kill $pid 2>/dev/null; exit' INT TERM
first=1
while :; do
    last=$(sig)
    [ "$first" = 1 ] || echo "dev: Rust sources changed, rebuilding tint..."
    first=0
    if cargo build -q -p tint-cli -p tint-analyzer; then
        ./target/debug/tint dev sandbox/src/site.tn "$@" & pid=$!
        while kill -0 $pid 2>/dev/null && [ "$(sig)" = "$last" ]; do sleep 1; done
        kill $pid 2>/dev/null; wait $pid 2>/dev/null
    else
        # Never serve a stale binary: wait for the next change and retry.
        echo "dev: build failed, waiting for changes..."
        while [ "$(sig)" = "$last" ]; do sleep 1; done
    fi
done
