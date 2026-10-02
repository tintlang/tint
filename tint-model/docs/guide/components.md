# Components with props and state

A `component` declared with parameters, `state` or `fn`s is a *component*. Each use
gets its own copy of the state and handlers.

```tn
ui fn App() {
    component Counter(label: string, start: i32 {0}) {
        state n = start
        fn inc() { n = n + 1 }
        Button { click||inc "{label}: {n}" }
    }
    Column {
        Counter { label::"A" start::5 }
        Counter { label::"B" }
    }
}
```

- Props are written as modifiers at the call site: `label::"A"`. A default in
  `{...}` makes a prop optional; a missing required prop is a compile error.
- `state` and `fn` declarations come first in the body; they are private to
  each use (internally renamed `Counter__1__n`).
- Children of a use fill the `Slot` of the component; other modifiers are
  forwarded as style modifiers.
- Components nest. A component with state cannot be used inside `for`
  (every row would share the state); props-only components can.
- Components are expanded at parse time, so they cost nothing at runtime and
  work identically in the interpreter and compiled mode.
