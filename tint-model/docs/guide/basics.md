# TintLogic: Basics

## Variables

Declared with `let`; `mut` is the only way to make one mutable.

```
let x: i32 = 10;
let mut count: i32 = 0;

count = count + 1;
count += 1;              // shorthand
```

`=` is for assignment, `:` for types. Neither `{}` nor `()` belongs in this kind of
statement.

## Type inference

Tint infers types when they're obvious:

```
let x = 10;              // i32
let name = "Tint";       // string
let p = vec2(1, 2);      // vec2, inferred via the vec2() constructor
```

Constructor functions are always called with `()`.

## Literals

```
10          3.14          0.5                          // numbers
"Hello Tint"    "User {name} logged in"                // strings, with {} interpolation
true        false                                       // bool
let nums = [1, 2, 3]                                    // list literal
let user = { name: "Alice", age: 20 }                   // object literal
```

The `{}` in a string interpolation and the `{}` in an object literal are both valid
in Logic Mode, and are unrelated to the UI-Mode modifier `{}` — context always
disambiguates them.

## Expressions

```
let n = (a + b) * c;
let ok = user.isAdult();     // method calls use ()
let pos = vec2(10, 20);      // constructors use ()
```

## Block scope

```
{
    let x = 10;
    // x is visible here
}
// x is gone here
```

## What changed from earlier drafts

These are no longer valid, now that Logic Mode and UI Mode are strictly separated:

```
log{"message"}        // wrong — log is a VM/debug helper, not a modifier here: log("message")
increment{5}           // wrong — a logic function call always uses (): increment(5)
let p = vec2{1,2};      // wrong — constructors use (): vec2(1,2)
let value = shadow{4px}; // wrong — shadow{} is a UI modifier, not a logic value
radius(12)               // wrong — radius::12 is a UI modifier, not a function call
```

The rule that makes all of this consistent: `{}` is for UI modifiers and
interpolation blocks; `()` is for logic function calls and constructors.
