# Tint Sandbox

The sandbox workbench is itself rendered by Tint through `DomSession`. It has
one Tint topbar, a CodeMirror editor, a live Tint UI preview, and a small output
area. There is no Svelte application or sidebar.

The Rust/WASM runtime owns the UI tree and its themes. JavaScript is only the
host adapter for CodeMirror and the preview session:

- `src/sandbox.tn` — the workbench layout and dark/light variants;
- `src/landing/topbar.tn` — the shared Tint topbar;
- `src/sandbox.js` — WASM bootstrap, CodeMirror, check/run, and preview mounting;
- `src/lib/tintLanguage.js` — CodeMirror Tint tokenizer;
- `pkg-web/` — generated `tint-wasm` bindings.

## Run it

```bash
npm install
npm run dev
```

Open `http://localhost:5173/app.html`. The landing page is available at
`http://localhost:5173/`.

Use **Cmd/Ctrl + Enter** to run the current source. `Check` parses it without
executing it. The preview keeps a persistent `UiSession`, so `state` and
`click||handler` interactions survive between renders.

## Rebuild WASM

After changing the Tint parser or runtime:

```bash
cd ../
wasm-pack build crates/tint-wasm --target web --out-dir ../sandbox/pkg-web
```

If the local `wasm-opt` binary is unavailable, use `--dev`; the generated
bindings are still valid for the sandbox.
