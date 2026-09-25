# RuneLogic: Type System

RuneLang's type system is strict and minimal, inspired by Rust but adapted for
WASM, GPU, and declarative UI. Types are predictable, stable, and fully static.

## Primitives

`i32` (32-bit signed int), `f32` (32-bit float), `bool`, `string` (immutable UTF-8).

## Math types

`vec2` `(x, y)`, `vec3` `(x, y, z)`, `vec4` `(x, y, z, w)`, `mat2` (2×2), `mat4`
(4×4).

```
let p: vec2 = vec2(10, 20);
```

## GPU-specific types

`Texture` (GPU texture, sample/read/write), `Image` (CPU image buffer), `Buffer`
(GPU buffer) — used in `kernel` and Rune2D/GPU2D.

## Containers

```
let users: List<User>;
let dict: Map<string, i32>;
```

## Structs

```
struct User { id: i32, name: string, age: i32 }
let u = User { id: 1, name: "A", age: 20 };
```

## Enums (algebraic types)

```
enum Result<T, E> { Ok(T), Err(E) }
enum State { Ready, Loading, Error(string) }
```

## Option\<T>

```
let name: Option<string>;   // Some(value) | None
```

## Type aliases

```
type UserId = i32;
type Pair = vec2;
```

## Guarantees

No implicit conversions, no dynamic typing, no nullable values (`Option<T>`
instead), everything resolved at compile time, WASM/RuneVM-compatible.

## Resources cannot live inside your types

```
struct Foo { img: image }           // error
enum State { Ready(image) }         // error
let t: (buffer, i32)                // error
```

The only valid form is a top-level declaration:

```
buffer a
image frame
tensor<f32> weights
```

Resources represent system-level owned memory; embedding them in ordinary values
would break ownership, borrow safety, async-safety, and lifetime determinism. See
`guide/resources-and-borrowing.md`.

## GPU-safe types

Inside `kernel`, only these are available: `i32 f32 vec2 vec3 vec4 Texture Buffer
mat2 mat4`. Not available: `string`, `List<T>`, `Map<K,V>`, structs with dynamic
fields.

## Tuples

```
let t: (i32, string) = (10, "ok")
```

May contain primitives, structs, enums, `vec2`/`3`/`4`, or other tuples. Immutable,
not usable in UI Mode, cannot contain resources.

## Generic structs

```
struct Box<T> { value: T }
let i: Box<i32> = Box { value: 10 }
```

Logic Mode only; a `struct<T>` cannot contain UI, `state`, `signal`, or `computed`,
and generic structs can't be used as UI types.

## Unit type

Written `()`. A function with no return value implicitly returns it:

```
fn log(x: string) -> () {
    print(x)
}
```

`()` cannot appear in UI Mode, struct fields, enum variants, or
`state`/`signal`/`computed`.
