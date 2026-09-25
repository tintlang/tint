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
- `gpu-kernels.md`, `tint2d.md`, `tensors.md` — GPU compute, the 2D engine, and the tensor/ML layer
- `kits.md` — reusable field groups for structs (`kit`)

**UI** (`ui/`) — the declarative UI layer
- `ui-syntax.md` — `<Block>` vs rendered components, state, the full example
- `modifiers.md` — the `{}` modifier DSL (layout, offset, visual, color, gradients)
- `styling.md` — TintStyle/TintColor and combo effects (`shadow+inner{...}`)
- `animations.md` — `animate{}`, triggers, timelines, frame-based physics
- `blocks.md`, `panels.md` — the `<Block>` and `<Panel>` components
- `slots.md` — the slot system (`<HeaderSlot/>`, `<Children/>`, ...)

**Project**
- `project-status.md` — what's implemented, what isn't, and the roadmap
- `examples-and-drafts.md` — informal syntax experiments and scratch examples, kept for reference but not part of the spec

## Reading order

New to Tint: `01-architecture.md` → `02-core-model.md` → `guide/basics.md` → `ui/ui-syntax.md`.

Looking up exact syntax: go straight to `03-language-reference.md`.
