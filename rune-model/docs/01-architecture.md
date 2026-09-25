# Architecture

RuneLang is not a DSL bolted onto Rust — it is a standalone language with its own
syntax, module system, VM, IR, bytecode, GPU layer (RuneGPU), 2D engine (Rune2D),
and UI engine (RuneUI). Rust is only needed if you want native modules, system
access, or hand-written low-level GPU kernels.

## Five subsystems

1. **RuneLogic** — the Rust-like core language (no borrow checker; the VM manages memory safety)
2. **RuneUI** — declarative UI on WebGPU (or a hybrid-DOM mode)
3. **Rune2D** (`gpu2d`) — a high-level, GPU-only 2D drawing engine
4. **RuneGPU** — low-level WebGPU compute kernels
5. **RuneVM** — the virtual machine that executes RuneIR and drives rendering

## Compile pipeline

```
Rune source (*.rn)
      │
   Lexer / Parser
      │
     AST
      │
   RuneIR (stable bytecode IR)
      │
    RuneVM
      │
┌─────────────────────────────────────────┐
│ UI Renderer (GPU / hybrid DOM)           │
│ Rune2D Engine (Canvas2D-equivalent, GPU) │
│ RuneGPU Compute Kernels                  │
│ Reactive Runtime (state / signal)        │
└─────────────────────────────────────────┘
```

RuneIR sits between the AST and the VM, playing the role WASM/Rust-MIR/Lua-bytecode
play elsewhere: it stabilizes the language across versions, gives the compiler room
to optimize, and guarantees identical behavior on WASM and native builds.

RuneVM executes RuneIR and owns the runtime: a bytecode interpreter, registers and
stack, arena memory, an object pool, the UI diff/layout engine, a scheduler for
async/event/frame work, and a GPU dispatcher for Rune2D and compute kernels. It runs
either compiled to WASM in the browser, or natively via a Rust implementation.

## Three GPU layers

Rune is unusual in spanning the whole GPU stack, from UI down to compute, in one
language:

```
HIGH LEVEL   RuneUI       declarative UI (<Panel>, <Text>, ...)
MID LEVEL    Rune2D       gpu2d { ... } — a GPU-only Canvas2D equivalent
LOW LEVEL    RuneGPU      kernel { ... } — WGSL-like compute
```

**RuneUI** lays out and renders the declarative tree: panels, text, icons, blend,
shadow, blur, animations, vector rendering. This is the mode Lynbor-style editors
and tools use.

**Rune2D** is a declarative, 100% WebGPU "Canvas2D": built-in shadow/blur/glow,
returns a `Texture`, and uses a real path engine instead of raster hacks. See
`guide/rune2d.md`.

**RuneGPU** is the WGSL-like compute DSL for GPGPU, postprocessing, ML inference,
procedural content, particles, and physics. See `guide/gpu-kernels.md`.

## Rendering pipeline

```
RuneLogic (UI state)
      │
RuneUI virtual tree
      │
Layout engine
      │
Rune2D commands (gpu2d)
      │
RuneGPU kernels (post-processing)
      │
WebGPU / wgpu-native backend
      │
Final frame
```

## UI rendering modes

**Zero-HTML Mode** — the UI is rendered entirely through WebGPU; there is no DOM
node anywhere. This is the default for editors and tools.

**Hybrid DOM Mode** — `<Text>` becomes `<div>`, `<Button>` becomes `<button>`, and so
on. Used for landing pages, SEO, and conventional web apps.

### RST — Rune Semantic Tree (SEO)

In Hybrid DOM Mode, RuneVM can additionally emit a semantic, read-only DOM mirror of
the UI purely for search engines — the **Rune Semantic Tree (RST)**:

- WebGPU stays the source of the visible UI
- the DOM mirror is generated automatically by RuneVM, not by hand-written Rust/JS
- it does not accept user events
- it updates reactively, like a read-only shadow DOM

```
<Panel>
    <Text>"Hello"</Text>
</Panel>
```

becomes, in the RST:

```
<rune-panel>
    <rune-text>Hello</rune-text>
</rune-panel>
```

RST is only active in Hybrid DOM Mode; it is disabled in Zero-HTML Mode.

### WebGL2 fallback

If WebGPU is unavailable, Rune falls back automatically: RuneGPU switches to a
WebGL2 backend, Rune2D uses a WebGL2 raster mode, UI falls back to a software GPU
renderer, and RST keeps working. The priority order is WebGPU → WebGL2 → an error
screen (or a custom fallback UI) if neither is available. A Rune app always starts.

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

Rune apps cannot execute arbitrary JS, `eval`, or inject into the DOM; there is no
XSS surface. Execution is sandboxed by WASM, FFI permissions are explicit, and the
GPU context is isolated.

## Rust integration

Rust is the systems backend RuneUI, Rune2D and RuneGPU share (via `wgpu-native`).

Embedding RuneVM in Rust:

```rust
let mut vm = RuneVM::new();
vm.load("app.rn");
vm.call("main");
```

Calling Rust from Rune:

```
extern fn log(msg: string);

fn test() {
    log("Hello from Rune");
}
```

## Design principles

1. GPU-first — everything renders through WebGPU or wgpu-native
2. WASM-first — the browser is a native target, not reached through JS
3. Zero HTML/CSS/JS
4. Static typing everywhere
5. Declarative-first UI and 2D
6. No undefined behavior — the VM is a sandboxed execution environment
7. Clean separation of layers: Logic ↔ UI ↔ Rune2D ↔ Kernels
8. Predictable performance — no GC, stable frame times
9. Reactivity without a framework
10. SEO through RST, not DOM hacks

## Summary

RuneLang unifies a Rust-like strict logic language, a declarative UI system, native
WebGPU graphics, a high-level 2D engine, low-level compute kernels, sandboxed
execution, WASM and native targets, and direct Rust interop — one architecture for
editors, tools, sites, games, 2D/3D visualization, and (see `guide/tensors.md`)
GPU-native ML.
