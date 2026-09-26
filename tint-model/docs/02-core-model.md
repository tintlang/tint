# Core model: Logic Mode and UI Mode

Tint has two contexts:

- **Logic Mode** — functions, state, expressions, and control flow.
- **UI Mode** — named UI nodes, text, visual modifiers, and event attributes.

## Logic Mode

Logic code uses ordinary assignments and function calls:

```tn
fn add(a, b) {
    return a + b
}
```

UI modifiers are not logic expressions. `padding::20`, `paint::{...}`, and
`motion::{...}` are valid only inside a UI node.

## UI Mode

UI functions contain a declarative tree:

```tn
ui fn App() {
    state count = 0

    Column {
        layout::{ direction::column, gap::12, padding::20 }
        Text { text::{20, bold, white} "Count: {count}" }
        Button { click||increment "+" }
    }
}
```

The canonical modifier groups are `layout`, `paint`, and `motion`. The flat
modifier form is still accepted for compatibility. Theme blocks use
`theme::name { ... }` and imports use `import "./file.tn";` in the sandbox
source pipeline.

## Boundary

Logic runs outside the UI tree. A UI node may read state and interpolated text,
but it does not contain arbitrary statements. Event attributes connect a node to
a named handler:

```tn
fn increment() {}

ui fn Counter() {
    Button { click||increment "+" }
}
```

This keeps computation and presentation separate without inventing a second
CSS-like language.
