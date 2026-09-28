# Changelog

All notable changes to Tint are documented here.

## [0.1.1] - 2026-09-28

- Unified the project version across the Rust crates, VS Code extension, sandbox, and generated WASM package metadata.
- Added `VERSION` as the single source of truth and a synchronization script for release metadata.
- Added end-to-end lambda execution with lexical capture through the tree-walking evaluator, with a documented IR fallback boundary.
- Added typed semantic checking for functions, UI functions, and `impl` methods, including inferred primitive, collection, struct, enum, function, and lambda types.
- Added checks for operators, conditions, assignments, returns, function and method arguments, struct fields, enum constructors and patterns, UI state, interpolation, and UI control flow.
- Added regression coverage for lambdas, semantic type checking, and enum-typed fields.
- Added UI tokens, reusable styles, components, slots, and variants with theme-aware resolution.
