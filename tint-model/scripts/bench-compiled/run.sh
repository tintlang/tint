#!/usr/bin/env bash
# Compiled-mode UI bench: rebuilds everything and runs the list ops + Pong frame timing.
# Usage: scripts/bench-compiled/run.sh   (from the tint-model directory)
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
D="$ROOT/scripts/bench-compiled"
cd "$ROOT"
cargo build --release -p tint-wasmgen --examples
cargo build --profile wasm --target wasm32-unknown-unknown -p tint-wasm --no-default-features --features compiled
wasm-bindgen --target web --no-typescript --out-dir "$D/pkg" target/wasm32-unknown-unknown/wasm/tint_wasm.wasm
target/release/examples/compile_app "$D/bench.tn" "$D/bench.wasm"
target/release/examples/compile_app sandbox/src/site.tn "$D/site.wasm"
cd "$D"
[ -d node_modules/playwright ] || { npm init -y >/dev/null; npm i playwright; npx playwright install chromium; }
echo "== list ops (1k rows, median ms per dispatch, incl. forced layout) =="
node b6.js bench.wasm
echo "== Pong rAF callback (5 s) =="
node pong.js
