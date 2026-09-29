# Tint

Tint is a small declarative UI DSL for Rust/WASM applications, with a direct
Rust-to-DOM rendering path.

```tint
ui fn App() {
    theme::dark { tokens {
        page-bg::#12141a
        button-bg::#ffffff
        button-text::#12141a
    } }
    theme::light { tokens {
        page-bg::#ffffff
        button-bg::#12141a
        button-text::#ffffff
    } }

    style ButtonBase {
        layout::{ padding.x::18, padding.y::10 }
        paint::{ radius::full }
        motion::{ transition::"transform .2s ease", hover::{ scale::1.04 } }
    }

    component Button {
        use::ButtonBase
        variant::solid {
            paint::{ background::@button-bg, color::@button-text }
        }
        slot::content
    }

    Page {
        layout::{ direction::column, padding::24, gap::12 }
        paint::{ background::@page-bg }
        Button { variant::solid "Hello Tint" }
    }
}
```

This small example demonstrates theme tokens, reusable styles, components,
slots, and variants without duplicating the UI tree for each theme.

## Demo

- Live sandbox + landing page: https://tint-gamma.vercel.app/
- Pong, built entirely in Tint (`.tn`) source: https://tint-gamma.vercel.app/pong

## Features

- **Compiler pipeline**: Lexer → Parser → AST → Semantic Checker → SSA IR → Optimizer → WASM
- **UI as a first-class value**: Tint owns the UI tree and rendering semantics, while CSS and JavaScript remain available for styling and host/browser integration
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
- **Interactive sandbox**: Edit, compile, and preview in the browser with Tint, CodeMirror 6, and WASM
- **CLI tools**: Check syntax, run, and generate a prototype HTML shell
- **Terminal I/O**: `print`, `println`, `read_line`, `parse_number`, and
  `read_key` for native terminal programs
- **Result handling**: `Option<T>`, `Result<T, E>`, `Some`/`None`, `Ok`/`Err`,
  `expect`, `unwrap`, and postfix `?` error propagation
- **Numeric types**: real `i32`, `i64`, `u8`, `u32`, `u64`, `f32`, and `f64`
  values with explicit `as` conversions and range checks; `number` is `f64`
- **Logic loops**: `while`, `loop`, `break`, and `continue`
- **Native Rust interop**: `TintVM::register_native("name", |args| ...)` registers a real Rust closure that `.tn` source calls directly by name -- no Rust syntax inside the language, no reimplementing rustc's borrow checker, just an ordinary Rust function called across the boundary (see `tint run <file> now_ms`, a demo native in `tint-cli`)
- **Multi-file modules**: `mod name;` resolves to `name.tn` or `name/mod.tn` (the `mod.rs` convention); `use path::to::item;` + `export` control what crosses file boundaries (see `examples/modules/`)
- **VSCode extension**: Syntax highlighting + build commands

## Quick Start

### CLI

For repository development, use the wrapper so the CLI is rebuilt from the
current source automatically:

```bash
export PATH="$PWD/scripts:$PATH"
tint run main.tn
```

For an installed release, `tint` is a prebuilt binary; end users do not need
Cargo or a manual CLI rebuild.

```bash
# Build the CLI
cd tint-model
cargo build -p tint-cli --release
export PATH="$PWD/target/release:$PATH"

# Check syntax
tint check app.tn

# Run main.tn / fn main()
tint run

# Run a specific Tint file (fn main() is the default entry point)
tint run app.tn

# Build a standalone HTML file (real embedded WASM, works offline from disk)
tint build app.tn [ui_fn] -o app.html

# Run in REPL
tint repl
```

### VSCode Extension

1. Install from marketplace (coming soon) or manually:
   ```bash
   cd vscode-tint
   npm install && npm run compile
   code --install-extension ./tint-lang-0.1.1.vsix
   ```

2. Open a `.tn` file
3. Press `Cmd+Shift+B` to build to HTML

### Web Sandbox

