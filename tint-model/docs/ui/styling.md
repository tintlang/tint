# Styling reference

Tint styling is a small declarative layer resolved by the UI runtime. Only the
modifiers listed here are currently implemented.

```tn
Card {
    paint::{ background::#6c5ce7, radius::20, border::{1, #8b7cf0} }
    motion::{ hover::{ scale::1.03 } }
}
```

## Supported paint modifiers

| Tint modifier | Result |
|---|---|
| `background::color` | `background-color` |
| `gradient::{...}` | linear `background-image` |
| `color::color` | text color |
| `font-family::"..."` | font family |
| `border::{width, color}` | solid border |
| `radius::number` | border radius in pixels |
| `radius::full` | pill/circle radius |
| `opacity::number` | opacity |
| `shadow::string` | box shadow |
| `blur::number` | element blur |
| `backdrop-blur::number` | backdrop blur |

## Supported layout modifiers

`direction`, `grid`, `align`, `justify`, `gap`, `grow`, `position`, `top`,
`right`, `bottom`, `left`, `z`, `padding`, `padding.x/y/t/b/l/r`, `margin`,
`min-width`, `min-height`, `overflow`, and `size`.

`grid::{ columns::2 }` creates equal columns. A string track list such as
`"minmax(0, 2fr) minmax(0, 1fr)"` gives explicit proportions.

Side and axis aliases work for spacing and borders: `margin.x/y/t/b/l/r` and
`border.x/y/t/b/l/r` (with full names such as `border.top` also accepted).

## Text

`text::{size, weight, color}` resolves the first number to `font-size`, a known
weight keyword to `font-weight`, and a color to `color`. Font selection is a
regular Tint style and inherits through the UI tree:

```tn
Text { text::{18, bold, #ffffff} "Tint" }
Logo { font-family::"ui-rounded, -apple-system, BlinkMacSystemFont, sans-serif" "tint" }
```

## Motion

```tn
Button {
    motion::{
        transition::"transform .2s ease",
        hover::{ scale::1.06, shadow::"0 8px 20px rgba(0,0,0,.25)" }
    }
}
```

The language currently supports transitions and hover style resolution through
`transition::...` and `hover::{...}`.
