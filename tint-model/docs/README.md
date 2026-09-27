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
- `basics.md`, `control-flow.md`, `functions.md`, `types.md` — core TintLogic
- `events.md` — current UI events and handlers

**UI** (`ui/`) — the declarative UI layer
- `ui-syntax.md` — Tint block nodes, state, themes, imports, grid, and routes
- `modifiers.md` — the `::` modifier syntax and semantic groups (`layout`, `paint`, `motion`)
- `styling.md` — the currently implemented layout, paint, gradient, blur, and hover modifiers
- `animations.md` — current transition and hover behavior
- `guide/assets.md` — URL-backed image assets and the resource boundary
- `guide/audio.md` — event sounds and the current audio boundary
- `guide/storage.md` — host storage primitives and browser persistence bridge
- `guide/http.md` — queued HTTP requests and completion callbacks
- `blocks.md` — UI block control flow currently supported by the parser

The browser sandbox is a Tint-rendered app. Run `npm run dev:all` from
`tint-model/sandbox` for Vite HMR, nested `.tn` import reloads, and automatic
Rust/WASM rebuilds. The landing page is `/`; the workbench is `/sandbox`.

GPU/WebGPU rendering, Tint2D, tensors, async execution, resource borrowing,
kits, and the full module system are planned or parser-level experiments. Their
draft notes are kept local only and ignored by Git.

**Project**
- `project-status.md` — what's implemented, what isn't, and the roadmap

## Reading order

New to Tint: `01-architecture.md` → `02-core-model.md` → `guide/basics.md` → `ui/ui-syntax.md`.

For exact current syntax, start with `guide/basics.md` and the UI guides.
