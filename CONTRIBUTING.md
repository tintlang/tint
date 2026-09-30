# Contributing

## Updating the project version

The canonical project version is stored in `VERSION`. After changing it, synchronize the Rust, npm, VS Code extension, sandbox, and generated WASM package metadata:

```bash
node scripts/sync-version.mjs
```

Before committing a release-version change, verify the workspace with:

```bash
cargo check --manifest-path tint-model/Cargo.toml --workspace --locked
```

### Publishing a CLI release

GitHub Actions builds and publishes CLI archives for Linux x86_64, macOS
x86_64, macOS Apple Silicon, and Windows x86_64 when a matching version tag is
pushed. The tag must use the `v` prefix and match `VERSION` exactly.

```bash
# VERSION is the single source of truth.
node scripts/sync-version.mjs
git diff --check
git tag "v$(tr -d '[:space:]' < VERSION)"
git push origin "v$(tr -d '[:space:]' < VERSION)"
```

The release workflow waits for the full CI suite, builds the four platform
archives, creates SHA-256 checksum files, and creates a GitHub Release with
generated release notes. It does not publish npm packages or the VS Code
extension yet.

The Windows installer has not been tested on a real Windows system yet; only
the PowerShell source and the release workflow structure have been checked.

## Tint model development

To run the current CLI without accidentally using an older globally installed
binary, add the repository wrapper to `PATH`:

```bash
export PATH="$PWD/scripts:$PATH"
tint run main.tn
```

The wrapper delegates to `cargo run -p tint-cli`, so Cargo rebuilds the CLI
when its source or runtime dependencies change. Release users receive a
prebuilt binary and never run this wrapper.

The compiler, browser runtime, Vite plugin, and sandbox live under
`tint-model/`. After changing the parser or runtime, run the focused checks:

```bash
cd tint-model
cargo fmt --all -- --check
cargo test -p tint-runtime -p tint-wasm
cargo run -q -p tint-cli -- build sandbox/src/site.tn -o /tmp/site.html
git diff --check
```

To run the site and sandbox (one `.tn` program, served by the `tint` CLI):

```bash
cd tint-model/sandbox
npm run dev       # tint dev src/site.tn
npm run wasm      # after Rust changes: rebuild the runtime the CLI embeds
```

The landing page is `/`; the workbench is `/sandbox`.

When Rust/WASM runtime code changes, regenerate the checked-in browser runtime
artifacts:

```bash
npm --prefix tint-model/packages/tint-runtime run assemble
```

This is a maintainer/release command. Application authors using
`@tintlang/runtime` or `@tintlang/vite` do not run `wasm-pack` or assemble the
runtime themselves. The user-facing Vite setup is documented in
`tint-model/docs/guide/vite.md`.

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


## Building from source

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
- **Evaluator** → executes the AST (the only engine on the default runtime path)
- **IR** → SSA form, optimization passes; experimental, behind the `ir` feature of `tint-runtime`

No external DSL files—grammar is in code for easy modification.

