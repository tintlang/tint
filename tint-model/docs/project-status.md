# Project Status & Roadmap

A snapshot of what the TintLang parser/compiler actually implements today, versus
what's still just spec or idea. This is the most volatile document in the set — treat
it as a working checklist, not a stable reference.

## Implemented

**Lexing & literals:** numbers (`10`, `3.14`, `-5`), strings, identifiers,
parentheses, unary (`-x`, `!x`), binary operators with precedence
(`+ - * / %`, `== != < > <= >=`, `&& ||`), `true`/`false`, `()` (unit),
string interpolation in logic (`"a {b}"`).

**Bindings:** `let a = 10` and `let a: i32 = 10` are canonical. The older
Tint-style initializer form `let a{10}` / `let a: i32{10}` remains accepted for
compatibility, although it is not used by current examples.

**Structs:** the canonical declaration is Rust-style (`struct User { id: i32,
name: string }`). The older Tint-style form (`struct User { id{i32},
name{string} }`) remains accepted for compatibility, but a single struct can't
mix the two styles. Initialization follows the same rule: `User { id: 10,
name: "Marek" }` is canonical, while `User { id{10}, name{"Marek"} }` is
legacy-compatible. Struct update syntax also works: `User { ..old, score: 40 }`.

**Enums:** unit and struct-like variants are usable in the current runtime.
Tuple-style variants are parser-level only and are not supported end to end.

**Arrays:** `[1, 2, 3]`, nested (`[[1,2], [3,4]]`).

**Functions:** `fn add(a: i32, b: i32) -> i32 { return a + b; }`, implicit return,
the short expression form `fn add(a: i32, b: i32) = a + b`, first-class function
types such as `fn(i32, f32, bool) -> bool`, and passing named functions as values.
Conditional expressions use `if condition { value } else { value }`; a separate
`condition` keyword is not needed. Default parameters
use `=` canonically (`fn greet(name: string, prefix: string = "Hello")`); the
older brace-default forms remain accepted. Named arguments use the brace call
form: `login{user: "Marek", password: "123"}`, or with defaults skipped
(`three{a: 1, c: 9}`); the checker, the tree-walker and the typed IR bind them
the same way.

**Lambdas:** lambdas work end to end through the tree-walking evaluator,
including lexical capture and calling a lambda stored in a local (`|| body`
for no parameters). The typed IR lowers lambdas with closure conversion. The
older SSA IR tier (`ir` feature) still lacks closure values, so functions with
lambdas stay on the evaluator there.

**Control flow:** `if/else`, `while`, `loop`, `for x in 0..10`, `return`, `break`,
`continue`.

**Pattern matching:** literal, wildcard, and guard patterns; struct and tuple
destructuring in `match` arms; nested enum patterns.

```
match value {
    x if x > 10 => "big",
    _ => "small"
}

match user {
    User { id, name } => print("{id}: {name}"),
    _ => print("unknown user"),
}
```

**Tuples:** the syntax parses and is covered by parts of the tree-walking runtime;
the full IR/runtime path is still being completed.

**Modules:** `use X::Y`, `mod`, and `export` work end to end via a
`tint-cli`-side loader (not a VM-level concept): `mod name;` resolves to
`name.tn` or `name/mod.tn` (the `mod.rs` convention). `use` paths are always
absolute from the entry file's own module tree (no `self::`/`super::`),
but otherwise support grouped imports (`use a::{b, c}`, arbitrarily
nested), `as` aliasing (`use a::b as c;`), and wildcard imports
(`use a::*;`) alongside the plain single-item form, and only
`fn`/`struct`/`enum` are exportable/importable. See `examples/modules/`
and `tint-cli/src/module_loader.rs`'s own tests. Note: this is a
different, simpler design than the `run.tn`-based one sketched in
`guide/modules.md` — that doc describes a future direction, not what's
built.

**UI loops:** two mechanisms. The `for{var in iterable}` modifier repeats
the node it's written on -- its whole subtree, once per item -- so grouping
more than one repeated node needs a wrapper. A standalone
`for { var in iterable } { ...body... }` block is a child in its own right:
it splices a whole run of sibling nodes per iteration straight into the
parent, needs no wrapper, and can sit between static siblings. Unlike the
modifier form, it requires explicit `{...}` braces around its body. See
`ui/blocks.md`.

