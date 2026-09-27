# Design sketches (not compiled)

The `.rs` files under this directory are **not part of the build** -- they used
to live in `tint-runtime/src/{async_rt,borrow,resources,state}` as
`#[allow(dead_code)]` modules, but nothing outside their own files ever
referenced them, so they were moved here on 2026-09-27 to stop carrying
never-called code in the actual crate.

They're kept as design placeholders/notes for future work that hasn't been
built yet:

- `async_rt/` -- an async runtime/scheduler sketch.
- `borrow/` -- a borrow-checking-style locking sketch (intrinsics + locks).
- `resources/` -- a resource table sketch (typed handle table).
- `state/` -- a reactive state sketch (signals, computed values, stores,
  watchers).

If one of these is ever picked back up for real, move it back into
`tint-runtime/src/` (or wherever it ends up belonging), wire it into
`lib.rs`, and delete the `#![allow(dead_code)]` once it's actually used.
