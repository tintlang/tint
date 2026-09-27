# TintLogic: Control Flow

This chapter covers Logic Mode. UI control flow uses `if{...}`, `for{...}`, and
`match{...}` directly on named UI nodes (see `ui/blocks.md`).

## If / else

```
if x > 10 {
    doSomething();
} else {
    doOther();
}
```

## Match

`=>` is used **only** in `match`.

```
match state {
    ready   => showReady(),
    error   => showError(),
    loading => showLoading(),
}

match result {
    Ok(value) => handle(value),
    Err(e)    => log(e),
}
```

## For

```
for i in 0..10 { print(i); }                       // range
```

Logic Mode's `for` only accepts a numeric range (`start..end`); it does not
support iterating an arbitrary collection or a `where` filter clause yet
(`for user in users { ... }` and `for item in items where ... { ... }` are
future ideas, not current syntax -- see `parse_for_stmt` in
`tint-parser/src/parse_stmt/control_flow.rs`). UI Mode's `for{item in
items}` on a UI node is a separate mechanism and does iterate an arbitrary
list already (see `ui/blocks.md`).

## While / loop

```
while x < 10 { x += 1; }

loop {
    tick();
    if done { break; }
}
```

## Break / continue

```
for i in 0..100 {
    if i == 5 { break; }
    if i % 2 == 0 { continue; }
    log(i);
}
```

## Error handling

```
match fetch() {
    Ok(value) => process(value),
    Err(err)  => log(err),
}

let res = fetch();
if res.isErr() {
    error("Network failed");
}
```

## A note on `{}` vs `()` in this context

There is no `{}`-based call syntax anywhere in Logic Mode. `log`, `print`,
`debug`, `dbg`, and `error` are ordinary built-in functions, called exactly
like any other function or constructor: with `()`. `log` is an alias for
`print`; `debug` is an alias for `dbg` (a combined-args dump prefixed
`DBG: `); `error` behaves like `print` but is also written to stderr
natively and prefixed `ERROR: `.

```
log{"text"}             // wrong — every function call uses (): log("text")
increment{5}            // wrong — a logic function call always uses (): increment(5)
shadow(4px)              // wrong — that's a UI modifier, it uses ::
```

## Full example

```
fn renderItems(items, count) {
    for i in 0..count {
        let item = items[i];
        if item.enabled {
            showItem(item);
        } else {
            debug("Skipped disabled item");
        }
    }
}
```

Logic Mode's `for` only ranges over `start..end` (see "For" above), so
looping over a list here means ranging over an index and indexing in --
`for item in items` is not current Logic Mode syntax. That form exists only
as UI Mode's `for{item in items}` modifier (see `ui/blocks.md`), which is a
different mechanism entirely.
