# Tint Sandbox

> **Status: working prototype.** The workbench is deliberately small: it is an
> editor and live preview, not a full IDE. It does not provide file trees,
> multi-tab editing, or a persistent project workspace.

The sandbox workbench is itself rendered by Tint through `DomSession`. It has
one Tint topbar, a CodeMirror editor, and a live Tint UI preview. The old
`Check`/`Run` output bar is currently hidden because preview compilation is
automatic.

The Rust/WASM runtime owns the UI tree and its themes. JavaScript is only the
host adapter for CodeMirror and the preview session:

- `src/sandbox.tn` — the workbench layout and dark/light variants;
- `src/landing/topbar.tn` — the shared Tint topbar;
- `src/sandbox.js` — WASM bootstrap, CodeMirror, and preview mounting;
- `src/lib/tintLanguage.js` — CodeMirror Tint tokenizer;
- `pkg-web/` — generated `tint-wasm` bindings.

## Run it

```bash
npm install
npm run dev
```

Open `http://localhost:5173/app.html`. The landing page is available at
`http://localhost:5173/`.

The preview recompiles the current editor contents automatically. It keeps a
persistent `UiSession`, so `state` and `click||handler` interactions survive
between renders. Invalid source is shown in the preview pane.

Contributor setup, runtime rebuilds, generated artifacts, and release checks
are documented in the repository [`CONTRIBUTING.md`](../../CONTRIBUTING.md).
