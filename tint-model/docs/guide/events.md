# UI events

Events are UI attributes. They connect an element to a named function with
`||`:

```tn
fn open_settings() {}

ui fn Toolbar() {
    Button { click||open_settings "Settings" }
}
```

The runtime currently recognizes `click`, `pointer_down`, `hover_in`, and `hover_out`. Event
handlers are names, not inline expressions, and they do not use the visual
modifier groups.

`pointer_down||handler` is the immediate press variant for controls and games.
It is normalized to the same click handler slot as `click||handler`, while the
browser renderer dispatches it on the pointer press.

Game-facing input events are also supported by the browser renderer:

```tn
GameRoot {
    key_down||on_key_down
    key_up||on_key_up
    frame||on_frame
}

fn on_key_down(key: string) { last_key = key }
fn on_key_up(key: string) { last_key = key }
fn on_frame(dt: f32) { elapsed = elapsed + dt }
```

`key_down` and `key_up` receive the browser key name as a string. `frame`
receives elapsed seconds as `dt`. The host supplies the events and clock; the
game state and simulation remain in Tint.

### Interval timers

`tick||handler` runs `handler` every `every||ms` milliseconds (1000 if `every`
is left out); `frame||` above is the per-frame clock, this one is a fixed
interval. The handler takes no arguments.

```tn
ui fn App() {
    state n = 0
    state running = true

    Column {
        Text { "count {n}" }
        Ticker { tick||step every||100 if{running} }
    }
}

fn step() {
    n = n + 1
    if n >= 5 { running = false }
}
```

A timer lives as long as the node that declares it is rendered: put the node
behind `if{}` to start and stop it from state. `every` needs a `tick` beside
it and does nothing on its own. The interval is at least 10 ms.

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
