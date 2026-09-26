# Architecture

TintLang is not a DSL bolted onto Rust — it is a standalone language with its own
syntax, module system, VM, IR, bytecode, GPU layer (TintGPU), 2D engine (Tint2D),
and UI engine (TintUI). Rust is only needed if you want native modules, system
access, or hand-written low-level GPU kernels.

## Five subsystems

1. **TintLogic** — the Rust-like core language (no borrow checker; the VM manages memory safety)
2. **TintUI** — declarative UI on WebGPU (or a hybrid-DOM mode)
3. **Tint2D** (`gpu2d`) — a high-level, GPU-only 2D drawing engine
4. **TintGPU** — low-level WebGPU compute kernels
5. **TintVM** — the virtual machine that executes TintIR and drives rendering

## Compile pipeline

```
Tint source (*.tn)
      │
   Lexer / Parser
      │
     AST
      │
   TintIR (stable bytecode IR)
      │
    TintVM
      │
┌─────────────────────────────────────────┐
│ UI Renderer (GPU / hybrid DOM)           │
│ Tint2D Engine (Canvas2D-equivalent, GPU) │
│ TintGPU Compute Kernels                  │
│ Reactive Runtime (state / signal)        │
└─────────────────────────────────────────┘
```

TintIR sits between the AST and the VM, playing the role WASM/Rust-MIR/Lua-bytecode
play elsewhere: it stabilizes the language across versions, gives the compiler room
to optimize, and guarantees identical behavior on WASM and native builds.

TintVM executes TintIR and owns the runtime: a bytecode interpreter, registers and
stack, arena memory, an object pool, the UI diff/layout engine, a scheduler for
async/event/frame work, and a GPU dispatcher for Tint2D and compute kernels. It runs
either compiled to WASM in the browser, or natively via a Rust implementation.

## Three GPU layers

Tint is unusual in spanning the whole GPU stack, from UI down to compute, in one
language:

```
HIGH LEVEL   TintUI       declarative UI (Panel, Text, ...)
MID LEVEL    Tint2D       gpu2d { ... } — a GPU-only Canvas2D equivalent
LOW LEVEL    TintGPU      kernel { ... } — WGSL-like compute
```

**TintUI** lays out and renders the declarative tree: named nodes, text, layout,
paint, transitions, hover states, and events. The current browser backend is a
direct DOM renderer (`DomSession`) with no UI framework.

**Tint2D** is a declarative, 100% WebGPU "Canvas2D": built-in shadow/blur/glow,
returns a `Texture`, and uses a real path engine instead of raster hacks. See
`guide/tint2d.md`.

**TintGPU** is the WGSL-like compute DSL for GPGPU, postprocessing, ML inference,
procedural content, particles, and physics. See `guide/gpu-kernels.md`.

## Rendering pipeline

```
TintLogic (UI state)
      │
TintUI virtual tree
      │
Layout engine
      │
Tint2D commands (gpu2d)
      │
TintGPU kernels (post-processing)
      │
WebGPU / wgpu-native backend
      │
Final frame
```

## UI rendering

Tint UI is written as named block nodes and rendered directly by the active
backend. In the browser, `DomSession` maps nodes to DOM elements, applies the
resolved `layout`/`paint`/`motion` styles, and binds named event attributes.
`route||"/path"` becomes a normal browser link.

### WebGL2 fallback

If WebGPU is unavailable, Tint falls back automatically: TintGPU switches to a
WebGL2 backend, Tint2D uses a WebGL2 raster mode, UI falls back to a software GPU
renderer, and RST keeps working. The priority order is WebGPU → WebGL2 → an error
screen (or a custom fallback UI) if neither is available. A Tint app always starts.

## Memory model

- arena allocation
- variables are immutable by default
- reference-counted objects
- no raw pointers
- no GC pauses, deterministic destruction

This gives Rust-level safety without a borrow checker in the surface language (the
lower-level `borrow{}` block, described in `guide/resources-and-borrowing.md`, is a
separate, narrower mechanism for system resources).

## Security model

Tint apps cannot execute arbitrary JS, `eval`, or inject into the DOM; there is no
XSS surface. Execution is sandboxed by WASM, FFI permissions are explicit, and the
GPU context is isolated.

## Rust integration

Rust is the systems backend TintUI, Tint2D and TintGPU share (via `wgpu-native`).

Embedding TintVM in Rust:

```rust
let mut vm = TintVM::new();
vm.load("app.tn");
vm.call("main");
```

Calling Rust from Tint:

```
extern fn log(msg: string);

fn test() {
    log("Hello from Tint");
}
```

## Design principles

1. GPU-first — everything renders through WebGPU or wgpu-native
2. WASM-first — the browser is a native target, not reached through JS
3. Zero HTML/CSS/JS
4. Static typing everywhere
5. Declarative-first UI and 2D
6. No undefined behavior — the VM is a sandboxed execution environment
7. Clean separation of layers: Logic ↔ UI ↔ Tint2D ↔ Kernels
8. Predictable performance — no GC, stable frame times
9. Reactivity without a framework
10. SEO through RST, not DOM hacks

## Summary

TintLang unifies a Rust-like strict logic language, a declarative UI system, native
WebGPU graphics, a high-level 2D engine, low-level compute kernels, sandboxed
execution, WASM and native targets, and direct Rust interop — one architecture for
editors, tools, sites, games, 2D/3D visualization, and (see `guide/tensors.md`)
GPU-native ML.
