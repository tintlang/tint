# Storage

Tint has a small host-independent storage API:

```tn
storage_set("high_score", "120")
let score = storage_get("high_score")
storage_remove("high_score")
```

`storage_get` returns `Unit` when the key does not exist. The runtime keeps
values in memory. Browser hosts can call `storage_snapshot()` and
`hydrate_storage(...)` on `UiSession` or `DomSession` to connect this store to
`localStorage`, IndexedDB, or another persistence layer.

Storage values are strings by design; the compiler does not silently serialize
arbitrary Tint values.
