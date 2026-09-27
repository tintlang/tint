# TintLogic: Type System

TintLang's current type system is experimental and Rust-inspired. The stable
surface today is the primitive/value syntax used by the parser and runtime;
advanced resource and GPU types below are draft syntax only.

## Primitives

`i32` (32-bit signed int), `f32` (32-bit float), `bool`, `string` (immutable UTF-8).

## Math types

`vec2` `(x, y)`, `vec3` `(x, y, z)`, `vec4` `(x, y, z, w)`, `mat2` (2×2), `mat4`
(4×4).

```
let p: vec2 = vec2(10, 20);
```

## Draft resource/GPU types

`Texture`, `Image`, `Buffer`, and tensor types are reserved/draft syntax. There
is no supported GPU/WebGPU runtime for them yet.

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
instead), everything resolved at compile time, WASM/TintVM-compatible.

## Resources cannot live inside your types

```
struct Foo { img: image }           // error
enum State { Ready(image) }         // error
let t: (buffer, i32)                // error
```

The following top-level resource syntax is not currently supported end to end:

```
buffer a
image frame
tensor<f32> weights
```

Resource ownership and borrowing remain future work.

## Draft GPU-safe types

Kernel-specific type rules are not part of the current runtime.

## Tuples

```
let t: (i32, string) = (10, "ok")
```

May contain primitives, structs, enums, `vec2`/`3`/`4`, or other tuples. Immutable,
not usable in UI Mode, cannot contain resources.

## Draft generic structs

```
struct Box<T> { value: T }
let i: Box<i32> = Box { value: 10 }
```

Generic syntax parses in some positions, but generic runtime semantics are not
supported end to end.

## Unit type

Written `()`. A function with no return value implicitly returns it:

```
fn log(x: string) -> () {
    print(x)
}
```

`()` cannot appear in UI Mode, struct fields, enum variants, or
`state`/`signal`/`computed`.
