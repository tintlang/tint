# Language Reference

This is the terse, precise syntax spec. For explanations, rationale, and worked
examples, see the `guide/` and `ui/` folders — this document is the lookup
reference, not the tutorial.

It assumes the [Logic Mode / UI Mode](02-core-model.md) split.

## 0. Lexical model

Tokens: identifiers (`myVar`, `addUser`, `UserProfile`), literals (`10`, `3.14`,
`"hello"`), keywords (`fn`, `let`, `state`, `ui`, `match`, `enum`, `module`,
`export`, ...), symbols `{} () : = -> => .. + |`, and `//` line comments
only (`/* */` is not supported).

Strings interpolate with `{}`: `"value = {a + b}"`.

### Naming rules

- UI nodes: PascalCase — `Button`, `Panel` (`panel` is not the canonical form)
- functions and variables: camelCase — `drawRect`, `userId`
- constants: UPPER_SNAKE_CASE — `const PI = 3.14`
- modules: dotted — `module ui.widgets`
- shadowing is not allowed within one `ui fn`

## 1. Expressions (Logic Mode only)

Arithmetic `+ - * /`, comparison `< > <= >= == !=`, logical `&& || !`, parentheses.

```
lambda_expression = "|" ident ("," ident)* "|" expr
try_expression    = "try" expr
move_expression   = "move" ident
clone_expression  = "clone" ident
```

Operator precedence matches Rust. Vector operators extend arithmetic: `+ - * · ×`.

**Pipe operator:** `value |> normalize() |> clamp(0,1) |> lerp(a,b)`.

**Semicolons** are an optional statement terminator in Logic Mode (`let x = 10;` and
`let x = 10` are both valid) and are **forbidden anywhere in UI Mode** — before,
inside, or after a UI node, or inside `{...}` interpolation. `Panel { ... };`,
`Text { "{value}" };`, `Button { click||run };` are all errors because UI mode
does not use semicolon terminators.

In UI Mode, expressions are allowed **only inside string interpolation**:
`Text { "Sum: {a + b}" }`.

## 2. Types

Primitives: `i32 i64 u32 u64 f32 f64 bool string unit` (`unit` is written `()`).
Math types: `vec2 vec3 vec4 mat2 mat4`. GPU/resource types:
`Texture Image Buffer buffer image tensor<T> vec<T>`. Containers: `List<T> Map<K,V>`.

```
struct User { id: i32, name: string, age: i32 }
let u = User { id: 1, name: "A", age: 20 }

enum Result<T, E> { Ok(T), Err(E) }
enum State { Ready, Loading, Error(string) }
enum Theme { dark, light, system }        // elements used unquoted: dark

type UserId = i32
```

`Option<T>` is `Some(value)` / `None`.

Rules: no implicit conversions, no dynamic typing, no nullable types (use
`Option<T>`), everything is known at compile time.

**Const / static:**

```
const PI = 3.14159          // compile-time
const VERSION = "1.0.0"

static theme: Theme = dark  // runtime-init, immutable, type annotation required
```

**Tuples:** `let t: (i32, string) = (10, "ok")`. Immutable, cannot appear in UI
Mode, cannot contain resources.

**Generic structs:** `struct Box<T> { value: T }` — Logic Mode only; a `struct<T>`
may not contain UI, `state`, `signal`, or `computed`.

**GPU-safe types** (inside `kernel`): only `i32 f32 vec2 vec3 vec4 Texture Buffer
mat2 mat4` — no `string`, `List<T>`, `Map<K,V>`, or dynamic structs.

**Resources cannot be embedded** in structs, enum variants, tuples, or
`Option`/`Result`/`List`/`Map` payloads — only as a top-level declaration
(`buffer a`, `image frame`). See `guide/resources-and-borrowing.md`.

### Struct spread

```
let u2 = User { ...u1 }
let u2 = User { ...u1, name: "Tint" }          // later fields win
let merged = User { ...defaults, ...override, age: 30 }
```

