# UI control flow

Tint UI control flow is written directly inside named nodes. There is no separate
wrapper element for control flow.

## Conditions

```tn
Panel {
    if{is_visible}
    Text { "Visible" }
}
```

## Loops

```tn
Column {
    for{item in items}
    Text { "{item.name}" }
}
```

The iterable is evaluated by the UI runtime and the child tree is built once
per item.

## Responsive branches

For layout changes based on viewport width, use the host-provided
`viewport_width` value:

```tn
Nav {
    if{viewport_width >= 768}
    Text { "Wide navigation" }
}
```

Style-only responsive changes use `mobile::{...}`, `tablet::{...}`,
`laptop::{...}`, and `desktop::{...}`. `match{...}` is parsed but is not yet
evaluated in the UI tree.