```bash
cd tint-model/sandbox
npm install
npm run dev:all
```

Open browser to `http://localhost:5173` and edit code live.

For using Tint in an existing Vite application, see
[`tint-model/docs/guide/vite.md`](tint-model/docs/guide/vite.md). Runtime
assembly and contributor-only build commands are documented in
[`CONTRIBUTING.md`](CONTRIBUTING.md).

> **Status:** the sandbox is a deliberately small workbench, not a full IDE.
> It currently uses `sandbox/src/sandbox.tn`, CodeMirror, a live WASM preview,
> and direct DOM rendering through `DomSession`.

## Project Structure

```
tint/
├── tint-model/              # Core language implementation (Rust)
│   ├── crates/
│   │   ├── tint-lexer/      # Tokenization
│   │   ├── tint-parser/     # Parsing → AST
│   │   ├── tint-ast/        # AST nodes
│   │   ├── tint-semantics/  # Semantic analysis
│   │   ├── tint-ir/         # SSA IR + optimizer
│   │   ├── tint-evaluator/  # Value system + built-ins
│   │   ├── tint-runtime/    # VM + UI renderer
│   │   ├── tint-wasm/       # WASM bindings
│   │   └── tint-cli/        # Command-line tool
│   ├── examples/            # Sample .tn programs
│   └── sandbox/             # Tint-rendered Web IDE
└── vscode-tint/             # VSCode extension (TypeScript)
```

Tests live inside each crate's own `tests/` directory (standard Rust layout),
not in a separate top-level folder -- `tint-runtime/tests/` has the most,
covering the UI runtime, sessions, layout, and rendering.

## What Works

✅ Proper compiler architecture (not hacked)
✅ UI grammar with modifiers
✅ Control flow (if/else statements and expressions, for loops, comparison operators)
✅ Function definitions, typed function values & calls
✅ Persistent REPL sessions
✅ WASM compilation & rendering
✅ Real state + click/hover handlers (`state`, `click||`, `hover_in||`/`hover_out||`) via a persistent `UiSession`
✅ Responsive layouts (`mobile::{}`/`tablet::{}`/`laptop::{}`/`desktop::{}` breakpoints + structural `if{ viewport_width ... }`)
✅ A second, direct-DOM rendering backend (`DomSession`, no JS framework)
✅ CLI tooling
✅ Experimental `tint-analyzer` LSP with diagnostics, hover types, and go-to-definition for open documents
✅ VSCode integration
✅ Hover effects & animations
✅ Grid layout and internal route links
✅ Automatic `.tn` reload and Rust/WASM rebuild with `npm run dev:all`
✅ A native fn called from *inside* a plain `fn`'s own body, invoked the normal top-level way through the IR VM -- `IrVM` now falls back to a native-lookup callback (`set_native_call`) when no IR-compiled function matches the callee name, backed by `TintVM::native_fns` (see `tint-runtime/tests/native_fn.rs`)
✅ Multi-file modules (`mod`/`use`/`export`) via a `tint-cli`-side loader -- absolute paths only (no `self::`/`super::`), but grouped (`use a::{b, c}`), `as`-aliased, and wildcard (`use a::*`) imports all work alongside the plain form; `fn`/`struct`/`enum` are exportable
✅ An evolving semantic/type checker with inference over `fn`, `ui fn`, and `impl` method bodies -- `tint check` fails on errors, `tint run` warns and still executes. Built-in and user-defined generic structs/enums are checked strictly; some host-specific UI contracts remain unsupported.
✅ One UI syntax: the earlier XML dialect (`<Tag ...>...</Tag>`) has been removed -- `Tag { ... }` block syntax is now the only way to write UI, including `match{}`'s case arms (see below)
✅ `children { ... }` -- an optional grouping tag for a node's own children, so they don't blur together with several modifiers in the same block; splices straight into its parent, no wrapper node of its own (see `tint-runtime/tests/render_ui.rs`)

## Known Gaps