Logic Mode only, single struct type, move semantics by default (`User { ...clone
base }` for a clone). `...settings` is only valid inside a struct literal.

## 3. Functions

```
fn add(a: i32, b: i32) -> i32 { return a + b }   // explicit return
fn add(a: i32, b: i32) -> i32 { a + b }          // implicit return (last expr)
fn lerp(a, b, t) = a + (b - a) * t               // short form
```

**Parameter types are always required** — `fn add(a, b) { a + b }` is a compile
error. This is deliberate: it's needed for TintIR optimization, VM predictability,
WebGPU shader specialization, and the borrow model. Return types can sometimes be
inferred (`fn length(v) = sqrt(...)`); parameter types never are.

Methods via `impl`:

```
impl User {
    fn isAdult(self) -> bool { self.age >= 18 }
}
let ok = user.isAdult()
```

Static methods: `impl Math { fn clamp(x, min, max) -> f32 { ... } }` → `Math.clamp(a, 0, 1)`.

Generics (Logic Mode only, bounded): `fn identity<T>(value: T) -> T { value }`,
`fn length<T: Vector>(v: T) -> f32 { v.len() }`.

No overloads, no varargs, no kwargs, no default parameters.

Ownership: move by default (`consume(a)` moves `a`); `clone a` for an explicit
copy; `borrow immut tex { sample(tex) }` for read-only access.

In UI Mode, functions are referenced by named event attributes:
`Button { click||increment }`.

## 4. Control flow (Logic Mode only)

```
if cond { ... } else { ... }

for i in 0..10 { ... }
for user in users { ... }

match value {
    A => { ... }
    B => { ... }
}
match node {
    MoveTo(p): ...          // extended colon form
    LineTo(p): ...
}

let {x, y} = v              // pattern destructuring
```

UI Mode uses `if{...}`, `for{...}`, and `match{...}` directly on a named node;
see `ui/blocks.md`.

Errors/`try`:

```
fn load() throws -> Data {
    let raw = try read()
    return parse(raw)
}
```

`try` and `Result` are fully forbidden in UI Mode.

## 5. UI modifiers, themes, and motion

UI modifiers use `::`; logic attributes use `||`. The canonical style form groups
properties by intent:

```
Card {
    layout::{ padding::24, gap::12, direction::column }
    paint::{ gradient::{ angle::135, from::#6c5ce7, to::#8b7cf0 }, radius::20 }
    motion::{ transition::"transform .2s ease", hover::{ scale::1.03 } }
}
```

The supported groups are `layout`, `paint`, and `motion`. Layout includes flex
and `grid::{ columns, rows, gap }`; use `minmax(0, 1fr)` tracks for panes that
must not widen their page. Theme variants use `theme::name { ... }`. Internal
links use `route||"/path"`. See `ui/modifiers.md` for the complete list.

## 7. `ui fn` and reactivity

```
ui fn Counter() {
    state n = 0
    fn inc() { n = n + 1 }

    Column {
        Text { "{n}" }
        Button { click||inc "+" }
    }
}
```

A `ui fn` returns exactly one root UI node, is memoized, and re-runs when its
parameters, `state`, `signal`, or `computed` change.

```
state c = 0
signal id = 42
computed isEven = (c % 2 == 0)
watch(c) { ... }
```

Typing rules for `state` / `signal` / `computed`:

- `state`/`signal` type is inferred from the initial value and then **fixed forever**
  (`state x = 10; x = "hi"` is an error). Allowed types: `number`, `string`, `bool`,
  `struct`, `enum`, resources. A system-mode value (theme, platform, mode) must be
  an `enum`, never a raw string.
- `computed`'s type is fixed by its **first successful evaluation**; returning a
  different type later is an error. `computed` may not contain `await`, a state
  write, or other side effects.
- `watch(x)` fires after `x` changes, and can only watch `state` or `signal`.

