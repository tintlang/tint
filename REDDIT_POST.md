# Rune — A UI Programming Language (Proof of Concept)

I built a programming language focused on UI development. It compiles to WebAssembly and runs in the browser.

## The Idea

Instead of HTML/CSS/JS split, Rune is a single language where UI is a first-class concept. You write:

```rune
ui fn App() {
    Column { padding::24 gap::12 background::#12141a
        Text { text::{24, bold, white} "Hello" }
        Button { radius::8 color::white "Click me" }
    }
}
```

And it compiles to WASM that renders to DOM. Modifiers (`padding::`, `gap::`, `color::`) replace CSS.

## What Works

- **Proper compiler pipeline**: Lexer → Parser → AST → Semantic Checker → SSA IR → Optimizer → WASM
- **UI grammar** with XML-style and block-mode syntax
- **Modifiers system**: `padding`, `margin`, `radius`, `background`, `color`, `text`, `border`, `gap`, `size`, `opacity`
- **Control flow**: `if/else`, `for` loops rendered as DOM siblings
- **Persistent REPL** (in sandbox): functions persist between entries
- **Interactive sandbox** (Svelte 5 + CodeMirror 6 + xterm.js) — edit, compile, render in the browser
- **CLI tooling**: `rune check`, `rune run`, `rune build` (generates standalone HTML)
- **VSCode extension**: Syntax highlighting + build commands + preview

## What Doesn't Yet

- Reactive state (`$state`, `$derived` — framework layer, not language)
- Click handlers that execute (parsed, not lowered to IR)
- Cross-file imports
- Async/await or streams

These are all deliberate gaps for a proof of concept, not architecture problems.

## Try It

### Run Locally

```bash
cd runelang/rune-model
cargo build -p rune-cli --release
./target/release/rune check app.rn
./target/release/rune build app.rn -o app.html
```

### Use VSCode

1. Install the extension from `vscode-rune/` (or npm publish later)
2. Open a `.rn` file
3. Cmd+Shift+B to build

### Web Sandbox

Open `sandbox/` in a browser (requires building Svelte first):

```bash
cd sandbox
npm install
npm run dev
```

## Why This Matters

Writing UI in a dedicated language lets you:
- Eliminate boilerplate (no JS/CSS glue)
- Type-check the whole pipeline (frontend + backend DSL can share the same language)
- Optimize rendering differently than imperative code

The compiler is properly structured, not hacked. Every phase is testable; the IR is serializable; you could theoretically emit to other targets (Flutter, React, etc.) without rewriting the whole frontend.

## The Code

All Rust. ~8000 LoC across:
- Lexer, Parser, AST, Semantic Analyzer
- SSA IR + Optimizer
- WASM runtime bridge
- CLI + VSCode extension

No external DSL files — the grammar is in the code, easy to modify.

## Next Steps

Post-launch polish:
1. Hover effects & animations (✓ done in sandbox demo)
2. Dropdown menus (✓ done with pure CSS)
3. State system (framework layer, probably Svelte-like signals)
4. Click handlers that actually work
5. Better error messages with snippet spans

## GitHub

[marekbender/runelang](https://github.com/marekbender/runelang)

Feedback welcome—especially on syntax, compiler design, or whether you'd actually want to use this for something.
