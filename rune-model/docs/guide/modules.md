# Modules & Imports

RuneLang uses the filesystem itself as the module system — no `crate::`, no
`mod.rs`, no relative imports.

## Core rules

- Each file `name.rn` is a module named `name`.
- Each directory is a namespace, but only becomes an importable module once it
  contains a `run.rn`.
- One file = one module.
- Imports are absolute: `use ui::panel`. The `.rn` extension is never written.
- Relative imports (`./`, `../`) are forbidden.
- File names must be snake_case, never PascalCase.
- Multi-file "module root" patterns like `mod.rs` or `foo/mod.rn` don't exist here.
- `use` is Logic Mode only — it cannot appear inside a `ui fn`.
- Everything resolves statically at compile time; there are no dynamic or lazy imports.

## Declaring a module

Every file starts with a `module` declaration:

```
// math.rn
module math

fn add(a: i32, b: i32) -> i32 { a + b }

export fn add
```

`module math` declares the namespace; it does not open a scope and takes no `{}`.

## Directories as namespaces

```
src/
    run.rn
    math/
        run.rn
        vec.rn
        matrix.rn
    ui/
        run.rn
        panel.rn
        button.rn
```

`math/vec.rn` → module `math.vec`; `ui/panel.rn` → module `ui.panel`. The project
root is `src/run.rn`.

## `run.rn` — a directory's public API

`run.rn` plays the role of `index.ts` or `mod.rs`, but simpler: it can hand-pick
exports, or auto-collect the whole directory.

**Manual:**

```
module ui
export mod panel
export mod button
export mod icon
```

**Automatic:**

```
module ui
export auto
```

`export auto` finds every `.rn` file in the directory (except `run.rn` itself),
imports each as a submodule, and re-exports every `export`ed symbol from it into the
directory's namespace. Two files exporting the same name is a compile error:

```
ERROR: symbol 'Panel' exported by ui/panel.rn and ui/container.rn
```

You can mix manual and automatic exports in one `run.rn` — manual entries are
applied first, then `export auto` fills in the rest:

```
export mod special;   // handled explicitly
export auto;          // everything else, automatically
```

A file with no `export` in it at all stays private to the directory, even under
`export auto`.

## Exporting symbols

```
export fn add
export struct Vec2
export enum Mode
export ui fn Panel
export mod vec
```

Only the name is exported — the body isn't duplicated, and by default everything in
a file is private.

## Importing — `use`

```
use math::vec
use ui::panel
use gfx::shaders::blur
use math::vec as v            // alias
```

```
let d = math::vec::dot(a, b)
let p = ui::panel::Panel()
let d2 = v::len(a)            // via the alias
```

Forbidden:

```
use ./local
use ../common
use ui/panel
use ui::panel.rn
```

`use` inside a `ui fn` is always an error:

```
ui fn App() {
    use math::vec      // error
}
```

## What can be imported

Functions, `ui fn`, structs, enums, submodules, type aliases, `const`, `static`.
**Not** importable: UI nodes, `theme` fields, `runestyle`, or `shader` blocks (all
of those stay file-local).

## Entry point

```
// src/run.rn
module app
use ui::App
run(App)
```

## Full example project

```
src/
    run.rn
    math/
        run.rn
        vec.rn
        matrix.rn
    ui/
        run.rn
        app.rn
        panel.rn
        button.rn
```

```
--- src/run.rn ---
module app
use ui::App
run(App)

--- math/run.rn ---
module math
export auto

--- math/vec.rn ---
module math.vec

struct Vec2 { x: f32, y: f32 }
fn len(v: Vec2) -> f32 { sqrt(v.x*v.x + v.y*v.y) }

export struct Vec2
export fn len

--- ui/run.rn ---
module ui
export auto

--- ui/panel.rn ---
module ui.panel

ui fn Panel(children: UIChildren) {
    <PanelContainer padding{12}>
        <Children />
    </PanelContainer>
}

export ui fn Panel

--- ui/app.rn ---
module ui.app
use ui::panel::Panel

ui fn App() {
    <Panel>
        <Text>"Hello Rune"</Text>
    </Panel>
}

export ui fn App
```

## Compared to Rust

| | Rune | Rust |
|---|---|---|
| Directory root file | `run.rn` | `mod.rs` |
| Auto-export | yes | no |
| Export keyword | `export` | `pub` |
| `crate::` prefix | not needed | needed |
| UI modules built in | yes | no |
| Path separator | `::` | `::` |

Rune's module system trades some of Rust's flexibility for predictability: no
`crate`/`super`, no `mod.rs` indirection, and `run.rn` doubling as both the
namespace root and (optionally) an automatic module collector.