UI types: `string, number, bool, UI, UIChild, UIChildren, Color, Length, Texture,
SlotRef`.

**Event propagation:** target → bubble, with `stopPropagation`; there is no capture
phase. Full event catalogue: `guide/events.md`.

## 8. Async status

`async`/`await` is not implemented in the current runtime. See
`guide/async.md`; it is not part of the active UI syntax.

## 10. Modifier values

Inside a modifier tuple, items are comma-separated. UI nodes use named blocks;
there is no indentation-sensitive syntax:

```
Button {
    click||open_menu
    layout::{ padding.x::18, padding.y::10 }
    paint::{ radius::full }
}
```

## 11. Symbol semantics

| Symbol | Meaning |
|---|---|
| `{}` | logic block scope, UI node, or modifier tuple (by context) |
| `()` | function call |
| `=` | logic assignment |
| `||` | named UI event attribute |
| `:` | type annotation |
| `->` | function return type |
| `=>` | match case |
| `..` | range |
| `::` | modifier path/value separator |
| `""` | string literal |

## 12. Common errors

```
padding=20             // visual values use ::
increment{5}            // logic function call needs ()
fn x() { Text { "x" } } // UI is not written in Logic Mode
for x in list() {}      // list() returns an object; iterate the collection itself
log("text")             // VM/debug helpers use {}: log{"text"}
```

## 13. Fragments, comments, entry point

Empty fragments are not part of the UI syntax. Comments are `//` only. Entry point:

```
ui fn App() { ... }
run(App)
```

## 14. Protocols (Logic Mode only)

Tint protocols are pure interfaces (no fields, no default implementations, no
generics/lifetimes) — closer to Swift protocols than Rust traits:

```
protocol Drawable { fn draw() }

struct Circle { radius: f32 }

impl Drawable for Circle {
    fn draw() { /* ... */ }
}

fn render(shape: Drawable) { shape.draw() }
```

Dispatch through a protocol is dynamic. `impl` must implement every function in the
protocol; `protocol A { x: i32 }` (a field) is an error.

## 15. `try` as an early-exit operator

```
fn loadUser() {
    let raw = try fetch("user.json")
    let parsed = try decode(raw)
    process(parsed)
}
```

If any step returns `Err(e)`, the function returns `Err(e)` immediately. Logic Mode
only; `try x + 1` (applied to a non-`Result`) is an error.

## 16. Matrix DSL (Logic Mode only)

```
matrix4x4 m
m = matrix.identity
m = matrix.translate(x, y, z)
m = matrix.scale(sx, sy, sz)
m = m * matrix.rotateZ(3.14)          // * is the only overloaded operator

borrow immut m { gpu.apply_transform(m) }
```

Not usable in UI Mode (`Panel { transform::"matrix.identity" }` is an error), not
comparable or hashable.

## 17. Stdlib

Zero-cost, Logic-Mode-only helpers that lower to plain loops/ops in TintIR:

```
array.map / filter / find / findIndex / any / all / sum / max / min
math.clamp / lerp / sqrt / abs / round / floor / ceil
option.unwrap / unwrapOr
result.unwrap / unwrapOr
string.length / contains / startsWith / endsWith
range(start, end) -> List<number>
```

The stdlib cannot be redefined, cannot do IO, cannot allocate resources
(`buffer`/`image`/...), and is not usable in UI Mode outside interpolation.

## 18. Modules — summary

See `guide/modules.md` for the full walkthrough. Core rules: one file = one module;
a directory becomes a namespace only if it has a `run.tn`; imports are absolute
(`use ui::panel`) with no `.tn` extension and no relative paths; `use` is Logic Mode
only.

## 19. Formal grammar (EBNF)

