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

### Reusable styles

Named `style` blocks contain reusable modifiers but do not create a DOM node.
Attach one with `use::StyleName`; local modifiers are applied after the named
style and therefore override its values:

```tn
style ButtonBase {
    layout::{ padding.x::18, padding.y::10 }
    motion::{ transition::"transform .2s ease", hover::{ scale::1.06 } }
}

Button {
    use::ButtonBase
    paint::{ background::@button-bg }
    "Open"
}
```

Styles can be declared inside imported UI fragments, including inside a theme
block. They are collected for the surrounding `ui fn` before rendering. A
missing style reference is reported by semantic checking as
`UnknownUiStyle`.

### Components and slots

Components are reusable UI templates. A component declaration does not render
by itself; calling its name creates the component root node. `slot::content` is
the default slot and receives ordinary children from the call site:

```tn
component ActionButton {
    use::ButtonBase
    slot::content
}

ActionButton {
    click||open_github
    "GitHub"
}
```

Named slots use `slot::name` in the component and `slot name { ... }` at the
call site:

```tn
component Card {
    slot::header
    slot::content
}

Card {
    slot header { Text { "Settings" } }
    Text { "Account settings" }
}
```

Slot declarations are structural placeholders and never create wrapper DOM
nodes. Components can use styles and tokens in their own modifiers.

Components may define named visual variants. A variant is selected at the call
site with `variant::name`; its modifiers are merged between the component's
base styles and the call-site overrides:

```tn
component Button {
    use::ButtonBase

    variant::solid {
        paint::{ background::@button-bg, color::@button-text }
    }

    variant::outline {
        paint::{ background::transparent, color::@button-outline-text }
    }

    slot::content
}

Button { variant::solid "Sandbox" }
Button { variant::outline "GitHub" }
```

An unknown variant is reported by semantic checking as `UnknownUiVariant`.

### Composition order and scope

The current reusable UI layers have a predictable merge order:

```text
component base modifiers
    ↓
selected variant modifiers
    ↓
use::StyleName modifiers
    ↓
local node modifiers
```

Theme tokens are resolved when the UI tree is built. A token is a value, while
a style is a group of modifiers and a component is a structural template with
slots. Keeping these roles separate makes it possible to change a theme without
duplicating component markup.

Styles and components declared in imported fragments belong to the surrounding
`ui fn`, so a fragment can provide reusable building blocks for the page that
imports it. Declarations do not render DOM nodes until a style is used or a
component is called.

The current component system intentionally has no props yet. Children, named
slots, event attributes, routes, styles, variants, and theme tokens cover the
current component use cases; props can be added later when components need
parameterized data rather than structural content.

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

## Grouping children

A node's modifiers and its children live in the same block, with no
syntax marking where one ends and the other begins -- normally fine, but
several modifiers followed by several structural children can blur
together at a glance. `children { ... }` is an optional tag for exactly
that case: it splices its own children straight into the parent (no
wrapper node of its own), so it's purely a readability grouping, not a
new kind of node.

```tn
Button {
    click||restart
    layout::{ padding::{ left::20, right::20 } }

    children {
        Icon { "refresh" }
        Text { "Restart" }
    }
}
```

A single child (plain text, or one element) is unambiguous either way, so
`children { ... }` is worth reaching for once a node has more than one --
not required for every block.

`children` is a reserved tag name: the builder recognizes it by name
alone and always splices it, so a component you name `children` will not
render as its own node -- pick a different name for one.

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

External URLs use the same syntax. Add `target||"_blank"` to open the link in
a new browser tab; the DOM renderer also adds `rel="noopener noreferrer"`:

```tn
GithubLink {
    route||"https://github.com/tintlang/tint"
    target||"_blank"
    "GitHub"
}
```

## Themes and imports

Themes can provide named UI tokens. A token is a named design value (for example
a color, spacing value, radius, or font size). A `tokens { ... }` block is consumed by the
active theme and does not create a DOM node. Token references use `@name` and
work anywhere a UI modifier value is accepted:

```tn
theme::dark {
    tokens {
        text-main::white
        button-bg::#ffffff
    }
}

Headline { text::{54, bold, @text-main} }
Button { paint::{ background::@button-bg } "Open" }
```

The existing form where a theme contains rendered UI remains supported. Tokens
are resolved while the UI tree is built, so the resulting style list contains
ordinary values and renderers do not need token support. The semantic checker
reports an error when a UI modifier references a token that is not declared in
any theme of the current `ui fn`.

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
