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
npm --prefix sandbox run build
git diff --check
```

To run the browser sandbox with automatic Rust/WASM rebuilds and Vite HMR:

```bash
cd tint-model/sandbox
npm install
npm run dev:all
```

The landing page is `/`; the workbench is `/sandbox` or
`/app.html?route=/sandbox`.

When Rust/WASM runtime code changes, regenerate the checked-in browser runtime
artifacts:

```bash
npm --prefix tint-model/packages/tint-runtime run assemble
```

This is a maintainer/release command. Application authors using
`@tintlang/runtime` or `@tintlang/vite` do not run `wasm-pack` or assemble the
runtime themselves. The user-facing Vite setup is documented in
`tint-model/docs/guide/vite.md`.