```
Program        ::= (Import | Export | Fn | UiFn)*

Import         ::= "use" Path
Export         ::= "export" (Fn | UiFn | Enum | Struct)
Path           ::= Ident ("::" Ident)*

# --- Functions ---
Fn             ::= "fn" Ident "(" ParamList? ")" Block
UiFn           ::= "ui" "fn" Ident "(" ParamList? ")" UiBlock
ParamList      ::= Param ("," Param)*
Param          ::= Ident ":" Type

# --- UI blocks ---
UiBlock        ::= UiNode+
UiNode         ::= Ident "{" UiContent "}"
UiContent      ::= (Modifier | Attribute | UiNode | TextLiteral)*
Modifier       ::= Ident ("." Ident)* "::" ModifierValue
Attribute      ::= Ident "||" (Ident | Literal | "{" Expr "}")
ModifierValue  ::= Literal | Ident | "{" TupleContent "}"
TupleContent   ::= (Literal | Ident | MiniModifier) ("," (Literal | Ident | MiniModifier))*
MiniModifier   ::= Ident ("." Ident)* "::" ModifierValue
ThemeNode      ::= "theme" "::" Ident "{" UiContent "}"

# --- Text & interpolation ---
TextLiteral    ::= '"' (Char | Interpolation)* '"'
Interpolation  ::= "{" Expr "}"

# --- Logic blocks ---
Block          ::= "{" Statement* "}"
Statement      ::= Let | Assign | If | For | Match | Watch | Return | Expr ";"
Let            ::= "let" Ident (":" Type)? "=" Expr
Assign         ::= Ident "=" Expr
Return         ::= "return" Expr?
If             ::= "if" Expr Block ("else" Block)?
For            ::= "for" Ident "in" Expr ".." Expr Block
Match          ::= "match" Expr MatchBody
MatchBody      ::= "{" (MatchCase)+ "}"
MatchCase      ::= Pattern "=>" Expr ","
Pattern        ::= Ident | Ident "(" PatternList ")"
PatternList    ::= Pattern ("," Pattern)*

# --- Expressions ---
Expr           ::= Lambda | TryExpr | MoveExpr | CloneExpr | BinaryExpr
Lambda         ::= "|" Ident ("," Ident)* "|" Expr
TryExpr        ::= "try" Expr
MoveExpr       ::= "move" Ident
CloneExpr      ::= "clone" Ident
BinaryExpr     ::= UnaryExpr (BinOp UnaryExpr)*
UnaryExpr      ::= ("!" | "-")? Primary
Primary        ::= Literal | Ident | Ident "(" ExprList? ")" | "(" Expr ")"
                 | StructLiteral | EnumConstructor
ExprList       ::= Expr ("," Expr)*
StructLiteral  ::= Ident "{" StructFieldList? "}"
StructFieldList ::= (StructField | StructSpread) ("," (StructField | StructSpread))*
StructField    ::= Ident ":" Expr
StructSpread   ::= "..." Ident
EnumConstructor ::= Ident "(" ExprList? ")"
BinOp          ::= "+" | "-" | "*" | "/" | "==" | "!=" | "<" | ">" | "<=" | ">=" | "&&" | "||"

# --- Borrow / resources ---
BorrowBlock      ::= "borrow" BorrowMode? Ident Block
BorrowMode       ::= "immut"
BorrowStatement  ::= IntrinsicCall
IntrinsicCall    ::= Ident "(" ExprList? ")"
ResourceDecl     ::= ResourceType Ident
ResourceType     ::= "buffer" | "image" | "tensor" "<" Type ">" | "vec" "<" Type ">"
MoveStmt         ::= Ident "=" "move" Ident
CloneStmt        ::= Ident "=" "clone" Ident
AsyncFn          ::= "async" "fn" Ident "(" ParamList? ")" Block
AwaitExpr        ::= "await" Expr

# --- Types ---
Type           ::= Ident | Ident "<" Type ">" | "tensor" "<" Type ">" | "vec" "<" Type ">"
Literal        ::= number | string | bool
Ident          ::= letter (letter | digit | "_")*
```
