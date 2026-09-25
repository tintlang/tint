# Tint Language for VSCode

Syntax highlighting and development tools for the [Tint](https://github.com/hawerz/tintlang) UI programming language.

## Features

- **Syntax highlighting** for `.tn` files
- **Build to HTML** - compile your Tint UI code to standalone HTML with embedded WASM
- **Syntax checking** - validate your code without running it
- **Live preview** - preview compiled UI directly in VSCode

## Installation

### From CLI

If you have the Tint CLI installed:

```bash
cargo install --path /path/to/tintlang/tint-model/crates/tint-cli
```

Then install this extension from VSCode marketplace (or manually).

### Manual Installation

1. Clone the tintlang repository
2. Navigate to `vscode-tint` directory
3. Run `npm install` && `npm run compile`
4. Run `code --install-extension tint-lang-0.1.0.vsix`

## Commands

- **Tint: Build to HTML** (Cmd+Shift+B) - Compile current file to HTML
- **Tint: Check syntax** - Validate syntax without building
- **Tint: Show preview** - Display the built HTML in a webview

## Example

Create a file `app.tn`:

```tint
ui fn App() {
    Column { padding::24 gap::16
        Text { text::{18, bold} "Hello Tint!" }
        Button { click||do_thing "Click me" }
        Card { radius::12 background::#f0f0f0
            Text { "Nested content" }
        }
    }
}
```

Press Cmd+Shift+B to build it into an HTML file.

## Requirements

- Tint CLI (`tint` command available in PATH)
- VSCode 1.80+
