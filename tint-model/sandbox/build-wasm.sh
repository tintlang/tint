#!/bin/sh
# Rebuilds the browser runtime and embeds it in the tint CLI, so `tint dev` and
# `tint build` ship the current DOM layer. Needs the wasm32 target and
# wasm-bindgen-cli.
set -eu
cd "$(dirname "$0")/.."
cargo build -p tint-wasm --target wasm32-unknown-unknown --release
wasm-bindgen target/wasm32-unknown-unknown/release/tint_wasm.wasm --target web --out-dir crates/tint-cli/embedded --no-typescript
