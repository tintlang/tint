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
