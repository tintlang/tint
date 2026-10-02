# TintLang Documentation

TintLang is an experimental statically typed language for declarative UI and
general logic. The current implementation compiles source through TintIR and
runs it natively or in the browser through WASM. The current browser UI backend
renders directly to the DOM through `DomSession`.

This is the English, restructured edition of the TintLang docs. It replaces the
original `docs/` folder — same topics, less duplication, one canonical place per
subject.

## Why TintLang

- Describe UI and logic in one language
- Rust-inspired syntax with a native and WASM runtime
- A declarative UI DSL with no virtual-DOM framework
- Direct DOM rendering through `DomSession`

## Map of the docs

**Language**
- [`01-architecture.md`](01-architecture.md) — the implemented compiler/runtime pipeline
- [`02-core-model.md`](02-core-model.md) — the one rule everything else follows: Logic Mode vs UI Mode

**Guide** (`guide/`) — narrative, one topic per file, with rationale and examples
- `basics.md`, `control-flow.md`, `functions.md`, `types.md` — core TintLogic,
  including `if` expressions and first-class function types
- [`guide/terminal.md`](guide/terminal.md) — CLI development workflow, terminal I/O,
  `read_key`, and `while`/`loop`
- `events.md` — current UI events and handlers
- [`guide/routing.md`](guide/routing.md) — `app { title, lang }`, `route_path`, the client router and width breakpoints

**UI** (`ui/`) — the declarative UI layer
- `ui-syntax.md` — Tint block nodes, state, themes, imports, grid, and routes
- `text-and-preview.md` — `TextArea`, `Preview`, text natives, storage and `include_str`
- `modifiers.md` — the `::` modifier syntax and semantic groups (`layout`, `paint`, `motion`)
- `styling.md` — the currently implemented layout, paint, gradient, blur, and hover modifiers
- `animations.md` — current transition and hover behavior
- `guide/gestures.md` — pan, swipe, long-press, hotkeys, URL params, derived values
- `guide/assets.md` — URL-backed image assets and the resource boundary
- `guide/audio.md` — event sounds and the current audio boundary
- [`guide/collections.md`](guide/collections.md) — methods on lists, strings, and maps
- `guide/storage.md` — host storage primitives and browser persistence bridge
- `guide/http.md` — queued HTTP requests and completion callbacks
- [`guide/vite.md`](guide/vite.md) — Vite plugin, `.tn` imports, and browser mounting
- [`guide/frameworks.md`](guide/frameworks.md) — React component and Svelte action adapters
- [`guide/escape-hatches.md`](guide/escape-hatches.md) — DOM refs and JavaScript callbacks
- `blocks.md` — UI block control flow currently supported by the parser

The site and sandbox are one Tint program with no HTML, JS or CSS files. Run
`npm run dev` from `tint-model/sandbox` (it runs `tint dev src/site.tn`); after
Rust changes run `npm run wasm`. The landing page is `/`; the workbench is
`/sandbox`.

GPU/WebGPU rendering, Tint2D, tensors, async execution, resource borrowing,
kits, and the full module system are planned or parser-level experiments. Their
draft notes are kept local only and ignored by Git.

**Project**
- `project-status.md` — what's implemented, what isn't, and the roadmap

## Reading order

New to Tint: `01-architecture.md` → `02-core-model.md` → `guide/basics.md` → `ui/ui-syntax.md`.

For exact current syntax, start with `guide/basics.md` and the UI guides.
