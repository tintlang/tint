# Tint UI syntax

Tint UI is a declarative tree. A rendered node is a named block; text is a
quoted child.

```tn
ui fn App() {
    Page {
        layout::{ direction::column, gap::16, padding::24 }
        paint::{ background::#0a0a0a }

        Text { text::{24, bold, white} "Hello Tint" }
        Button { click||open_menu "Open" }
    }
}
```

Indentation is optional. Braces define the tree and commas separate values
inside a modifier tuple.

`Column` and `Row` are built-in layout containers: `Column` defaults to a
vertical flex layout and `Row` to a horizontal flex layout. Therefore
`gap::16` works on them without an explicit `direction::...`; use
`direction::...` when a different axis is needed.

## Nodes and modifiers

```tn
Card {
    layout::{ direction::column, padding::24 }
    paint::{
        radius::20,
        gradient::{ angle::135, from::#6c5ce7, to::#8b7cf0 }
    }
    motion::{ transition::"transform .2s ease", hover::{ scale::1.03 } }

    Text { text::{18, bold, white} "Balance" }
}
```

`layout`, `paint`, and `motion` are semantic modifier groups. The flat form is
still parsed for compatibility, but the grouped form is canonical.

Grid layouts use the `grid` modifier. A numeric column count creates equal
tracks; a string can provide an explicit track list. `minmax(0, 1fr)` keeps a
long editor or preview from forcing the page wider:

```tn
Workspace {
    layout::{ grid::{ columns::"minmax(0, 1fr) minmax(0, 1fr)", gap::24 } }
    mobile::{ grid::{ columns::"1fr" } }
}
```

## State and events

State is declared inside a UI function. Event attributes use `||` followed by a
handler name:

```tn
ui fn Counter() {
    state count = 0

    Column {
        Text { "Count: {count}" }
        Button { click||increment "+" }
    }
}
```

Internal navigation uses the same attribute channel with `route||"/path"`.
The DOM renderer emits a normal `<a href="...">`, so links work with browser
history and can target paths such as `/` and `/sandbox`:

```tn
Logo { route||"/" "Tint" }
SandboxLink { route||"/sandbox" "Open sandbox" }
```

`route||` is for application paths. External URLs should remain ordinary
host/browser links rather than being treated as Tint routes.

## Themes and imports

```tn
import "./topbar.tn";

ui fn Landing() {
    state theme = "dark"

    theme::dark { Page { paint::{ background::#0a0a0a } } }
    theme::light { Page { paint::{ background::#ffffff } } }
}
```

Theme blocks are selected at runtime. `.tn` imports are currently expanded by
the sandbox loader before parsing.

## Conditions and responsive styles

```tn
Caption {
    mobile::{ paint::{ color::#9a9a9a } }
    if{viewport_width >= 640}
    "Visible on wide screens"
}
```

The supported responsive names are `mobile`, `tablet`, `laptop`, and `desktop`.
Ordinary modifiers (including responsive ones like `mobile::{...}`) must come
before any `if{}`/`for{}`/`match{}` control-flow modifier in the same node —
the parser reads a node's plain modifiers first, then its control-flow
modifiers, so `if{...}` followed by `mobile::{...}` does not parse.
