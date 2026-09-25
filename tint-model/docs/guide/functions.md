# TintLogic: Functions & Methods

TintLang functions borrow from Rust and Swift: strict typing, implicit return,
methods via `impl`, bounded generics, async/await, `throws`/`try`, inline handlers,
lambdas — and deliberately no overloading.

## Definition

```
fn add(a: i32, b: i32) -> i32 {
    return a + b;
}

fn add(a: i32, b: i32) -> i32 {   // implicit return
    a + b
}

fn now() -> i32 {                  // no parameters
    time()
}
```

## Parameter types are mandatory

```
fn x(a, b) { ... }                 // error
fn x(a: i32, b: string) { ... }    // correct
```

Why: TintLang is statically typed throughout, and inferred parameter types would
break TintIR optimization, VM predictability, WebGPU shader specialization, and the
borrow model. Return types *can* sometimes be inferred; parameter types never are.

## Short forms

```
fn lerp(a: f32, b: f32, t: f32) = a + (b - a) * t     // expression body

onClick = fn() {                                        // inline function
    counter = counter + 1
}

let doubled = map(values, |x| x * 2)                    // lambda: |param| expr
```

## Async

```
async fn loadData() -> Data {
    let v = await fetch()
    parse(v)
}
```

## Throws / try

```
fn load() throws -> File {
    let raw = try readFile()
    parse(raw)
}
```

`try`/`throws` are forbidden in UI Mode.

## Methods via `impl`

```
struct User { id: i32, name: string, age: i32 }

impl User {
    fn isAdult(self) -> bool {
        self.age >= 18
    }

    fn rename(self, newName: string) -> User {
        User { id: self.id, name: newName, age: self.age }
    }
}

let ok = user.isAdult()
```

Static methods work the same way, called on the type:

```
impl Math {
    fn clamp(x: f32, min: f32, max: f32) -> f32 {
        if x < min { min } else if x > max { max } else { x }
    }
}

let v = Math.clamp(a, 0, 1)
```

## Generics (bounded, Logic Mode only)

```
fn identity<T>(value: T) -> T { value }
fn length<T: Vector>(v: T) -> f32 { v.len() }
```

## What's deliberately missing

No overloads (`fn add(a: i32, b: i32)` and `fn add(a: f32, b: f32)` together is an
error), no varargs, no kwargs, no default parameters. A return type is always
required — `fn bar() { ... }` without `-> T` is an error.

## Ownership

```
fn consume(v: Vec2) { ... }
consume(a)              // a is moved

let b = clone a          // explicit clone

borrow immut tex {       // read-only access
    sample(tex)
}
```

See `guide/resources-and-borrowing.md` for the full ownership/borrow model.

## Functions in UI Mode

UI Mode cannot define or call functions, except as a handler value:

```
<Button onClick=increment>
```

```
fn click(e: ClickEvent) {
    count = count + 1
}

<Button onClick=click>
```

## Naming

Functions and variables: camelCase (`loadData`, `userId`, `isReady`). Components:
PascalCase (`<Panel>`, `<Text>`). Constants: UPPER_SNAKE_CASE (`const PI = 3.14`).

## Full example

```
struct Vec2 { x: f32, y: f32 }

impl Vec2 {
    fn length(self) -> f32 {
        sqrt(self.x*self.x + self.y*self.y)
    }

    fn scale(self, s: f32) -> Vec2 {
        Vec2 { x: self.x*s, y: self.y*s }
    }
}

fn dist(a: Vec2, b: Vec2) -> f32 {
    let d = Vec2 { x: b.x - a.x, y: b.y - a.y }
    d.length()
}

fn midpoint(a: Vec2, b: Vec2) -> Vec2 {
    Vec2 { x: (a.x + b.x)/2.0, y: (a.y + b.y)/2.0 }
}
```
