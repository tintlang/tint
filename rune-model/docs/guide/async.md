# RuneAsync — Asynchronous Model

RuneAsync is closer to Rust futures than JS promises: typed, cancellable, and never
allowed to block the UI.

## Principles

1. `async` is available only in Logic Mode.
2. `await` is only valid inside an `async fn`.
3. UI Mode itself can never be async.
4. `ui fn` can trigger async work through events (`onClick=...`).
5. An `async fn` always returns `Future<T>` (the signature just says `T`).
6. `cancel()`, `timeout()`, `race()` are built into the runtime.
7. Errors follow Rust's `Result<T, E>` + `throws`.
8. The UI is never blocked.
9. Each `ui fn` has its own event loop.

## Defining and calling

```
async fn loadUser(id: number) -> User {
    let raw = await fetch("/user/" + id)
    return parseUser(raw)
}
```

`await` is only legal inside an `async fn`:

```
async fn loadAll() {
    let a = await loadUser(1)
    let b = await loadUser(2)
    return [a, b]
}

fn f() {
    let x = await load()    // error
}
```

## Calling async from the UI

```
<Button onClick=refresh />

async fn refresh() {
    data = await loadUser(id)
}
```

UI can trigger async work; async code can't change UI syntax; when it mutates
`state`/`signal`, the UI re-renders automatically.

## Errors: `throws` and `try`

```
async fn load() throws -> User {
    let raw = await try fetch()
    return try parse(raw)
}

async fn safe() {
    let u = try await load()   // Result propagation
}
```

## Cancellation

Every `await` is a cancellation point. Starting an async call again through the same
event automatically cancels the previous future; `cancel(task)` also works
explicitly, and cancellation is always soft (no panic, no UI change):

```
let task = start refresh()
cancel(task)
```

## Timeout and race

```
async fn load() {
    let data = await timeout(fetchData(), 2000ms)   // TimeoutError if exceeded
}

async fn fetchFastest() {
    let a = fetchA()
    let b = fetchB()
    return await race(a, b)     // first to finish wins; the other is cancelled
}
```

## Parallel and fire-and-forget

```
let (a, b) = await parallel(loadA(), loadB())   // run together, wait for both

let t = start refresh()                          // fire-and-forget
cancel(t)                                         // returns a TaskHandle
```

## Async + state

```
async fn loadProfile() {
    loading = true
    profile = await fetchProfile()
    loading = false
}
```

The UI reacts automatically to the `state` changes.

## Restrictions

Not allowed: `async` inside `ui fn` itself, `await` inside `<Block>`, passing a
future as a UI attribute, storing a `TaskHandle` in UI.

```
<Button onClick=loadProfile />       // correct

<Panel data=await load()>            // wrong
<Text>{await load()}</Text>          // wrong
<Block if=await load()>              // wrong
<Image source=task>                  // wrong
```

## Event loop

Each `ui fn` owns an independent event loop: async functions run in Logic Mode, the
UI re-renders after a `state` change, `await` never blocks the UI, and destroying
the component (`onDestroy`) cancels its in-flight async work.

## Types

`Future<T>`, `TaskHandle`, `CancelledError`, `TimeoutError` — all Logic Mode only;
UI Mode never sees them.

## Full example

```
ui fn UserCard() {
    state loading = false
    state user: User? = null

    fn reload() { start load() }

    async fn load() {
        loading = true
        user = await timeout(fetch("/user/1"), 2000ms)
        loading = false
    }

    <Panel padding{20} radius{12px}>
        <Button onClick=reload>"Reload"</Button>

        <Block if=loading>
            <Text>"Loading…"</Text>
        </Block>

        <Block if=(!loading && user != null)>
            <Text>"Name: {user.name}"</Text>
        </Block>
    </Panel>
}
```
