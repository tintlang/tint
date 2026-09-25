# System Resources & the Borrow Model

TintLang exposes low-level, Rust-backed system resources directly in the language,
with a `borrow` block as the only way to access them safely.

## Resource types

```
buffer          // linear block of memory (like Rust's &[T])
image           // 2D image (GPU/CPU hybrid)
tensor<T>       // multi-dimensional numeric array
vec<T>          // dynamic list of primitives
```

These are Rust-backed objects with a unique identity (like Rust owning values), not
copyable by default, existing **only in Logic Mode**, requiring a `borrow` block for
any low-level access.

## Declaring a resource

```
buffer audio
image frame
tensor<f32> weights
vec<i32> points
```

The type is fixed by the declaration keyword; resources can't be initialized with a
literal (`buffer a = 10` is an error). An extended creation form is also available:

```
buffer data = create.buffer{size: 4096}
image frame = create.image{width: 512, height: 512}
tensor<f32> t = create.tensor{shape: [4,4,4]}
vec<number> list = create.vec{capacity: 128}
```

### Loading resources

```
image tex = load.image("ui/button.png")
buffer mesh = load.binary("model.bin")

tensor<f32> w = await load.remote("weights.bin")     // async, returns Result
tensor<f32> v = try await load.remote("model.zip")
```

## Where resources live

A resource can be declared at the top level of a file (global lifetime, lives for
the whole runtime) or inside a `ui fn` (destroyed when the component unmounts, and
inaccessible after `onDestroy`). Resources cannot be declared inside an ordinary
`fn`.

```
buffer a
buffer b = move a       // ok
use(a)                  // error: a was already moved

async fn f() {
    image x
    await load(...)      // error: the owning ui fn could be torn down mid-await
}
```

A resource can't be returned out of a `ui fn`, stored in `state`/`signal`, or used
as a UI-attribute value directly — only passed to a Logic-Mode function or to a
component built to accept it:

```
return frame                          // error: can't leave the UI boundary
state img = frame                     // error
<Panel padding{frame}>                // error: resource as a UI parameter
<ImageView source=frame />            // fine — a real image-consuming component
```

Assigning a resource to another name creates an alias, not a copy — both names
refer to the same underlying resource.

## The `borrow` block

`borrow { ... }` is the only safe way to get exclusive access to a resource.

```
buffer audio

fn process() {
    borrow audio {
        fft(audio)
        normalize(audio)
    }
}
```

It gives exclusive access (like `&mut T`), only allows calling intrinsic functions
inside it, closes access automatically at the end of the block, and synchronizes the
resource between GPU and CPU.

Rules:

- one resource per block — `borrow a, b { ... }` is an error
- borrows can't nest — `borrow a { borrow b { ... } }` is an error
- while borrowed, the resource can't be read as a value: `borrow tex { let x = tex }` is an error
- `borrow` is Logic Mode only — never inside a UI attribute, `animate{}`, or `color{}`

```
<Button onClick=borrow{tex{...}}>   // error
animate{borrow{...}}                 // error
```

The correct pattern is: a plain function does the borrowing, and the UI calls that
function.

```
ui fn Editor() {
    image frame

    fn refresh() {
        borrow frame { sobel(frame) }
    }

    onMount { refresh() }

    <ImageView source=frame />
}
```

### Read-only access — `borrow immut`

```
image frame

fn analyze() {
    borrow immut frame {
        histogram(frame)
        detect_edges(frame)
    }
}
```

Read-only; the resource can't be written inside the block, and only intrinsics
marked `pure` are callable there (`borrow immut frame { blur(frame) }` is an error,
since `blur` mutates). Borrows can't nest, and you can't take an exclusive borrow
while an immut borrow on the same resource is active.

## Intrinsic functions

Intrinsics are TintLang's standard-library system functions, implemented in Rust,
callable **only** inside a `borrow` block:

```
fft(buffer)
blur(image, radius)
bloom(image, threshold)
sobel(image)
normalize(buffer)
triangulate(buffer)
pack_nodes(buffer)
boolean_union(bufferA, bufferB)
boolean_subtract(bufferA, bufferB)
```

