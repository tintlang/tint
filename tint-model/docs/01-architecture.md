# Architecture

TintLang is an experimental standalone language with its own syntax, AST, IR,
runtime, and declarative UI layer. Rust provides the implementation and native
tooling; the browser target runs the runtime through WASM.

## Current subsystems

1. **TintLogic** — expressions, functions, control flow, and values
2. **TintUI** — declarative nodes, modifiers, themes, and events
3. **TintIR** — the intermediate representation used by the runtime
4. **TintVM** — native/WASM execution and UI state
5. **DomSession** — the current direct-DOM browser adapter

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
│ Direct DOM UI renderer                   │
│ Reactive UI session (state / events)     │
│ Native/WASM runtime                      │
└─────────────────────────────────────────┘
```

TintIR sits between the AST and the VM, playing the role WASM/Rust-MIR/Lua-bytecode
play elsewhere: it stabilizes the language across versions, gives the compiler room
to optimize, and guarantees identical behavior on WASM and native builds.

TintVM executes TintIR and owns the current runtime behavior: logic evaluation,
UI state, layout/style resolution, and event dispatch. It runs natively or
through WASM in the browser.

**TintUI** lays out and renders the declarative tree: named nodes, text, layout,
paint, transitions, hover states, and events. The current browser backend is a
direct DOM renderer (`DomSession`) with no UI framework.

## Rendering pipeline

```
TintLogic (UI state)
      │
TintUI render tree
      │
Layout engine
      │
DomSession / web-sys
      │
DOM elements
```

## UI rendering

Tint UI is written as named block nodes and rendered directly by the active
backend. In the browser, `DomSession` maps nodes to DOM elements, applies the
resolved `layout`/`paint`/`motion` styles, and binds named event attributes.
`route||"/path"` becomes a normal browser link.

## Current boundaries

The browser package uses WASM for Tint execution, JavaScript only for the
CodeMirror host and DOM integration, and CSS for the surrounding workbench.
The current public runtime does not promise GPU isolation, FFI permissions,
WebGPU rendering, or a standalone embedded-WASM HTML artifact.

Native Rust integration is available through the runtime's registered native
functions. The exact API is implemented in the Rust crates and is covered by
the native function tests.

## Summary

Tint currently provides a Rust-implemented parser/runtime, a declarative UI
model, persistent UI sessions, direct DOM rendering, native tooling, and a WASM
browser sandbox. GPU/WebGPU, async, and resource features remain future work.
