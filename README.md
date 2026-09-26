# Tint

A UI programming language that compiles to WebAssembly and runs in the browser.

```tint
ui fn App() {
    Column {
        layout::{ padding::24, gap::12 }
        paint::{ background::#12141a }
        Text { text::{24, bold, white} "Hello Tint" }
        Button { paint::{ radius::8, color::white } "Click me" }
    }
}
```

## Features

- **Compiler pipeline**: Lexer → Parser → AST → Semantic Checker → SSA IR → Optimizer → WASM
- **UI as first-class citizen**: No HTML/CSS/JS split—UI is the language
- **Semantic modifier groups**: `layout::{...}`, `paint::{...}`, and `motion::{...}`; flat modifiers remain accepted for compatibility
- **Control flow**: `if/else`, `for` loops, functions with persistence, full comparison operators (`< > <= >= == !=`) alongside `&& ||`
- **Real state + events**: `state`, `click||`, `hover_in||`/`hover_out||` drive a persistent, re-rendering session
- **Responsive design**: `mobile::{}`/`tablet::{}`/`laptop::{}`/`desktop::{}` style breakpoints, plus a host-exposed `viewport_width` variable for structural `if{}` layout swaps (see `tint-model/sandbox/src/landing/`)
- **Layout primitives**: flex direction and a typed `grid::{ columns, rows, gap }` modifier with `minmax(0, 1fr)`-friendly tracks
- **Internal routing**: `route||"/path"` renders as a normal browser link, including `/` and `/sandbox`
- **Direct DOM rendering**: `DomSession` renders Tint through Rust/`web-sys`, without a UI framework
- **Interactive sandbox**: Edit, compile, and preview in the browser with Tint, CodeMirror 6, and WASM (the sandbox UI itself is currently mid-rework and will land as a deliberately simple workbench, not a full IDE -- see `sandbox/README.md` for status)
- **CLI tools**: Check syntax, run, build to HTML
- **Native Rust interop**: `TintVM::register_native("name", |args| ...)` registers a real Rust closure that `.tn` source calls directly by name -- no Rust syntax inside the language, no reimplementing rustc's borrow checker, just an ordinary Rust function called across the boundary (see `tint run <file> now_ms`, a demo native in `tint-cli`)
- **VSCode extension**: Syntax highlighting + build commands

## Quick Start

### CLI

```bash
# Install
cd tint/tint-model
cargo build -p tint-cli --release
export PATH="$PWD/target/release:$PATH"

# Check syntax
tint check app.tn

# Build to HTML
tint build app.tn -o app.html

# Run in REPL
tint repl
```

### VSCode Extension

1. Install from marketplace (coming soon) or manually:
   ```bash
   cd vscode-tint
   npm install && npm run compile
   code --install-extension ./tint-lang-0.1.0.vsix
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

> **Status:** the sandbox UI is currently being reworked -- migrating from a
> single `sandbox.tn` file to a `src/sandbox/` directory of Tint files
> (`workbench.tn`, `actions.tn`, `filesystem.tn`, `editor-pane.tn`,
> `index.tn`). The sandbox will keep existing, just deliberately simple --
> not a full IDE. It is not confirmed working end-to-end yet; see
> `sandbox/README.md` for the current state before relying on it.

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
│   ├── sandbox/             # Tint-rendered Web IDE
│   └── tests/               # Integration tests
└── vscode-tint/             # VSCode extension (TypeScript)
```

## What Works

✅ Proper compiler architecture (not hacked)
✅ UI grammar with modifiers
✅ Control flow (if/else, for loops, comparison operators)
✅ Function definitions & calls
✅ Persistent REPL sessions
✅ WASM compilation & rendering
✅ Real state + click/hover handlers (`state`, `click||`, `hover_in||`/`hover_out||`) via a persistent `UiSession`
✅ Responsive layouts (`mobile::{}`/`tablet::{}`/`laptop::{}`/`desktop::{}` breakpoints + structural `if{ viewport_width ... }`)
✅ A second, direct-DOM rendering backend (`DomSession`, no JS framework)
✅ CLI tooling
✅ VSCode integration
✅ Hover effects & animations
✅ Grid layout and internal route links
✅ Automatic `.tn` reload and Rust/WASM rebuild with `npm run dev:all`

## Known Gaps

❌ Multiple independent component instances (state is one flat scope per `UiSession`)
❌ `match{}` in UI trees (parsed, not evaluated)
✅ Compile-time `.tn` imports in the Vite sandbox source pipeline
❌ A native fn (or any function) called from *inside* a plain `fn`'s own body, when that `fn` runs the normal top-level way -- the IR VM's `Call` instruction is still a stub (see `ir_vm.rs`). Native fns ARE reachable from click/hover handlers, UI fn bodies, and any other tree-walked call site today (see `tint-runtime/tests/native_fn.rs`)
❌ Type annotations (inference-only)
❌ Generics
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
        
        for { item in ["Item 1", "Item 2", "Item 3"] }
            ListItem { layout::{ padding::12 } paint::{ radius::8, background::#eee }
                Text { "{item}" }
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

Unlicense (public domain) - do whatever you want with it.

## Author

Mark Bender ([@hawerz](https://github.com/hawerz))

Feedback welcome—especially on syntax design, compiler architecture, or whether you'd use this for something real.
