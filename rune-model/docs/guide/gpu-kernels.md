# GPU Kernel System

`kernel { ... }` is a declarative RuneLogic construct that compiles directly to a
WebGPU compute shader — no WGSL, no external shader files. It's the low-level,
WebGPU-first layer of the language, for writing your own shaders and GPGPU
processes in Rune itself.

## Defining a kernel

```
kernel blur(tex: Texture, radius: f32) -> Texture {
    // GPU compute code
}
```

A kernel takes strictly typed arguments, returns a `Texture` or `Buffer`, runs as a
GPU compute pipeline, has no access to global state, and cannot call UI or Rune2D.

## Calling a kernel

```
fn applyBlur(img: Image) -> Image {
    return gpu.compute(blur, img, 10.0);
}
```

`gpu.compute(kernelFn, arg1, arg2, ...)` runs the kernel and returns its result.

## Types available inside a kernel

```
i32, u32, f32, bool
vec2, vec3, vec4
mat2, mat3, mat4

Texture
Image
Buffer
Sampler          // planned

GlobalId    // thread's global ID
LocalId     // ID within the workgroup
GroupId     // workgroup index
```

```
let id = global_id();    // vec3
let gid = group_id();    // vec3
```

## Compute-shader body

```
kernel invert(tex: Texture) -> Texture {
    let id = global_id();
    let px = tex.read(id);
    let inv = vec4(1.0 - px.r, 1.0 - px.g, 1.0 - px.b, px.a);
    write(inv);
}
```

Reading and writing: `let px = tex.read(id);` reads a pixel; `write(color);` writes
to the output texture for the current thread. Rune generates the WGSL, configures
the workgroup, and bounds-checks texture access automatically.

## Math library inside kernels

```
sin(x), cos(x), tan(x)
pow(x, y)
sqrt(x)
abs(x)
clamp(x, a, b)
mix(a, b, t)
length(v)
normalize(v)
dot(a, b)
cross(a, b)
```

## Control flow inside kernels

`if`/`else`, `for`, and `loop`/`break`/`continue` are all supported:

```
if px.r > 0.5 { write(vec4(1,0,0,1)); } else { write(px); }

for i in 0..4 { acc += pow(value, i); }

loop { if x > 1.0 { break; } }
```

## Restrictions

Not allowed inside a kernel: runtime struct creation, dynamic allocation,
`Option`/`Result`, UI components, reading CPU variables, recursion.

Allowed: pure computation, texture read/write, vector math, workgroup
synchronization (planned for v3).

## Workgroups (planned)

v2's scheduling is automatic. A future version adds explicit control:

```
@workgroup(16, 16)
kernel myKernel(...) { ... }

gpu.dispatch(kernel, size=vec2(1024,1024));
```

## Integration

With UI:

```
ui App {
    let blurred = gpu.compute(blur, avatar, 6.0);
    <Image source=blurred />
}
```

With Rune2D:

```
let base = gpu2d { fill { rect(0,0,200,200) color=blue-500 } };
let final = gpu.compute(blur, base, 8.0);
```

## Worked example: Gaussian blur

```
kernel gaussian(tex: Texture, sigma: f32) -> Texture {
    let id = global_id();
    let px = tex.read(id);

    let sum = vec4(0,0,0,0);
    let wsum = 0.0;

    for x in -3..3 {
        for y in -3..3 {
            let n = tex.read(id + vec2(x,y));
            let d = float(x*x + y*y);
            let w = exp(-d / (2.0*sigma*sigma));
            sum += n * w;
            wsum += w;
        }
    }

    write(sum / wsum);
}
```

## Built-in kernel utilities

```
global_id()      -> vec3
local_id()       -> vec3
group_id()       -> vec3
dispatch_size()  -> vec3
texture_size(t)  -> vec2
```

## Compile errors

The kernel compiler rejects: wrong types, out-of-bounds access, unsupported
operations, recursion, UI/2D API calls, `Option`/`Result`, dynamic allocation.

## Summary

The kernel system gives Rune full low-level WebGPU compute access with a
declarative DSL, strict typing, no undefined behavior, direct
`Texture`/`Image`/`Buffer` interop, and integration with Rune2D and UI — GPU as a
native part of the language, not an external shader file.
