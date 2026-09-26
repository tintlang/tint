# UI events

Events are UI attributes. They connect an element to a named function with
`||`:

```tn
fn open_settings() {}

ui fn Toolbar() {
    Button { click||open_settings "Settings" }
}
```

The runtime currently recognizes `click`, `hover_in`, and `hover_out`. Event
handlers are names, not inline expressions, and they do not use the visual
modifier groups.

```tn
Icon {
    click||open_menu
    hover_in||highlight_menu
    hover_out||clear_menu
}
```

Visual hover changes belong in `motion`:

```tn
Button {
    motion::{ transition::"transform .2s ease", hover::{ scale::1.04 } }
    click||submit
    "Submit"
}
```

Use only named `click||`, `hover_in||`, and `hover_out||` attributes for event
handlers. Visual hover changes belong in `motion`/`hover` modifiers.
