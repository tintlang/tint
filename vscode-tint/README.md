# Tint Language for Visual Studio Code

Syntax highlighting and development tools for [Tint](https://github.com/tintlang/tint),
a statically typed language for declarative UI and general logic.

## Features

- Syntax highlighting for `.tn` files
- Syntax checking through the Tint CLI
- Build Tint UI files to standalone HTML with embedded WASM
- Preview generated HTML in a VS Code webview
- Experimental hover information, diagnostics, and go-to-definition
- Tint file and icon theme support

## Installation

Install **Tint Language** from the
[Visual Studio Code Marketplace](https://marketplace.visualstudio.com/items?itemName=tintlang.tint-lang).

The extension provides syntax highlighting immediately. To use build and
syntax-check commands, install the Tint CLI from the
[GitHub Releases page](https://github.com/tintlang/tint/releases). On macOS or
Linux, the latest release can be installed with:

```bash
curl -fsSL https://raw.githubusercontent.com/tintlang/tint/main/scripts/install-tint.sh | bash
export PATH="$HOME/.local/bin:$PATH"
```

If Rust is already installed, install the CLI directly from the repository:

```bash
cargo install --git https://github.com/tintlang/tint.git --package tint-cli --bin tint
```

## Commands

Open a `.tn` file, then use the Command Palette (`Cmd+Shift+P` /
`Ctrl+Shift+P`):

- **Tint: Build to HTML** — compile the current file to standalone HTML.
- **Tint: Check syntax** — validate the current file without running it.
- **Tint: Show preview** — open the most recently built HTML in a webview.

On macOS, **Tint: Build to HTML** is also available with `Cmd+Shift+B`.

## Example

Create `app.tn`:

```tint
ui fn App() {
    Column {
        layout::{ direction::column, padding::24, gap::16 }
        Text { text::{18, bold} "Hello Tint!" }
        Button { click||do_thing "Click me" }
    }
}
```

Run **Tint: Build to HTML**, then use **Tint: Show preview**.

## Requirements

- VS Code 1.80 or newer
- Tint CLI in `PATH` for build and syntax-check commands
- The optional `tint-analyzer` binary for experimental hover, diagnostics,
  and go-to-definition support

The analyzer integration is experimental and currently requires a locally
built or installed analyzer binary. Syntax highlighting and the CLI commands
work without it.

## Manual development installation

```bash
git clone https://github.com/tintlang/tint.git
cd tint/vscode-tint
npm install
npm run compile
npx @vscode/vsce package
code --install-extension tint-lang-0.1.2.vsix
```

## Support

Report bugs and request features in
[GitHub Issues](https://github.com/tintlang/tint/issues).
