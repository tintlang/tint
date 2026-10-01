# Changelog

All notable changes to Tint are documented here.

## [Unreleased]

- JS and CSS from Tint: `app { js::"./utils.js" css::"./theme.css" }`. Every function a JS module exports is callable from `.tn` as `name(...)` (sync; numbers, strings, bools, lists, maps); stylesheets are loaded and nodes take `class||"a b"`. Works in `tint build`/`tint dev` (files are embedded) and in `mount()` (`natives` and `baseUrl` options, `import()` loading). `DomSession`/`from_bytecode` take an optional `natives` object; new `app_assets(source)`. See `docs/guide/escape-hatches.md`.
- Host components: `Host { component||"Chart" props||{ ChartProps { ... } } }` mounts a JS component into the node. `mount(..., { components })` runs mount/update/destroy (props compared as canonical JSON; the element survives re-renders); `reactComponent(C)` in `@tintlang/react`. Not available in `tint build` pages.
- Rust interop (`docs/guide/rust-interop.md`): new `tint` crate (`Tint::new(..).call::<R>(name, args)`, `FromTint`/`IntoTint` + derives, `tint_file!`, `UiSession` access) and `#[tint::export]` to call Rust from `.tn`. `app { rs::"./native.rs" }` makes `tint run` build and use a runner with those functions linked in. `tint-cli` is now also a library (`tint_cli::main_with`).
- `rs::` in the browser: `tint build`/`tint dev` compile the Rust files to wasm with `wasm-pack` (size-optimised, gzipped, ~46 KB for the example) and register the `#[tint::export]` functions as natives; the `tint` crate has a `host` feature (default) that can be turned off to link no interpreter. Structs, `Option` and `Result` cross to JS as tagged objects and come back as the same Tint values.
- Callbacks and async (`docs/guide/async.md`): lambdas and `fn`s can be passed to JS and Rust functions (`tint::Callback` in Rust) and run against the live app after the call returned, re-rendering it. A JS function returning a Promise, or a Rust `async fn` (`#[tint::export]`, returns `Result<T, E: Display>`), takes a trailing callback that gets `Result::Ok/Err`. In `async fn`, `await f(a)` / `let r = await f(a)` is sugar for that callback (top level of the body only). Lambdas now see and assign `state`/root variables live. New `UiSession::call_value`, `tint_runtime::vm::set_deferred_runner`, `Value::Callback`. Example: `examples/async`.
- Styling and motion (`docs/ui/modifiers.md`, `docs/ui/animations.md`): states `focus`, `focus-visible`, `focus-within`, `active`/`tap`, `disabled`, `checked` and the pseudo-elements `placeholder`, `before`, `after`, `selection` (written as rules in one shared stylesheet); HTML attributes `placeholder||`, `disabled||{expr}`, `readonly||`, `title||`, `alt||`, `tabindex||`; `tap||handler`. New properties: `object-fit`, `object-position`, `clip-path`, `mix-blend-mode`, `text-overflow`, `line-clamp`, `flex-basis`, `grid-area/column/row`, `scroll-snap-*`, `perspective`, `order`, `ring`, and more; `rotate`, `skew`, `translate.x/y` combine with `scale` into one `transform`. `spring::{ stiffness, damping, mass, props }` is a spring transition (simulated once, sent to the browser as CSS `linear()`), `drag::{ both, back }` drags a node. `keyframes.name::{...}` in `app {}` is now documented.
- Fluid text: `text::{16..44, bold}` / `size::16..44` is a font size that grows with the screen (16px at 360px wide, 44px at 1280px, `clamp` + `vw`); `size::"clamp(1rem, 3vw, 2rem)"` and other CSS math functions pass through. `a..b` is now a valid modifier value. The landing page uses it.
- A failing function call no longer silently falls back to a variable lookup (it is reported); the checker accepts the host functions a session is given.
- A `UiSession` no longer auto-mounts `App`/`Main` before its `state` exists (`TintVM::load_program`).

## [0.1.3] - 2026-09-30

- The Tint site and sandbox are now one `.tn` program (`sandbox/src/site.tn`) with no hand-written HTML, JS or CSS: `index.html`, `site.js`, the CodeMirror islands, `lib/*.js`, Vite config and the CSS files are gone. `tint dev` / `tint build` generate the page shell; `npm run wasm` rebuilds the runtime embedded in the CLI.
- Routing: `app { route.Ui::"/path" }` (or `route.Ui::{ path::, title:: }`) maps URLs to `ui fn`s in one program, with `router::on`, per-route titles and navigation without reload. Theme state survives page switches.
- `app { }` now also takes `page::{ ... }` (body style), `keyframes.name::{ from::, pN::, to:: }` and `font.Family::{ src::, weight:: }`; the CLI emits the matching CSS (`@keyframes`, `@font-face`). A page that declares `page::` owns the whole page box (no default body padding).
- New `TextArea` node with `input||handler` (receives the new text) and `submit||handler` (Ctrl/Cmd+Enter). Tab inserts two spaces; the DOM value is only written when it differs, so the caret stays put.
- New `Preview` node: `Preview { entry||"App" "{source}" }` mounts a nested Tint session from source text, re-renders on edit and shows compile errors in place. Nested sessions ignore `key_down||`.
- New pointer events: `pointer_start||`, `pointer_move||`, `pointer_up||` (x, y), also tracked outside the element while dragging.
- New text natives backed by the real lexer: `tint_highlight(src)` (segments with classes for keyword, atom, string, number, type, fn, punct, op, opkw, comment), `line_count`, `max_line_len`, `line_numbers`. The sandbox editor is a transparent `TextArea` over a highlighted layer, written in Tint.
- Persistence: `storage_get_or(key, default)` next to `storage_get/set/remove`; in the browser these use `localStorage` (prefix `tint:`) and are loaded before `state` initialises. Theme and sandbox code survive reloads.
- `include_str("relative/path")` embeds a file as a string literal at build time. `\{` and `\}` write literal braces in strings.
- More keyword style properties: `caret-color`, `letter-spacing`, `word-break`, `font-style`, `color-scheme`, `transform-origin`, `line-height`, `tab-size`.
- Docs: `docs/ui/text-and-preview.md`, updated `routing.md`, `modifiers.md` and the README (you no longer write HTML/CSS/JS by hand).

