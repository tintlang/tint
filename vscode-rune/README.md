# Rune Language for VSCode

Syntax highlighting and development tools for the [Rune](https://github.com/hawerz/runelang) UI programming language.

## Features

- **Syntax highlighting** for `.rn` files
- **Build to HTML** - compile your Rune UI code to standalone HTML with embedded WASM
- **Syntax checking** - validate your code without running it
- **Live preview** - preview compiled UI directly in VSCode

## Installation

### From CLI

If you have the Rune CLI installed:

```bash
cargo install --path /path/to/runelang/rune-model/crates/rune-cli
```

Then install this extension from VSCode marketplace (or manually).

### Manual Installation

1. Clone the runelang repository
2. Navigate to `vscode-rune` directory
3. Run `npm install` && `npm run compile`
4. Run `code --install-extension rune-lang-0.1.0.vsix`

## Commands

- **Rune: Build to HTML** (Cmd+Shift+B) - Compile current file to HTML
- **Rune: Check syntax** - Validate syntax without building
- **Rune: Show preview** - Display the built HTML in a webview

## Example

Create a file `app.rn`:

```rune
ui fn App() {
    Column { padding::24 gap::16
        Text { text::{18, bold} "Hello Rune!" }
        Button { click||do_thing "Click me" }
        Card { radius::12 background::#f0f0f0
            Text { "Nested content" }
        }
    }
}
```

Press Cmd+Shift+B to build it into an HTML file.

## Requirements

- Rune CLI (`rune` command available in PATH)
- VSCode 1.80+