❌ Multiple independent component instances (state is one flat scope per `UiSession`)
✅ `match{}` in UI trees: `case label { ... }` children (`case _ { ... }` as the wildcard arm), matched by comparing the scrutinee's display text against each label -- see `tint-runtime/tests/render_ui.rs`
✅ Compile-time `.tn` imports in the Vite sandbox source pipeline
❌ Namespace access (`Ns::item`) and lambda expressions are unimplemented specifically in the IR VM path (both already work through the tree-walking path used for UI handlers)
❌ Tuple-style enum variants (`enum E { A(T) }`) -- rejected at the semantic-check stage; only named-field variants (`enum E { A { x } }`) are supported
❌ Full type-system coverage (some host-specific UI contracts and generic functions are still incomplete)
❌ Generic functions and advanced generic constraints
❌ Async/await

These are deliberate design choices for a proof of concept, not bugs. The architecture supports adding them without major changes.

## Example

Create `app.tn`:

```tint
ui fn App() {
    Column {
        layout::{ padding::24, gap::16 }
        paint::{ background::#f5f7fa }
        Text { text::{32, bold, #000} "Dashboard" }
        
        Card { layout::{ padding::16 } paint::{ radius::12, background::white }
            Row { gap::8
                Text { text::{14, #666} "Status:" }
                Text { text::{14, bold, #00aa00} "Online" }
            }
        }
        
        for { item in ["Item 1", "Item 2", "Item 3"] } {
            ListItem { layout::{ padding::12 } paint::{ radius::8, background::#eee }
                Text { "{item}" }
            }
        }
    }
}
```

Build & view:

```bash
tint build app.tn -o app.html
open app.html
```

### Responsive example

Style breakpoints (`mobile::{}`/`tablet::{}`/`laptop::{}`/`desktop::{}`) restyle a node per
viewport width, and a host-exposed `viewport_width` variable lets `if{}` swap in structurally
different content — not just a different style. See `tint-model/sandbox/src/landing/` for a full
page built this way (a nav that becomes a hamburger menu below 768px, a hero that stacks on
mobile), rendered through `DomSession` with no JS framework -- it's the site's own real landing
page (`sandbox/src/main.tn` mounted by a minimal `sandbox/index.html`), split into `landing.tn`, `topbar.tn`, `hero.tn`, `demo.tn`, and
`footer.tn`, with compile-time `.tn` imports and `theme::dark`/`theme::light`
blocks instead of duplicated comment-marked trees:

```tint
Nav {
    layout::{ direction::row }
    NavLinks { layout::{ direction::row, gap::24 } if{viewport_width >= 768} Text { "Docs" } }
    MenuButton { click||toggle_menu layout::{ padding::10 } paint::{ radius::10 } if{viewport_width < 768} "Menu" }
}

Hero {
    layout::{ direction::row, gap::40 }
    mobile::{ direction::column, gap::24 }
    HeroCopy { grow::1 Text { text::{44, bold, white} "..." } }
}
```

## Development

### Build Everything

```bash
cd tint-model

# Native (native CLI)
cargo build -p tint-cli

# WASM (web sandbox)
cargo build -p tint-wasm --target wasm32-unknown-unknown --release
wasm-pack build crates/tint-wasm --release

# Tests
cargo test
```

### Architecture

Each compiler phase is independent:
- **Lexer** → tokens (no state)
- **Parser** → AST (recursive descent)
- **Semantics** → validates scopes, types, call graph
- **IR** → SSA form, optimization passes
- **VM** → executes IR (native + WASM)

No external DSL files—grammar is in code for easy modification.

## Performance

- **Binary size**: ~500KB WASM (release, uncompressed)
- **Compile time**: ~100ms per file (native)
- **Runtime**: Direct DOM rendering (no virtual DOM)

## License

MIT - see [LICENSE](LICENSE) for details.

## Author

Mark Bender ([@hawerz](https://github.com/hawerz))

Feedback welcome!
