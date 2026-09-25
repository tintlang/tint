# RuneTensor — GPU-Native ML Layer

RuneLang has a built-in tensor engine and a declarative ML layer on top of it —
Rune's pitch is that UI, general logic, GPU graphics, and machine learning all live
in one language, with no Python, no JS, and no external ML framework. It runs in the
browser, in WASM, on WebGPU.

Status: the tensor type system and GPU ops below are a formal, near-final spec; the
higher-level `model { }` layer described at the end is an earlier-stage design.

## The tensor type

A `tensor<T>` is a GPU buffer of fixed shape; it lives only on the GPU and is only
touched through `borrow`, in mut mode, for anything that writes to it.

```
tensor<f32>     tensor<i32>     tensor<bool>     tensor<f16>     tensor<u8>
```

```
t.shape       // [d0, d1, ...]
t.rank        // number of dimensions
t.size        // total element count
```

## Constructors

```
let t = tensor.zero<f32>(shape{ [3, 3] })
let t = tensor.ones<f32>(shape{ [256] })
let t = tensor.rand<f32>(shape{ [8, 8] })
let t = tensor.from([ [1,2], [3,4] ])
let t = tensor.load("model.bin")
```

## High-level operations (GPU)

Every operator below dispatches as its own GPU compute call:

```
t1 + t2    t1 - t2    t1 * t2    t1 / t2
t * 3.0    3.0 * t

t1.matmul(t2)          t1 @ t2                 // matrix multiply

t.sum(axis=1)          t.mean(axis=0)          t.max(axis=2)     // reductions

t.reshape(shape{ [...] })   t.transpose()   t.squeeze()   t.expand(shape{ [...] })

t.clamp(min{0}, max{1})     t.where(mask, a, b)

t.softmax(axis{1})     t.layernorm()     t.gelu()
```

## Low-level, borrow-gated ops (intrinsics)

Mutating tensor ops need a `borrow` block, same as any other resource (see
`resources-and-borrowing.md`):

```
borrow x {
    matmul(x, W)
    add(x, b)
    softmax(x)
}

borrow immut x {
    print(x.sum())
}
```

Intrinsics: `add mul matmul softmax layernorm gelu` (all taking tensors).

## GPU execution model

Each expression compiles to a separate compute shader, automatically split into
workgroups and scheduled — there is no CPU fallback; this is GPU-only by design.

## Memory model

Tensors live in GPU memory. Reading on the CPU needs an explicit copy
(`t.to_host()`); mutation always requires a `borrow`. The usual borrow rules apply:
`borrow x { ... }` for mut access, `borrow immut x { ... }` for read-only, no nested
borrows, and no holding a borrow across an `await`.

## Async loading

```
async fn load_model() {
    let w = await tensor.load("w.bin")
    let b = await tensor.load("b.bin")
    borrow w { normalize(w) }
}
```

## Tensors and UI don't mix

The UI can never mutate a tensor directly — a plain function does the mutation, and
the UI just triggers it:

```
fn update() {
    borrow weights { normalize(weights) }
}

ui fn App() {
    <Button onClick=update>"Recompute"</Button>
}
```

## Worked example: a mini neural layer

```
tensor<f32> W
tensor<f32> B

fn linear(x) {
    borrow x { matmul(x, W) }
    x + B
}

fn classify(input) {
    let x = linear(input)
    x.softmax(axis{1})
}
```

---

## The `model { }` layer

On top of raw tensors, Rune sketches a declarative syntax for whole models,
compiled to an optimized compute graph (with fusing, tiling, smart batching, and
weight preloading):

```
model MLP {
    dense 256 -> relu
    dense 256 -> relu
    dense vocab
}

let out = MLP.forward(input)     // the whole forward pass runs as one GPU program
```

**Training loops** are written directly in the language:

```
state loss = 0.0

fn trainStep(batch: tensor<f32>, target: tensor<f32>) {
    let pred = MLP.forward(batch)
    let diff = tensor.sub(pred, target)

    loss = tensor.mean(diff * diff)

    compute SGD {
        uniform lr{0.001}
        write(Weights, Weights - lr * diff)
    }
}
```

No Python, no PyTorch, no JS — everything runs on WebGPU, in the browser.

**Autodiff** (planned, not yet built): `let grads = MLP.backward(loss)`, with
RuneVM constructing the gradient graph automatically.

**ML + UI** compose directly — a tensor can be rendered as an image with no manual
conversion step, and training and UI updates stay reactive together:

```
ui fn Dashboard(loss: number, tensor img) {
    <Column padding{20}>
        <Text text{xl}> "Loss: {loss}" </Text>

        <Rune2D height{320}>
            rune2d {
                texture img
                colormap{inferno}
            }
        </Rune2D>
    </Column>
}
```

**A small CNN:**

```
model CNN {
    conv 3x3 -> relu
    conv 3x3 -> relu
    dense 128 -> relu
    dense 10  -> softmax
}

fn classify(img: tensor<f32>) {
    let out = CNN.forward(img)
    return tensor.argmax(out)
}
```

**Larger models (concept only)** — the same Tensor + Compute DSL is meant to scale
to GPT-style transformers, LoRA-style adapter tuning, RNN/GRU, and attention
kernels:

```
model TinyGPT {
    embed 256
    transformer 4 heads 256 dim
    dense vocab
}
```

The pitch: one language where UI, GPU graphics, and ML training/inference share a
single syntax, running client-side with no server and no Python — useful for visual
ML demos, on-device training, edge inference, and WebGPU AI playgrounds.
