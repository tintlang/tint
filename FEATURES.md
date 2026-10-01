# Features and status

Tint is experimental. This page lists what is implemented today and what is
still missing. The short introduction is in the [README](README.md).

## Feature list

- **Compiler pipeline**: Lexer → Parser → AST → Semantic Checker → tree-walking evaluator (the single execution engine; the SSA IR + optimizer is an optional `ir` cargo feature of `tint-runtime`, off by default)
- **UI as a first-class value**: Tint owns the UI tree, styling and rendering. You write `.tn` only for UI and UI logic; `tint dev`/`tint build` generate the HTML shell, styles, fonts, animations and routes, so no hand-written HTML, JS or CSS is needed (CSS and JS escape hatches remain for host integration)
- **Semantic modifier groups**: `layout::{...}`, `paint::{...}`, and `motion::{...}`; flat modifiers remain accepted for compatibility
- **Control flow**: `if/else` statements and expressions, `for` loops, functions with persistence, full comparison operators (`< > <= >= == !=`) alongside `&& ||`
- **First-class functions**: typed function values such as `fn(i32, f32, bool) -> bool`, named-function callbacks, and lambdas
- **Real state + events**: `state`, `click||`, `hover_in||`/`hover_out||` drive a persistent, re-rendering session
- **Responsive design**: `mobile::{}`/`tablet::{}`/`laptop::{}`/`desktop::{}` style breakpoints, plus a host-exposed `viewport_width` variable for structural `if{}` layout swaps (see `tint-model/sandbox/src/landing/`)
- **Layout primitives**: flex direction and a typed `grid::{ columns, rows, gap }` modifier with `minmax(0, 1fr)`-friendly tracks
- **Internal routing**: `route||"/path"` renders as a normal browser link, including `/` and `/sandbox`
- **New-tab links**: add `target||"_blank"` to routed links; the DOM renderer adds `rel="noopener noreferrer"`
- **Direct DOM rendering**: `DomSession` renders Tint through Rust/`web-sys`, without a UI framework
- **Prebuilt browser runtime**: `@tintlang/runtime` ships the Rust/WASM runtime and JS/TS host API; application authors do not rebuild WASM
- **Vite integration**: `@tintlang/vite` imports `.tn` files as HMR-friendly source modules with `source` and `entry` exports
- **Interactive sandbox**: the whole site is one `.tn` program; the highlighted editor (`TextArea` + real-lexer `tint_highlight`) and live `Preview` are Tint too -- no HTML, JS or CSS files
- **CLI tools**: Check syntax, run, and generate a prototype HTML shell
- **Terminal I/O**: `print`, `println`, `read_line`, `parse_number`, and
  `read_key` for native terminal programs
- **Collections and strings**: methods on lists, strings, and maps (`len`, `push`,
  `map`, `filter`, `join`, `split`, `trim`, `keys`, ...) -- see
  [`tint-model/docs/guide/collections.md`](tint-model/docs/guide/collections.md)
- **Result handling**: `Option<T>`, `Result<T, E>`, `Some`/`None`, `Ok`/`Err`,
  `expect`, `unwrap`, and postfix `?` error propagation
- **Numeric types**: real `i32`, `i64`, `u8`, `u32`, `u64`, `f32`, and `f64`
  values with explicit `as` conversions and range checks; `number` is `f64`
- **Logic loops**: `while`, `loop`, `break`, and `continue`
- **Native Rust interop**: `TintVM::register_native("name", |args| ...)` registers a real Rust closure that `.tn` source calls directly by name -- no Rust syntax inside the language, no reimplementing rustc's borrow checker, just an ordinary Rust function called across the boundary (see `tint run <file> now_ms`, a demo native in `tint-cli`)
- **Multi-file modules**: `mod name;` resolves to `name.tn` or `name/mod.tn` (the `mod.rs` convention); `use path::to::item;` + `export` control what crosses file boundaries (see `examples/modules/`)
- **VSCode extension**: Syntax highlighting + build commands


