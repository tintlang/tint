# Tint

A UI programming language that compiles to WebAssembly and runs in the browser.

```tint
ui fn App() {
    Column { padding::24 gap::12 background::#12141a
        Text { text::{24, bold, white} "Hello Tint" }
        Button { radius::8 color::white "Click me" }
    }
}
```

## Features

- **Compiler pipeline**: Lexer → Parser → AST → Semantic Checker → SSA IR → Optimizer → WASM
- **UI as first-class citizen**: No HTML/CSS/JS split—UI is the language
- **Modifiers system**: `padding::`, `margin::`, `color::`, `background::`, `radius::`, `border::`, `gap::`, `size::`, `opacity::`, `text::`
- **Control flow**: `if/else`, `for` loops, functions with persistence
- **Interactive sandbox**: Edit, compile, and preview in the browser (Svelte 5 + CodeMirror 6 + xterm.js)
- **CLI tools**: Check syntax, run, build to HTML
- **VSCode extension**: Syntax highlighting + build commands

## Quick Start

### CLI

```bash
# Install
cd tintlang/tint-model
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
npm run dev
```

Open browser to `http://localhost:5173` and edit code live.

## Project Structure

```
tintlang/
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
│   ├── sandbox/             # Web IDE (Svelte 5)
│   └── tests/               # Integration tests
└── vscode-tint/             # VSCode extension (TypeScript)
```

## What Works

✅ Proper compiler architecture (not hacked)
✅ UI grammar with modifiers
✅ Control flow (if/else, for loops)
✅ Function definitions & calls
✅ Persistent REPL sessions
✅ WASM compilation & rendering
✅ CLI tooling
✅ VSCode integration
✅ Hover effects & animations (demo)
✅ Dropdown menus via CSS (demo)

## Known Gaps

❌ Reactive state system (framework layer, post-launch)
❌ Click handlers that execute (parsed but not lowered)
❌ Cross-file imports
❌ Type annotations (inference-only)
❌ Generics
❌ Async/await

These are deliberate design choices for a proof of concept, not bugs. The architecture supports adding them without major changes.

## Example

Create `app.tn`:

```tint
ui fn App() {
    Column { padding::24 gap::16 background::#f5f7fa
        Text { text::{32, bold, #000} "Dashboard" }
        
        Card { radius::12 padding::16 background::white
            Row { gap::8
                Text { text::{14, #666} "Status:" }
                Text { text::{14, bold, #00aa00} "Online" }
            }
        }
        
        for { item in ["Item 1", "Item 2", "Item 3"] }
            ListItem { padding::12 radius::8 background::#eee
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

Mark Bender ([@BenderMare1316](https://x.com/BenderMare1316))

Feedback welcome—especially on syntax design, compiler architecture, or whether you'd use this for something real.
