# Project Status & Roadmap

A snapshot of what the RuneLang parser/compiler actually implements today, versus
what's still just spec or idea. This is the most volatile document in the set — treat
it as a working checklist, not a stable reference.

## Implemented

**Lexing & literals:** numbers (`10`, `3.14`, `-5`), strings, identifiers,
parentheses, unary (`-x`, `!x`), binary operators with precedence
(`+ - * / %`, `== != < > <= >=`, `&& ||`), `true`/`false`, `()` (unit),
string interpolation in logic (`"a {b}"`).

**Bindings:** `let a = 10`, `let a: i32 = 10`, and the Rune-style field-literal form
`let a{10}` / `let a: i32{10}`.

**Structs:** both a Rust-style declaration (`struct User { id: i32, name: string }`)
and a Rune-style one (`struct User { id{i32}, name{string} }`) are accepted, but a
single struct can't mix the two styles. Initialization mirrors this: `User { id: 10,
name: "Marek" }` or `User { id{10}, name{"Marek"} }`. Struct update syntax also
works: `User { ..old, score{40} }`.

**Enums:** unit variants (`Ready`), tuple variants (`Error(string)`), and struct-like
variants (`BadInput { msg: string, code: i32 }`).

**Arrays:** `[1, 2, 3]`, nested (`[[1,2], [3,4]]`).

**Functions:** `fn add(a: i32, b: i32) -> i32 { return a + b; }`, implicit return,
and the short expression form `fn add(a: i32, b: i32) = a + b`. Default parameters
(`fn greet(name: string, prefix: string{"Hello"})`) and named-argument-style calls
(`login { user{"Marek"}, password{"123"} }`) both parse.

**Lambdas:** `|x| x + 1`, `|a,b| a*b`.

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

**Tuples:** `(10, 20)`, destructuring (`let (x, y) = p`), as return types
(`fn dims() -> (i32, i32) { (1920, 1080) }`), and in `match`.

**Modules:** `use X::Y`, `mod`, `export` parse (see `guide/modules.md` for the
settled design).

**Map literals:** `map { a{1}, b{2} }`.

**Generics:** `Option<T>` / `Result<T, E>` as tagged unions with struct-like
variants (`Some { value }` / `None {}`, `Ok { value }` / `Err { error }`), plus
user-defined generic structs (`Pair<T, U>`).

**Union type sugar:** `type Number = union(i32 | f32 | f64)`.

**Kits and GPU kernels:** `@kit.*` struct composition (see `guide/kits.md`) and
`kernel` declarations with `global_id()`, `.gpu()` calls, and `borrow`-gated GPU
work all parse, alongside `space` blocks for grouping kernels/impls/functions.

## Not yet implemented

- A settled `Option`/`Result` DSL (`throw`, `try`, `catch`) — several shapes have
  been prototyped but none is final.
- Move-checking beyond the basic resource model in
  `guide/resources-and-borrowing.md` (e.g. `let t = s; print(s)` as a hard error for
  ordinary values, not just resources).
- The full UI DSL: `ui fn`, UI nodes, modifiers, `animate{}`, events. The UI syntax
  documented in `ui/` describes the target design; the parser doesn't yet accept all
  of it.

## Open syntax questions (still being explored)

A few areas have more than one candidate syntax still in play, not yet converged on
one form:

- **Parameter binding in UI-ish calls:** `checked: item.done` vs `checked{item.done}`,
  `onClick: || onToggle(id)` vs `onClick{|| onToggle(id)}`.
- **UI function parameter typing:** `ui App(count: i32)` vs `ui App(count{i32})`.
- **Attribute style tags** like `mod [@(style)]`, `fn [@classic]`, `ui [@(opt.speed)]`
  — an experiment in marking declarations as using the "classic" (Rust-like,
  `field: Type`) vs "Rune-style" (`field{Type}`) convention, or requesting a compiler
  optimization hint. Not settled.
- **Component-style declaration blocks** — `component`, `system`, `server`, `view`
  keywords appear in exploratory snippets (an ECS-flavored component/system model, a
  server/interval-driven background task, a projection/"view" of a struct's fields)
  but aren't part of the current spec.
- **Shader block variants** — a `shader compute Name(...) { ... }` form and a
  `shader Name { input{} output{} wgsl { ... } }` form (embedding raw WGSL) have both
  been sketched, alongside the `kernel` form documented in `guide/gpu-kernels.md`.

None of the above should be treated as settled syntax — they're recorded here so the
exploration isn't lost, not as something to build against.

## Roadmap (suggested order)

1. Booleans + the unit literal
2. Tuples
3. Default parameters
4. Named-argument call syntax
5. Struct update syntax
6. Pattern matching
7. Map literals + `Option`/`Result`
8. Modules
9. The UI DSL

Later, lower priority: async/await with a minimal state machine, guard patterns in
`match` (already partly working), built-in error types, confirming `match` behaves
as an expression everywhere (it mostly already does).
