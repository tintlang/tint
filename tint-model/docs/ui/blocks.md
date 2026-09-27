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
per item. This modifier form repeats the node it's written on (here,
`Column`'s own children) -- it can only repeat a single node's worth of
children, and needs a wrapper node to group more than one.

A standalone block form repeats a whole run of sibling nodes per iteration,
spliced directly into the parent -- no wrapper node needed, so it can sit
between static siblings:

```tn
Column {
    Text { "Header" }
    for { item in items } {
        Text { "{item.name}" }
        Divider {}
    }
    Text { "Footer" }
}
```

Unlike the modifier form, this one requires explicit `{...}` braces around
its body.

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
