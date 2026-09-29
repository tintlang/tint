# TintLogic: Type System

TintLang's current type system is experimental and Rust-inspired. The stable
surface today is the primitive/value syntax used by the parser and runtime;
advanced resource and GPU types below are draft syntax only.

## Primitives

`()`/`unit`, `bool`, `string`/`str`, `number`, `i32`, `i64`, `u8`, `u32`,
`u64`, `f32`, and `f64`.

`i32`, `i64`, `u8`, `u32`, `u64`, `f32`, and `f64` are distinct runtime
values. `number` is the compatibility alias for `f64`.

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

## Option\<T> and Result\<T, E>

```
let name: Option<string> = Option::Some { value: "Tint" }
let missing: Option<string> = Option::None {}
let parsed: Result<number, string> = parse_number("42")

match parsed {
    Ok { value } => value,
    Err { error } => 0
}
```

`Option<T>` uses `Some { value }` and `None {}`. `Result<T, E>` uses
`Ok { value }` and `Err { error }`. `expect(message)` and `unwrap()` return
the successful payload and fail at runtime for `None`/`Err`. The postfix `?`
operator extracts `Some`/`Ok` and returns `None`/`Err` from the current
function:

```tn
fn read_value() -> Result<f32, string> {
    let value = parse_number(read_line())?
    Result::Ok { value: value }
}
```

Entry-point calls convert failed `expect`/`unwrap` operations into runtime
errors instead of exposing an uncaught evaluator panic.

Standard methods are available for branching and callback-style composition:

```tn
let fallback = missing.unwrap_or("guest")
let upper = name.map(|value| value)
let next = parsed.and_then(|value| Result::Ok { value: value })
let present = name.is_some()
let absent = missing.is_none()
let successful = parsed.is_ok()
let failed = parsed.is_err()
```

`is_some`/`is_none` apply to `Option<T>`; `is_ok`/`is_err` apply to
`Result<T, E>`; calling one on the other type is a type error. `unwrap_or`
returns the payload or its fallback value. `map` transforms a successful
payload and preserves failure, while `and_then` returns the callback's
`Option`/`Result` and preserves failure. Callback signatures and payload types
are checked statically. A bare `Option` or `Result` without type arguments is
an error.

Full method set (`T` payload, `E` error):

| Method | On | Returns |
|---|---|---|
| `unwrap()`, `expect(msg)` | both | `T` |
| `is_some()`, `is_none()` | `Option` | `bool` |
| `is_ok()`, `is_err()` | `Result` | `bool` |
| `is_some_and(f)`, `is_ok_and(f)` | `Option`, `Result` | `bool` |
| `is_err_and(f)` | `Result` | `bool` |
| `unwrap_or(v)` | both | `T` |
| `unwrap_or_else(f)` | both (`f` takes `E` for `Result`) | `T` |
| `map(f)` | both | same kind, new payload |
| `map_or(d, f)` | both | type of `d`/`f` |
| `and_then(f)` | both | `f`'s `Option`/`Result` |
| `filter(f)` | `Option` | `Option<T>` |
| `or(o)`, `or_else(f)` | both | same kind |
| `ok_or(e)`, `ok_or_else(f)` | `Option` | `Result<T, E>` |
| `map_err(f)` | `Result` | `Result<T, F>` |
| `ok()`, `err()` | `Result` | `Option<T>`, `Option<E>` |
| `unwrap_err()`, `expect_err(msg)` | `Result` | `E` |

The checker and the typed IR support all of these. The tree-walking runtime
(`tint run`) implements `unwrap`, `expect`, `is_*`, `unwrap_or`, `map` and
`and_then` only. A lambda without parameters (`|| body`) does not parse yet,
so `unwrap_or_else` on an `Option` needs a named function.

## Type aliases

```
type UserId = i32;
type Pair = vec2;
```

## Function types

Function types use the form `fn(ParameterType, ...) -> ReturnType` and can be
used for callbacks and predicates:

```tn
type Predicate = fn(i32, f32, bool) -> bool;

fn accepts(predicate: Predicate, value: i32) -> bool {
    predicate(value, 0.5, true)
}
```

Named functions and lambdas are both callable values. Function values are
checked by parameter count/types and return type; there are no implicit
conversions.

## Guarantees

No implicit conversions, no dynamic typing, no nullable values (`Option<T>`
instead), everything resolved at compile time, WASM/TintVM-compatible.

## Numeric conversions

Explicit numeric conversions use Rust-like `as` syntax, for example
`let count: u8 = value as u8` and `let ratio: f32 = count as f32`.
Implicit conversion between numeric types is intentionally not part of the
language. Converting to an integer rejects fractional, non-finite, and
out-of-range values; unsigned types also reject negative values. Converting to
`f32` rejects values outside the representable finite range.

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

## Generic types

```
struct Box<T> { value: T }
let i: Box<i32> = Box { value: 10 }
```

`Option<T>` and `Result<T, E>` are supported by the semantic checker and
runtime. User-defined generic structs and enums are now checked strictly:
type arguments must match their declared arity and field/variant payloads are
validated after substituting `T`, `E`, and other parameters.

```tn
struct Box<T> { value: T }
enum Response<T, E> {
    Ok { value: T },
    Err { error: E }
}

let boxed: Box<i32> = Box { value: 42 }
let response: Response<i32, string> = Response::Ok { value: 42 }
```

The runtime executes these values with their concrete payloads; generic
argument checking is a compile-time responsibility.

## Unit type

Written `()`. A function with no return value implicitly returns it:

```
fn log(x: string) -> () {
    print(x)
}
```

`()` cannot appear in UI Mode, struct fields, enum variants, or
`state`/`signal`/`computed`.
