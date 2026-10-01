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

## Keyword properties

These CSS properties have no typed Tint form. They take a bare word
(`cursor::pointer`) or a string (`outline::"2px solid #fff"`), and work in
`layout`, `paint`, `hover` and breakpoint blocks alike:

`cursor`, `pointer-events`, `touch-action`, `user-select`, `outline`,
`box-sizing`, `align-self`, `flex-wrap`, `text-align`, `appearance`,
`transform-origin`, `color-scheme`, `caret-color`, `letter-spacing`, `word-break`, `font-style`, `animation` (and its `animation-*` longhands), `object-fit`, `object-position`, `clip-path`, `mix-blend-mode`, `text-overflow`, `text-transform`, `text-wrap`, `vertical-align`, `visibility`, `resize`, `will-change`, `backface-visibility`, `isolation`, `justify-self`, `place-items`, `place-content`, `grid-area`, `grid-column`, `grid-row`, `scroll-snap-type`, `scroll-snap-align`, `scroll-behavior`, `transform-style`, `perspective-origin`.

Also typed: `flex-basis` and `perspective` (number = px or string), `order` and `scroll-margin` (number), `line-clamp::3` (cut text after 3 lines with an ellipsis), `content::"x"` (for `before`/`after`).

Transforms combine instead of overriding each other: `rotate::15` (deg), `scale::1.1`, `skew::10`, `skew.x::10`, `skew.y::10`, `translate.x::4` and `translate.y::4` (px) become one `transform` value, in the order written.

`ring::{2, #6c5ce7}` is a `box-shadow` ring (`0 0 0 2px #6c5ce7`) that is added to any `shadow` on the node, the usual focus ring.

Sizes accept numbers (pixels) or strings: `min-width`, `min-height`,
`max-width`, `max-height`, `inset` (`min-height::"100dvh"`). `flex`,
`flex-shrink` and `aspect-ratio` accept a number or a string.

Tint also injects a zero-specificity reset for its own nodes (`box-sizing:
border-box`, links inherit color and have no underline), so an app needs no
stylesheet for it; any stylesheet you add wins.

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

## States and pseudo-elements

Besides `hover::{...}`, a node can style these states with the same block syntax (in `motion::{...}`, or flat on the node):

`focus`, `focus-visible`, `focus-within`, `active` (`tap` is the same), `disabled`, `checked`, `read-only`, `invalid`, and the pseudo-elements `placeholder`, `before`, `after`, `selection`.

```tn
TextArea {
    placeholder||"Name"
    disabled||{locked}
    paint::{ background::#1d2030, color::white }
    motion::{
        focus::{ ring::{2, #6c5ce7} },
        disabled::{ opacity::0.5 },
        placeholder::{ color::#7a7f95 }
    }
}
Label { before::{ content::"* ", color::red } "Name" }
```

`placeholder||"text"`, `disabled||{expr}` (a bool: present when true), `readonly||{expr}`, `title||"..."`, `alt||"..."` and `tabindex||n` set plain HTML attributes. These states are written as rules in one shared `<style>` and win over the node's own style.

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

The site entry is `sandbox/src/site.tn`. It is the whole program: page
structure, routes, themes, imports, fonts, the code editor and the live preview
are all Tint. `tint dev src/site.tn` serves it and `tint build src/site.tn`
produces one HTML file; there are no hand-written HTML, JS or CSS files. See
[text-and-preview.md](text-and-preview.md) for `TextArea` and `Preview`.
