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

## Function types and predicates

Functions can be passed as values. A function type lists its parameter types
and return type with `fn(...) -> ...`:

```tn
fn isPositive(value: i32) -> bool {
    value > 0
}

fn check(predicate: fn(i32) -> bool, value: i32) -> bool {
    predicate(value)
}

fn example() -> bool {
    check(isPositive, 10)
}
```

There is no separate `condition` type or keyword: a predicate is an ordinary
function whose return type is `bool`. Lambdas can be used in the same position:

```tn
let positive = |value| value > 0
check(positive, 10)
```

## UI event handlers

UI connects to named functions through `||` attributes:

```tn
fn increment() {}

ui fn Counter() {
    state count = 0
    Button { click||increment "+" }
}
```

The runtime currently supports `click`, `pointer_down`, `hover_in`, and
`hover_out` handler attributes, plus the game-facing `key_down`, `key_up`,
and `frame` events on a `GameRoot` node. A UI node only references a
handler by name. See `guide/events.md` for the full list and the
`GameRoot` example.

## Methods and generic types

```tn
struct User { id: i32, name: string }

impl User {
    fn label(self) -> string { self.name }
}
```

User-defined generic structs and enums are checked strictly, including generic
arity and field/variant payload types. Generic functions such as
`identity<T>(value: T) -> T` infer their type arguments per call; the typed IR
compiles one instance per distinct set of arguments. Resource borrowing is not
part of the current supported runtime.