**Semantic checker:** a typed pass now runs over `fn` bodies, `ui fn` bodies
and `impl` methods. It infers primitive, array, tuple, map, struct, enum and
function/lambda types; checks operators, conditions, assignments, explicit
returns, function arguments, struct fields, method arguments and UI
state/interpolation expressions; and reports `TypeMismatch` alongside the
existing undefined-variable, duplicate-binding and immutability errors.
Lambda parameters, `Option`/`Result` holes (`None {}`) and `?` are inferred, and on
the conformance suite and the examples no expression is left `Unknown`
(`tint check --strict` turns every remaining uninferred type into a
`CannotInfer` error). Generic functions are inferred per call (rigid type
parameters inside their bodies). Every UI attribute's host-specific value
contract remains future work. `tint check` reports errors as fatal;
`tint run` reports them as non-fatal warnings and still executes.

**Typed IR:** `tint-ir::typed` lowers the checked AST to a typed register CFG
(closure conversion, monomorphised enums, layout contract, verifier) and runs it
on a reference interpreter. The conformance suite passes on it and on the
tree-walker. Generic functions are monomorphised on demand, default parameters
and named arguments are bound at the call site, and `ui fn` lowers to a
function that emits `UiOpen`/`UiText`/`UiClose` instructions (styles,
components, variants and slots are expanded statically; `if{}`, `for{}`,
`match{}`, themes and interpolations are ordinary IR). With the runtime feature
`typed-ir`, `IrUiSink` turns the emitted stream into the same `UiTree` the
tree-walking builder makes; `tests/ui_ir.rs` compares the two on the examples
and on events. Kernels are not lowered: the checker and the tree-walker give
them no semantics yet. Limits: expressions inside theme `tokens` are not
evaluated, and non-scalar modifier values render as `auto` (as in the walker).

**Map literals:** `map { a{1}, b{2} }`.

**Generics:** `Option<T>` and `Result<T, E>` are supported by the checker and
runtime (the checker, the typed IR and the tree-walker all implement the full
method set listed in `guide/types.md`). User-defined generic `struct` and `enum` declarations now preserve
their parameters in the AST; the checker validates arity and substitutes
generic parameters while checking fields and variant payloads.

**Numeric types:** `i32`, `i64`, `u8`, `u32`, `u64`, `f32`, and `f64` have
distinct runtime values. Explicit `as` casts, range checks, typed arithmetic,
and typed numeric return values are supported. `number` is the compatibility
alias for `f64`.

**Union type sugar:** `type Number = union(i32 | f32 | f64)`.

**Kits and GPU kernels:** syntax experiments exist in the parser, but there is
no supported GPU/WebGPU runtime for them.

**Browser runtime package:** a prebuilt `@tintlang/runtime` package now owns
WASM initialization and exposes source and serialized-program loading through
the same `mount`/session API. Application authors do not run `wasm-pack`.

**Vite integration:** `@tintlang/vite` makes `.tn` files importable as JS
modules and preserves the same source/entry contract for development and HMR.

## Not yet implemented or not supported end to end

- Error handling is implemented through `Option<T>`, `Result<T, E>`, their
  standard methods (the full set in `guide/types.md`), and postfix `?`
  propagation. A
  larger `throw`/`try`/`catch` error DSL remains future work.
- Move-checking beyond the basic runtime model, including ordinary-value moves.
- Broader component semantics and additional runtime behavior remain future work.
- GPU/WebGPU, Tint2D, tensors, async execution, resource borrowing, and kits
  are intentionally outside the current supported scope. The module system
  has a basic working version (see above); a richer `run.tn`-style design
  (relative paths, grouped-with-`as`-in-one-statement ergonomics beyond
  what's already supported, a real external-package concept) remains
  future work.
- The browser sandbox is a working prototype. `npm run dev:all` watches nested
  `.tn` imports and rebuilds Rust/WASM changes automatically.
- The serialized program format is versioned and functional, but its first
  payload is still a serialized Tint program rather than compact SSA bytecode.

## Roadmap

A typed end-to-end semantic checker (covering `fn`, `ui fn`, and `impl`
method bodies; fully inferred on the conformance suite and the examples),
a real standalone build that embeds the WASM runtime, a
module system with grouped/aliased/wildcard `use` imports, and `match{}`
evaluation in UI trees (`case label { ... }`/`case _ { ... }` children --
see `tint-runtime/tests/render_ui.rs`) have all landed. The earlier XML
UI dialect (`<Tag ...>...</Tag>`) has been removed entirely -- block
syntax (`Tag { ... }`) is now the only way to write UI, including
`match{}`'s case arms, and a `children { ... }` grouping tag is available
for separating a node's own children from its modifiers when both are
present in the same block. The next useful milestones are a stable
language reference, direct lambda lowering to SSA IR (the runtime fallback
is documented above and `tint-ir/src/compiler/expressions.rs` still has
the lowering placeholder), and broader UI component semantics. GPU/WebGPU
and async features remain future experiments.
