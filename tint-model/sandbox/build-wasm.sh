#!/bin/sh
# Rebuilds the browser runtime and embeds it in the tint CLI, so `tint dev` and
# `tint build` ship the current DOM layer. Needs the wasm32 target and
# wasm-bindgen-cli. The runtime is embedded gzipped (the page inflates it with
# DecompressionStream), the JS glue is minified and the bytecode API (serde_json)
# is left out: pages are ~6x smaller.
set -eu
cd "$(dirname "$0")/.."
OUT=crates/tint-cli/embedded
cargo build -p tint-wasm --no-default-features --target wasm32-unknown-unknown --profile wasm
wasm-bindgen target/wasm32-unknown-unknown/wasm/tint_wasm.wasm --target web --out-dir "$OUT" --no-typescript --remove-name-section --remove-producers-section
# wasm-opt (binaryen) shrinks the module a further ~15%; skipped when unavailable.
if command -v wasm-opt >/dev/null 2>&1; then WASM_OPT=wasm-opt; else WASM_OPT="npx --yes -p binaryen wasm-opt"; fi
$WASM_OPT -Os --enable-bulk-memory --enable-sign-ext --enable-mutable-globals --enable-nontrapping-float-to-int --enable-multivalue --enable-reference-types \
    -o "$OUT/opt.wasm" "$OUT/tint_wasm_bg.wasm" && mv "$OUT/opt.wasm" "$OUT/tint_wasm_bg.wasm" || echo "wasm-opt unavailable, shipping the unoptimized module"
gzip -9nf "$OUT/tint_wasm_bg.wasm"
# Whitespace/syntax only: build.rs refers to the exported names directly.
npx --yes esbuild "$OUT/tint_wasm.js" --minify-whitespace --minify-syntax --allow-overwrite --outfile="$OUT/tint_wasm.js"