### Changed

- Pac-Man is hidden from the site routes until it is finished.
- CI builds the site with `tint check` / `tint build` instead of npm.

## [0.1.2] - 2026-09-30

- UI rendering is much cheaper per frame. The browser renderer keeps the previous render tree and diffs in Rust, touching the DOM only where something changed. The builder reuses a pure subtree (no calls, blocks, lambdas or `if`/`match` expressions) when none of the outer variables it read has changed, and shares it as an `Rc` so the renderer can skip it by pointer. `UiRenderNode.children` is now `Vec<Rc<UiRenderNode>>`; `UiSession::render`/`dispatch` return `Vec<Rc<UiRenderNode>>` (`UiSession::set_reuse(false)` turns reuse off). Pong frame time went from 11.5 ms to 1.4 ms in headless Chromium.
- The tree-walking evaluator is the single execution engine on the runtime path. The SSA IR moved behind the optional `ir` cargo feature of `tint-runtime` (off by default).
- Added an engine-agnostic conformance suite (`tint-model/conformance/<category>/*.tn`, run by `tint-runtime/tests/conformance.rs` on the tree-walker and on the typed IR); known language gaps live in `conformance/errors/` and are recorded as expected failures.
- Added methods on lists, strings, and maps (see `docs/guide/collections.md`) and `tick||`/`every||` timers.
- Checker typing is complete on the conformance suite and the examples: no expression is left `Unknown` (census 32% -> 0%; `cargo test -p tint-semantics --test type_census -- --nocapture --ignored`). Return types of functions/methods without `-> T` are inferred, unannotated parameters take their type from call sites (when all agree), lambda parameters are inferred from the call they are passed to, `Option`/`Result` holes such as `None {}` are resolved from context, `?` unifies the error type with the enclosing function's, `frame||` handlers get `dt: number` and `key_down||`/`key_up||` get `key: string`, `&mut self` is recognised as `self`, and `state` initialisers give handlers typed state. `tint check --strict` additionally reports every expression whose type cannot be inferred (`CannotInfer`).
- Functions and UI functions are stored as `Rc`, so calling a function or rendering a `ui fn` no longer deep-clones its AST.
- Pong: the field is four wall rectangles instead of 288 cells (818 -> 53 DOM nodes), and the overlays are no longer rendered 12 times each (a `for{y in rows}` modifier was repeating the whole field).
- Added native terminal I/O: `print`, `println`, `read_line`, `parse_number`, and Unicode-aware `read_key`.
- Added `while`, `loop`, `break`, and `continue` for terminal and logic programs.
- Added typed numeric values: `i32`, `i64`, `u8`, `u32`, `u64`, `f32`, and `f64`, with explicit `as` conversions and range checks.
- Added typed `const` declarations.
- Added `Option<T>`/`Result<T, E>` flow with `expect`, `unwrap`, postfix `?`, `is_some`, `is_none`, `is_ok`, `is_err`, `unwrap_or`, `map`, and `and_then`.
- The checker knows the full `Option`/`Result` method set: `unwrap`, `expect`, `is_some`/`is_none`/`is_ok`/`is_err`, `is_some_and`/`is_ok_and`/`is_err_and`, `unwrap_or`, `unwrap_or_else`, `map`, `map_or`, `and_then`, `filter`, `or`, `or_else`, `ok_or`, `ok_or_else`, `map_err`, `ok`, `err`, `unwrap_err`, `expect_err`. A method of the other type (`is_some` on a `Result`) and a bare `Option`/`Result` annotation without type arguments are errors. The tree-walker still implements only the original nine (up to `and_then`); the rest run on the typed IR only.
- Added a typed IR (`tint-ir::typed`): interned types, monomorphised enums, a register CFG with a verifier, lowering from the checked AST (closures, `inout` methods, places, `?`) and a reference interpreter. It covers the whole conformance suite; generic functions, default parameters, named arguments, `ui fn` and kernels are not lowered yet. The older dynamic IR is unchanged.
- Added strict generic user-defined `struct` and `enum` types with checked type arguments and payloads.

### Fixed

- `%` now works in the evaluator and in IR constant folding.
- `+` with a string on either side concatenates in the evaluator (previously only the IR VM did).
- Nested string literals inside interpolations (`"{xs.join(" ")}"`) no longer break the lexer and parser, in both plain strings and UI text.
- String escapes `\n`, `\t`, `\r`, `\0` are turned into the real characters.
- Unary operators apply to the whole postfix chain: `-x.y()` and `!xs.is_empty()` work, and `-1 as u32` is `(-1) as u32`.
- `every||180` accepts a bare number as an attribute value.
- `&mut self` is recognised as `self` in the checker.

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
