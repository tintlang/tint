# RuneLogic: Control Flow

All of this is Logic Mode only — UI Mode uses `<Block if=...>` / `<Block for=...>` /
`<Block match=...>` instead (see `ui/blocks.md`).

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
    Err(e)    => log{e},
}
```

## For

```
for i in 0..10 { print{i}; }                       // range
for user in users { log{user.name}; }               // collection
for item in items where item.isActive { process(item); }  // filtered
```

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
    log{i};
}
```

## Error handling

```
match fetch() {
    Ok(value) => process(value),
    Err(err)  => log{err},
}

let res = fetch();
if res.isErr() {
    error{"Network failed"};
}
```

## A note on `{}` vs `()` in this context

Built-in VM/debug helpers like `log{}`, `print{}`, `debug{}`, `error{}` use `{}`;
ordinary user-defined function calls use `()`. Mixing them up is the most common
mistake here:

```
log("text")          // wrong — VM helpers use {}
increment{5}           // wrong — logic functions use ()
shadow(4px)             // wrong — that's a UI modifier, it uses {}
animate( ... )           // wrong — animate is a {} block, not a call
```

## Full example

```
fn renderItems() {
    for item in items {
        if item.enabled {
            showItem(item);
        } else {
            debug{"Skipped disabled item"};
        }
    }
}
```