Calling one outside a `borrow` block is an error, and their arguments must be
resources, numbers, or literals — never a UI value (`blur(frame, color{blue})` is an
error).

## Move / clone

```
buffer a
buffer b = move a        // a is no longer accessible
buffer c = clone a        // only if the resource type supports clone
```

`move` is not allowed inside a `borrow` block, and a resource with an active borrow
can't be moved.

## Async + borrow

A `borrow` block must complete **before** the first `await` in the function that
contains it — borrows can't be held across an `await` (this mirrors Rust's async
model):

```
async fn work() {
    borrow img { blur(img) }
    await load()          // fine — the borrow already ended
}

async fn wrong() {
    borrow img {
        await load()      // error: borrow held across an await
    }
}
```

## Slots

A `borrow` can't appear directly inside slot content; put it in a function and call
that from a slot handler instead:

```
<Children>
    borrow img { ... }     // error
</Children>

fn update() { borrow img { ... } }
<Slot onMount=update />
```

## Matrix / shader interop

```
shader {
    fn main(uv: Vec2) -> Color {
        return sample(frame, uv)
    }
}

m = frame              // error — a resource is not a matrix
frame * matrix.scale    // error
```

Resources can be passed to the GPU as textures, but never treated as matrices.

## Formal grammar

```
BorrowBlock      ::= "borrow" BorrowMode? Ident Block
BorrowMode       ::= "immut"
Block            ::= "{" BorrowStatement* "}"
BorrowStatement  ::= IntrinsicCall
IntrinsicCall    ::= Ident "(" ExprList? ")"
ResourceDecl     ::= ResourceType Ident
ResourceType     ::= "buffer" | "image" | "tensor" "<" Type ">" | "vec" "<" Type ">"
MoveStmt         ::= Ident "=" "move" Ident
CloneStmt        ::= Ident "=" "clone" Ident
AsyncFn          ::= "async" "fn" Ident "(" ParamList? ")" Block
AwaitExpr        ::= "await" Expr
```

## Full examples

```
// Audio processing
buffer audio
fn process() {
    borrow audio { fft(audio); normalize(audio) }
}

// Image postprocessing
image frame
fn applyEffects() {
    borrow frame { blur(frame, 4); bloom(frame, 0.7) }
}

// Tensor load
tensor<f32> weights
async fn reload() {
    let data = await load.remote("model.bin")
    weights = data
}

// Async + borrow, done safely
async fn safe() {
    borrow frame { sobel(frame) }
    await wait()
}

// move + borrow
buffer a
buffer b = move a
fn optimize() { borrow b { triangulate(b) } }
```

## Common errors, at a glance

```
<Panel padding{frame}>          // resource used as a UI value
borrow a, b { ... }              // more than one resource in a block
buffer a; b = move a; use(a)     // use after move
enum A { x(image) }              // resource embedded in a value type
state m = frame                  // resource stored in state
```

## Experimental: strict / group borrow modes (draft)

Early design notes sketch a second, opt-in borrow mode for engine/GPU/WASM-runtime
code, alongside the default described above. This is **not finalized** — it's kept
here for context, not as settled spec.

The idea: by default, `borrow(x)` is a high-level, identity-based borrow — you can
`map`/`filter`/`push`/restructure the collection inside it, similar to Python or
Swift. A module or function can opt into a stricter, Rust-like mode with the
`[@(strict)]` attribute (or a local `strict { ... }` block), where `borrow(x)`
becomes a "core" borrow — no `map`/`sort`/`move` inside it, identity is frozen — and
a separate `borrow@group(x)` form is available for the high-level behavior even
inside a strict function:

```
fn [@(strict)] gpu_process() {
    borrow(frame) {          // core borrow (frame's identity is frozen)
        sobel(frame)
    }

    borrow@group(kernel) {   // opts back into high-level borrow
        sort(kernel)
    }
}
```

If this lands, expect it to appear as a clearly versioned addition to the rules
above, not a replacement for them.
