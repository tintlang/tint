# Core Model: Logic Mode vs UI Mode

Every rule in the language reference follows from one split. RuneLang has exactly
two modes, and they never mix:

- **Logic Mode** — procedural, computational, control-flow code
- **UI Mode** — declarative description of interface, visual parameters, and behavior

## Logic Mode

Used for algorithms, conditionals, loops, reactivity (`state`, `signal`, `computed`,
`watch`), data structures, function calls, and low-level work (resources, the
`borrow` model, matrices, the shader DSL).

Allowed: `fn`, `let`, `const`, `static`, `if`/`else if`/`else`, `match`,
`for`/`while`/`loop`, `return`, `state`/`signal`/`computed`/`watch`, `try`/`throws`,
`protocol`/`impl`, `borrow`/`borrow immut`, the matrix DSL, `shader { ... }`.

Forbidden: any UI modifier or UI construct — `padding{}`, `margin{}`, `width{}`,
`height{}`, `color{}`, `shadow{}`, `animate{}`, `onHover{}`, etc.

Reason: Logic Mode is meant to be a pure computational layer, independent of the UI.

## UI Mode

Used inside `<Component>` tags, inside `ui fn`, and inside `rune2d { ... }`. Its job
is to describe appearance, visual behavior, animation, and reaction to user events —
nothing else.

Allowed: `<Component>...</Component>`, `padding{}`, `radius{}`, `width{}`,
`height{}`, `color{}`, `text{}`, `shadow{}`, `animate{}`, `transition{}`,
`onClick{}`, `onHover{}`, `onInput{}`, `effect:onX{}`, the slot system
(`<Children/>`, `<HeaderSlot/>`, ...).

Forbidden: `let`, `const`, `static`, `if`, `match`, `for`, `while`, `loop`, `return`,
`try`, `throws`, `protocol`/`impl`, any resource operation (`buffer`, `image`,
`borrow`), the matrix DSL, the shader DSL.

Reason: UI Mode must stay a pure declaration of the interface. Logic never mixes
into presentation.

## The boundary is enforced

```
fn f() {
    padding{20}       // error: UI modifier in Logic Mode
}

<Panel>
    let x = 10         // error: logic construct in UI Mode
</Panel>
```

## How the mode is decided

The mode is purely contextual:

- everything outside `<UI tags>` and outside `ui fn` is Logic Mode
- everything inside `<...>` is UI Mode
- the body of a `ui fn` is mixed: its top level is Logic Mode, its markup (`<...>`) is UI Mode

```
ui fn ButtonCounter() {
    state count = 0          // Logic Mode

    fn inc() { count += 1 }  // Logic Mode

    <Button onClick=inc>     // UI Mode
        <Text>{count}</Text>
    </Button>
}
```

`ui fn` is the only place where both modes coexist — but even there, they never mix
inside the same block:

```
ui fn Example() {
    <Panel>
        if n > 0 { ... }     // error: logic inside UI Mode
    </Panel>
}
```

## One-way dependency

Logic Mode can drive UI Mode (through parameters, `state`, `computed`). UI Mode can
never drive Logic Mode back, and cannot create variables or mutate state directly:

```
Logic Mode  →  UI Mode      (allowed)
UI Mode     →  Logic Mode   (never)
```

This keeps the UI fully deterministic: it can't be mutated from inside itself, and
Logic Mode can't accidentally build UI structure.

## Formal summary

```
Logic Mode = { fn, let, const, static, if, match, for, while, loop,
                state, signal, computed, watch,
                try, throws, protocol, impl,
                borrow, borrow immut, matrix, shader }

UI Mode = { <UI>, </UI>, modifiers{}, animate{}, effect:onX{}, color{}, text{},
             layout{}, padding{}, margin{}, radius{}, width{}, height{},
             event handlers {...}, slots, Children }

Logic Mode ∩ UI Mode = ∅
```

1. Logic Mode may contain no UI.
2. UI Mode may contain no logic.
3. `ui fn` is the only point where both contexts meet — never where they mix.
4. UI is a pure, declarative view over data; Logic is the sole source of computation.
5. The boundary is enforced by the compiler, not by convention.
