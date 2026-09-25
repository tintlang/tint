# Language Reference (Syntax v2.9)

This is the terse, precise syntax spec. For explanations, rationale, and worked
examples, see the `guide/` and `ui/` folders — this document is the lookup
reference, not the tutorial.

It assumes the [Logic Mode / UI Mode](02-core-model.md) split.

## 0. Lexical model

Tokens: identifiers (`myVar`, `addUser`, `UserProfile`), literals (`10`, `3.14`,
`"hello"`), keywords (`fn`, `let`, `state`, `ui`, `match`, `enum`, `module`,
`export`, ...), symbols `{} () <> </> /> : = -> => .. + |`, and `//` line comments
only (`/* */` is not supported).

Strings interpolate with `{}`: `"value = {a + b}"`.

### Naming rules

- UI components: PascalCase — `<Button>`, `<Panel>` (`<panel>` is an error)
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
inside, or after a UI node, or inside `{...}` interpolation. `<Panel;>`,
`<Text>{value};</Text>`, `<Button onClick=run>;` are all errors.

In UI Mode, expressions are allowed **only inside string interpolation**:
`<Text>"Sum: {a + b}"</Text>`.

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

In UI Mode, functions can't be defined or called except as a handler value:
`<Button onClick=increment>`.

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

UI Mode uses `<Block if=...>` / `<Block for=...>` / `<Block match=...>` instead —
see `ui/blocks.md`.

Errors/`try`:

```
fn load() throws -> Data {
    let raw = try read()
    return parse(raw)
}
```

`try` and `Result` are fully forbidden in UI Mode.

## 5. UI modifiers, assignment, and animation tokens

Every visual parameter uses `{}`; every logical/behavioral attribute uses `=`.
Full modifier catalogue: `ui/modifiers.md`. Full styling/effects: `ui/styling.md`.

```
padding{20}
<Block if=isReady>
<Button onClick=submit>
```

Animation transitions use `->`, with semantic shorthands:

```
opacity: 0 -> 1
visible = 1        hidden = 0
grow    = scale 1.0 -> 1.1     shrink = scale 1.0 -> 0.9
enter   = opacity 0->1 + y 12px->0px
exit    = opacity 1->0 + y 0px->12px
```

## 6. Style system: `tintstyle` and `theme`

Both are declarative, zero-cost UI-Mode DSLs (inlined into TintIR at compile time);
neither allows logic, expressions, or computation.

**`tintstyle`** — reusable mixin groups of modifiers, top-level only:

```
tintstyle CardStyle {
    padding{14}
    radius{12}
    background{gray-900}
    shadow{0px 4px 16px black{24%}}
}

<Panel ...CardStyle>
    <Text>"Hello"</Text>
</Panel>
```

`...Name` inlines the style. A `tintstyle` cannot contain `<UI/>` elements,
variables, `state`, `signal`, or `computed`, but may reference `theme` fields.

**`theme`** — a namespaced hierarchy of visual tokens (colors, lengths, numbers,
opacity only — no expressions, strings, variables, or function calls):

```
theme DarkTheme.Button {
    primary{blue-400}
    background{gray-900}
    textColor{white}
}

<Text color{DarkTheme.Button.textColor}>
```

Theme fields are compile-time aliases; they cannot be read in Logic Mode
(`let x = DarkTheme.Button.background` is an error) and a theme is not an object
(`background{DarkTheme}` is an error — you must name a leaf field).

Precedence when combined: manual UI modifiers > `tintstyle` modifiers > theme
fields referenced inside a `tintstyle`.

## 7. `ui fn` and reactivity

```
ui fn Counter() {
    state n = 0
    fn inc() { n = n + 1 }

    <Column>
        <Text>{n}</Text>
        <Button onClick=inc>"+"</Button>
    </Column>
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

**Binding:** `<Input bind=text />` — `bind` only accepts `state` or `signal`
(`bind=42`, `bind=user.name` are errors).

**Event propagation:** target → bubble, with `stopPropagation`; there is no capture
phase. Full event catalogue: `guide/events.md`.

## 8. Lifecycle

```
animate:onMount{...}
onUpdate(state)
onDestroy=cleanup
```

## 9. Async (Logic Mode only — see `guide/async.md` for the full model)

```
async fn load() { ... }
<Button onClick=load />
```

`await` is only valid inside `fn`/`async fn`.

## 10. List/parameter syntax

Inside `{}`, items are comma-separated: `text{xl, bold}`. Outside `{}`, on a tag,
commas between attributes are accepted and ignored by the parser:

```
<Button
    onClick=foo,
    padding{20},
>
```

Strings are for user-facing text only — never for system configuration like
`theme`/`mode` (use an `enum` there).

## 11. Symbol semantics

| Symbol | Meaning |
|---|---|
| `{}` | logic block scope, UI modifier, or object literal (by context) |
| `()` | function call |
| `=` | logic assignment / logic attribute |
| `:` | type annotation |
| `->` | animation transition, or function return type |
| `=>` | match case |
| `..` | range |
| `<>` | UI node |
| `""` | string literal |

## 12. Common errors

```
padding=20            // visual param needs {}, not =
increment{5}           // logic function call needs (), not {}
fn x() { <Text/> }     // no UI inside Logic Mode
animate=...             // animate needs {}, not =
<Else> outside <Block>  // Else only inside <Block if>
for x in list() {}      // list() returns an object; iterate the collection itself
log("text")             // VM/debug helpers use {}: log{"text"}
```

## 13. Fragments, comments, entry point

Empty fragments `<>...</>` are not allowed. Comments are `//` only. Entry point:

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

Not usable in UI Mode (`<Panel transform{matrix.identity}>` is an error), not
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
UiNode         ::= "<" Ident AttributeList "/>"
                 | "<" Ident AttributeList ">" UiChildren "</" Ident ">"
UiChildren     ::= (UiNode | TextLiteral | Interpolation | Block)*

# --- Offset modifiers (axis model, shared by layout/offset modifiers) ---
OffsetModifierIdent ::= "offset" | "offset" "." Axis
Axis           ::= "t" | "b" | "l" | "r" | "x" | "y" | "all"
ModifierItem   ::= OffsetModifierIdent UiModifierBlock
                 | Ident
                 | Ident ":" ModifierValue
                 | Ident UiModifierBlock

# --- Attributes ---
AttributeList  ::= (Attribute (","?) )*
Attribute      ::= AttributeName "=" Value
                 | AttributeName UiModifierBlock
AttributeName  ::= Ident (":" Ident)?              # supports animate:onMount
Value          ::= Ident | Literal | Interpolation | FnCall
UiModifierBlock ::= "{" UiModifierContent "}"
UiModifierContent ::= (ModifierItem ("," ModifierItem)*)?
ModifierValue  ::= Literal | Ident | Interpolation | AnimationExpr

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
