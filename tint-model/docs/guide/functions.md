# TintLogic functions

Logic functions use typed parameters, explicit or inferred return values, and
ordinary Tint expressions. UI nodes are declared separately in `ui fn` bodies.

```tn
fn add(a: i32, b: i32) -> i32 {
    a + b
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
```

Functions and variables use camelCase. UI node names use PascalCase.

## UI event handlers

UI connects to named functions through `||` attributes:

```tn
fn increment() {}

ui fn Counter() {
    state count = 0
    Button { click||increment "+" }
}
```

The runtime currently supports `click`, `hover_in`, and `hover_out` handler
attributes. A UI node only references a handler by name.

## Methods and generics

```tn
struct User { id: i32, name: string }

impl User {
    fn label(self) -> string { self.name }
}

fn identity<T>(value: T) -> T { value }
```

Ownership and resource borrowing are described in
`resources-and-borrowing.md`.
