# Changelog

All notable changes to Tint are documented here.

## [Unreleased]

- UI rendering is much cheaper per frame. The browser renderer keeps the previous render tree and diffs in Rust, touching the DOM only where something changed. The builder reuses a pure subtree (no calls, blocks, lambdas or `if`/`match` expressions) when none of the outer variables it read has changed, and shares it as an `Rc` so the renderer can skip it by pointer. `UiRenderNode.children` is now `Vec<Rc<UiRenderNode>>`; `UiSession::render`/`dispatch` return `Vec<Rc<UiRenderNode>>` (`UiSession::set_reuse(false)` turns reuse off). Pong frame time went from 11.5 ms to 1.4 ms in headless Chromium.
- The tree-walking evaluator is the single execution engine on the runtime path. The SSA IR moved behind the optional `ir` cargo feature of `tint-runtime` (off by default).
- Added an engine-agnostic conformance suite (`tint-model/conformance/*.tn`, run by `tint-runtime/tests/conformance.rs`); known language gaps are recorded as expected failures.
- Added methods on lists, strings, and maps (see `docs/guide/collections.md`) and `tick||`/`every||` timers.
- Checker typing: return types of functions/methods without `-> T` are inferred, unannotated parameters take their type from call sites (when all agree), `frame||` handlers get `dt: number` and `key_down||`/`key_up||` get `key: string`, `&mut self` is recognised as `self`, and `state` initialisers give handlers typed state. Share of expressions left `Unknown` on the census corpus: 32% -> 5.4% (`cargo test -p tint-semantics --test type_census -- --nocapture --ignored`). Still open: lambda parameters and `Option`/`Result` holes such as `None {}`.
- Functions and UI functions are stored as `Rc`, so calling a function or rendering a `ui fn` no longer deep-clones its AST.
- Pong: the field is four wall rectangles instead of 288 cells (818 -> 53 DOM nodes), and the overlays are no longer rendered 12 times each (a `for{y in rows}` modifier was repeating the whole field).
- Added native terminal I/O: `print`, `println`, `read_line`, `parse_number`, and Unicode-aware `read_key`.
- Added `while`, `loop`, `break`, and `continue` for terminal and logic programs.
- Added typed numeric values: `i32`, `i64`, `u8`, `u32`, `u64`, `f32`, and `f64`, with explicit `as` conversions and range checks.
- Added typed `const` declarations.
- Added `Option<T>`/`Result<T, E>` flow with `expect`, `unwrap`, postfix `?`, `is_some`, `is_none`, `is_ok`, `is_err`, `unwrap_or`, `map`, and `and_then`.
- Added strict generic user-defined `struct` and `enum` types with checked type arguments and payloads.

## [0.1.1] - 2026-09-28

- Unified the project version across the Rust crates, VS Code extension, sandbox, and generated WASM package metadata.
- Added `VERSION` as the single source of truth and a synchronization script for release metadata.
- Added end-to-end lambda execution with lexical capture through the tree-walking evaluator, with a documented IR fallback boundary.
- Added typed semantic checking for functions, UI functions, and `impl` methods, including inferred primitive, collection, struct, enum, function, and lambda types.
- Added checks for operators, conditions, assignments, returns, function and method arguments, struct fields, enum constructors and patterns, UI state, interpolation, and UI control flow.
- Added regression coverage for lambdas, semantic type checking, and enum-typed fields.
- Added UI tokens, reusable styles, components, slots, and variants with theme-aware resolution.
- Added first-class function types and function-valued runtime behavior.
- Added the shared prebuilt `@tintlang/runtime` package with source and versioned bytecode loading through one browser runtime.
- Added `@tintlang/vite` for importing `.tn` files as HMR-friendly JS modules with `source` and `entry` exports.
- Added `target||"_blank"` for new-tab links, with automatic `rel="noopener noreferrer"` in the DOM renderer.
- Added `ref||"name"` DOM escape hatches and `js||callback` host callbacks through the runtime mount API.
- Added `@tintlang/react` and `@tintlang/svelte` adapters for mounting Tint inside existing framework applications.
