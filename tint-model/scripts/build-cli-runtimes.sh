#!/usr/bin/env bash
# Rebuilds the browser runtimes `tint build` embeds (crates/tint-cli/embedded/):
#   rt_dom        runtime of apps compiled to WebAssembly (no interpreter), size-optimized
#   rt_dom_full   the same plus the interpreter, for apps that use `Preview`
# Needs: rustup target wasm32-unknown-unknown, wasm-bindgen-cli (same version as the crate).
set -euo pipefail
cd "$(dirname "$0")/.."
out=crates/tint-cli/embedded
build() { # name profile features
  local name=$1 profile=$2 features=$3
  cargo build --profile "$profile" --target wasm32-unknown-unknown -p tint-wasm --no-default-features --features "$features"
  local tmp
  tmp=$(mktemp -d)
  wasm-bindgen --target web --no-typescript --out-dir "$tmp" "target/wasm32-unknown-unknown/$profile/tint_wasm.wasm"
  cp "$tmp/tint_wasm.js" "$out/$name.js"
  gzip -9 -c "$tmp/tint_wasm_bg.wasm" > "$out/${name}_bg.wasm.gz"
  rm -rf "$tmp"
  ls -l "$out/${name}_bg.wasm.gz"
}
build rt_dom wasm-small compiled,lean-ast
build rt_dom_full wasm compiled,interpreter
build tint_wasm wasm interpreter