## Working today

- `app { title, lang, router }` page metadata, `route_path` and a client router (`tint dev` serves it; see `tint-model/docs/guide/routing.md`)
- Width breakpoints `max-N::{}` / `min-N::{}` and negative modifier numbers (`margin.b::-72`)
- Keyword CSS properties (`cursor`, `pointer-events`, `user-select`, `aspect-ratio`, ...) and built-in reset for Tint nodes

- Proper compiler architecture (not hacked)
- UI grammar with modifiers
- Control flow (if/else statements and expressions, for loops, comparison operators)
- Function definitions, typed function values & calls
- Persistent REPL sessions
- Browser runtime (Tint is interpreted by a prebuilt WebAssembly runtime) & rendering
- Real state + click/hover handlers (`state`, `click||`, `hover_in||`/`hover_out||`) via a persistent `UiSession`
- Responsive layouts (`mobile::{}`/`tablet::{}`/`laptop::{}`/`desktop::{}` breakpoints + structural `if{ viewport_width ... }`)
- A second, direct-DOM rendering backend (`DomSession`, no JS framework)
- CLI tooling
- Experimental `tint-analyzer` LSP with diagnostics, hover types, and go-to-definition for open documents
- VSCode integration
- Hover effects & animations
- Grid layout and internal route links
- `tint dev` / `tint build` serve and bundle a multi-page `.tn` site with `app { route.* }`; `npm run wasm` rebuilds the embedded runtime
- `TextArea` (`input||`, `submit||`), `Preview` (nested session), `include_str`, `storage_get_or`, text natives (`tint_highlight`, `line_numbers`, …)
- A native fn called from *inside* a plain `fn`'s own body, invoked the normal top-level way through the IR VM -- `IrVM` now falls back to a native-lookup callback (`set_native_call`) when no IR-compiled function matches the callee name, backed by `TintVM::native_fns` (see `tint-runtime/tests/native_fn.rs`)
- Multi-file modules (`mod`/`use`/`export`) via a `tint-cli`-side loader -- absolute paths only (no `self::`/`super::`), but grouped (`use a::{b, c}`), `as`-aliased, and wildcard (`use a::*`) imports all work alongside the plain form; `fn`/`struct`/`enum` are exportable
- An evolving semantic/type checker with inference over `fn`, `ui fn`, and `impl` method bodies (including inferred return types and call-site parameter types) -- `tint check` fails on errors, `tint run` warns and still executes. Built-in and user-defined generic structs/enums are checked strictly; some host-specific UI contracts remain unsupported.
- One UI syntax: the earlier XML dialect (`<Tag ...>...</Tag>`) has been removed -- `Tag { ... }` block syntax is now the only way to write UI, including `match{}`'s case arms (see below)
- `children { ... }` -- an optional grouping tag for a node's own children, so they don't blur together with several modifiers in the same block; splices straight into its parent, no wrapper node of its own (see `tint-runtime/tests/render_ui.rs`)


## Known gaps

- Multiple independent component instances (state is one flat scope per `UiSession`)
- Done: `match{}` in UI trees: `case label { ... }` children (`case _ { ... }` as the wildcard arm), matched by comparing the scrutinee's display text against each label -- see `tint-runtime/tests/render_ui.rs`
- Done: Compile-time `.tn` imports in the Vite sandbox source pipeline
- Namespace access (`Ns::item`) and lambda expressions are unimplemented specifically in the IR VM path (both already work through the tree-walking path used for UI handlers)
- Tuple-style enum variants (`enum E { A(T) }`) -- rejected at the semantic-check stage; only named-field variants (`enum E { A { x } }`) are supported
- Full type-system coverage (lambda parameters, `Option`/`Result` holes, generic functions, and some host-specific UI contracts are still incomplete)
- Generic functions and advanced generic constraints
- Async/await

These are deliberate design choices for a proof of concept, not bugs. The architecture supports adding them without major changes.

