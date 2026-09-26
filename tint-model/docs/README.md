# TintLang Documentation

TintLang is a safe, statically-typed, WebGPU-first language for building UIs, tools,
games and 2D/ML applications without HTML, CSS, or JavaScript. It compiles to a
bytecode IR and runs on TintVM, in the browser (WASM) or natively (Rust).

Tint = WebGPU + WASM + strict types + declarative UI.

This is the English, restructured edition of the TintLang docs. It replaces the
original `docs/` folder — same topics, less duplication, one canonical place per
subject.

## Why TintLang

- Replace HTML/CSS/JS with a single language
- Render UI on the GPU instead of the DOM
- Rust-level static typing, without a borrow checker
- Safe WASM execution, sandboxed by the VM
- A declarative UI DSL with no virtual-DOM framework
- Full, direct control over WebGPU

## Map of the docs

**Language**
- [`01-architecture.md`](01-architecture.md) — how the pieces fit together (TintLogic, TintUI, Tint2D, TintGPU, TintVM)
- [`02-core-model.md`](02-core-model.md) — the one rule everything else follows: Logic Mode vs UI Mode
- [`03-language-reference.md`](03-language-reference.md) — the full syntax spec (terse, precise, grouped by topic)

**Guide** (`guide/`) — narrative, one topic per file, with rationale and error examples
- `basics.md`, `control-flow.md`, `functions.md`, `types.md` — core TintLogic
- `events.md`, `async.md` — events and asynchronous code
- `modules.md` — the file-based module system and `run.tn`
- `resources-and-borrowing.md` — `buffer` / `image` / `tensor` / `vec` and the `borrow` model
- `gpu-kernels.md`, `tensors.md` — GPU compute and the tensor/ML layer
- `kits.md` — reusable field groups for structs (`kit`)

**UI** (`ui/`) — the declarative UI layer
- `ui-syntax.md` — Tint block nodes, state, themes, imports, grid, and routes
- `modifiers.md` — the `::` modifier syntax and semantic groups (`layout`, `paint`, `motion`)
- `styling.md` — the currently implemented layout, paint, gradient, blur, and hover modifiers
- `animations.md` — current transition and hover behavior
- `blocks.md` — UI block control flow currently supported by the parser

The browser sandbox is a Tint-rendered app. Run `npm run dev:all` from
`tint-model/sandbox` for Vite HMR, nested `.tn` import reloads, and automatic
Rust/WASM rebuilds. The landing page is `/`; the workbench is `/sandbox`.

**Project**
- `project-status.md` — what's implemented, what isn't, and the roadmap

## Reading order

New to Tint: `01-architecture.md` → `02-core-model.md` → `guide/basics.md` → `ui/ui-syntax.md`.

Looking up exact syntax: go straight to `03-language-reference.md`.
