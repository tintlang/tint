# Kits — Reusable Field Groups

Status: draft → ready.

A `kit` is a named, reusable group of struct fields — declarative,
ECS-style composition. A kit expands into the AST at parse time (compile-time,
before typechecking); it doesn't generate text, so its behavior stays predictable,
type-safe, and fast.

## Defining a kit

```
kit position2d {
    x{f32},
    y{f32},
}

kit uv {
    u{f32},
    v{f32},
}

kit color {
    r{f32},
    g{f32},
    b{f32},
    a{f32},
}
```

## Using a kit in a struct

```
struct Sprite {
    @kit.position2d
    @kit.uv
    @kit.color
}
```

expands, at compile time, to:

```
struct Sprite {
    x{f32}, y{f32},
    u{f32}, v{f32},
    r{f32}, g{f32}, b{f32}, a{f32},
}
```

## Composing kits from other kits

```
kit rotation { angle{f32}, }

kit transform2d {
    @kit.position2d
    @kit.rotation
}
```

`@kit.transform2d` expands to `x{f32}, y{f32}, angle{f32}`.

## Nested kit blocks

A kit can carry its own scope for more complex composite layouts:

```
@kit.transform2d {
    @kit.position {
        x{f32},
        y{f32},
    }

    @kit.rotation {
        angle{f32},
    }
}
```

## GPU layout metadata

Kits can carry layout tags for automatic buffer generation, std140/std430 packing,
and shader binding:

```
kit gpu_vertex {
    @align(16)
    position{vec3<f32>},
    normal{vec3<f32>},
}
```

## Rules

- A kit can't redefine a field that already exists on the struct it's applied to.
- Kits can't be used inside UI blocks.
- Expansion happens at parse time (compile time).
- Nested kits expand recursively.

## Worked example: a GPU sprite vertex

```
kit position2d { x{f32}, y{f32} }
kit size2d     { w{f32}, h{f32} }
kit uv         { u{f32}, v{f32} }
kit color4     { r{f32}, g{f32}, b{f32}, a{f32} }

struct SpriteVertex {
    @kit.position2d
    @kit.size2d
    @kit.uv
    @kit.color4
}

// expands to:
// struct SpriteVertex {
//     x{f32}, y{f32},
//     w{f32}, h{f32},
//     u{f32}, v{f32},
//     r{f32}, g{f32}, b{f32}, a{f32},
// }
```
