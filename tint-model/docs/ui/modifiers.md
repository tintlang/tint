# UI modifiers

Tint UI uses one modifier form:

```tn
name::value
name::{ value, value }
```

Whitespace is only a separator. It is never significant. Logic attributes use
`||`:

```tn
Button {
    click||open_settings
    layout::{ padding.x::18, padding.y::10 }
}
```

## Semantic groups

For new code, put related modifiers in one of three semantic groups. Groups are
a Tint language feature, not CSS blocks; the runtime flattens them into the same
style model.

```tn
Card {
    layout::{ direction::column, gap::12, padding::24 }
    paint::{
        gradient::{ angle::135, from::#6c5ce7, to::#8b7cf0 },
        radius::20,
        border::{1, #8b7cf0}
    }
    motion::{
        transition::"transform .2s ease",
        hover::{ scale::1.03, shadow::"0 16px 40px rgba(0,0,0,.35)" }
    }
}
```

The groups are:

- `layout` — direction, alignment, flex spacing, grid tracks, size, position, padding and margin;
- `paint` — colors, gradients, borders, radius, opacity, blur and shadows;
- `motion` — transitions and the `hover::{...}` state.

`font-family::"..."` is also resolved to CSS and can be placed directly on a
node; descendants inherit it unless they choose another family.


The group name does not change the value syntax. The flat form remains accepted
for compatibility, but new code and documentation should use semantic groups.

## Layout

`Column` and `Row` automatically use flex layout (`column` and `row`
respectively), so their children can use `gap` directly. Other node names are
ordinary elements: `gap` requires `direction::row`, `direction::column`, or a
`grid::{...}` layout on those nodes.

```tn
Panel {
    layout::{
        direction::row,
        align::center,
        justify::space-between,
        gap::12,
        padding.x::24,
        padding.y::16,
        margin.b::20
    }
}
```

Padding aliases are available as `padding.x`, `padding.y`, `padding.t`,
`padding.b`, `padding.l`, and `padding.r`.

The same axis/side aliases are available for margins and borders:

```tn
Card {
    layout::{ margin.x::16, margin.b::24 }
    paint::{ border.t::{1, #2b2b2b}, border.b::{1, #2b2b2b} }
}
```

Supported forms are `margin.x/y/t/b/l/r` and `border.x/y/t/b/l/r`. Full names
such as `border.top` and `margin.bottom` are accepted as well. Border values use
the same `{width, color}` tuple as `border::{...}`.

Grid uses a numeric column count or a CSS track list. `minmax(0, 1fr)` is the
recommended track form for editors and previews because long content stays
inside its column:

```tn
SourceRender {
    layout::{ grid::{ columns::"minmax(0, 2fr) minmax(0, 1fr)", gap::24 } }
    mobile::{ grid::{ columns::"1fr" } }
}
```

`min-width::0`, `min-height::0`, and `overflow::hidden` are available as raw
layout properties when a nested scroll container must not expand its parent.

Responsive values use a nested modifier block:

```tn
Hero {
    layout::{ direction::column, padding.y::100, gap::22 }
    mobile::{ padding.y::40, gap::18 }
}
```

## Paint

```tn
Card {
    paint::{
        background::#12141a,
        border::{1, #2b2b2b},
        radius::20,
        blur::8,
        backdrop-blur::14,
        shadow::"0 16px 40px rgba(0,0,0,.35)"
    }
}
```

Gradient syntax currently supports a linear gradient:

```tn
paint::{ gradient::{ angle::135, from::#6c5ce7, to::#8b7cf0 } }
```

## Motion

```tn
Button {
    motion::{
        transition::"transform .2s ease, background-color .2s ease",
        hover::{ scale::1.06, background::#8b7cf0 }
    }
}
```

`scale::1.06` is the typed shorthand for a CSS `transform: scale(1.06)`.

## Themes and imports

```tn
import "./topbar.tn";

ui fn App() {
    state theme = "dark"
    theme::dark { Page { paint::{ background::#0a0a0a } } }
    theme::light { Page { paint::{ background::#ffffff } } }
}
```

`.tn` imports are currently expanded by the sandbox loader before the normal
Tint parser runs.

The landing entry is `sandbox/src/main.tn`. Its HTML shell only mounts the
runtime; page structure, themes, imports, and font selection are expressed in
Tint. The small `sandbox/src/main.js` file remains as the browser adapter for
WASM, CodeMirror, and the live preview.
