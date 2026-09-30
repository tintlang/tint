# Tint Sandbox

> **Status: working prototype.** An editor and live preview, not a full IDE:
> no file tree, tabs or project workspace.

The whole site is one Tint program. There are no HTML, JS or CSS files: the
`tint` CLI generates the page shell, and everything else is `.tn`.

- `src/site.tn` — entry: `app { route.* }` maps URLs to the landing page, Pong, Pac-Man and the sandbox;
- `src/sandbox.tn` — the workbench (editor, resizer, live preview), dark and light;
- `src/editor/` — the syntax-highlighting editor: a transparent `TextArea` over a layer highlighted by `tint_highlight`;
- `src/landing/` — topbar, hero and the landing demo (same editor + `Preview`);
- `src/tint-pong/` — the Pong game (`src/pacman/` is hidden until it is finished).

## Run it

```bash
npm run dev      # tint dev src/site.tn  → http://localhost:5173
npm run build    # tint build src/site.tn -o dist/index.html
```

Routes: `/` (landing), `/sandbox`, `/pong`; the router switches
between them without reloading. Serve `dist/index.html` for every path (see
`vercel.json`).

The preview recompiles the editor contents on every edit and keeps its state;
invalid source is shown in the preview pane. The theme and the sandbox code
are kept in `localStorage`.

After changing Rust code under `crates/tint-wasm` or `crates/tint-runtime`,
run `npm run wasm` to rebuild the browser runtime that the CLI embeds.
See [`CONTRIBUTING.md`](../../CONTRIBUTING.md) for the rest of the setup.
