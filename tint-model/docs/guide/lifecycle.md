# Life cycle and resources

A node can run a handler when it appears on the page, when it leaves it, and when a value it
depends on changes. They are ordinary attributes; the handlers are ordinary `fn`s.

```tn
fn note_in() { log = "{log}in " }
fn note_out() { log = "{log}out " }

ui fn App() {
    state show = true
    state log = ""
    Panel { mount||note_in unmount||note_out if{show} "hello" }
}
```

| attribute | runs the handler |
|---|---|
| `mount||h` | once, when the node is added to the page |
| `unmount||h` | once, when it is removed (after its `exit` animation, if it has one) |
| `effect||h deps||{value}` | on mount, and again whenever `value` changes |

A node that only moves (a keyed list that reorders) is neither mounted nor unmounted. A handler
that sets state re-renders as usual; an `effect` that always changes its own `deps` loops, as in
any framework.

On a `component` the attributes go on the component's element, and `h` is one of its own `fn`s
(so every use runs its own copy).

## `resource`

`resource` is a line in a `component` for a value that arrives later from a host function:

```tn
ui fn App() {
    component Notes(url: string) {
        resource text = fetch_text(url)
        Label { if{text_loading} "loading..." }
        Label { "{text}" }
        Label { if{text_error != ""} "failed: {text_error}" }
    }
    Notes { url::"/notes.txt" }
}
```

It declares the state `text` (empty until the answer comes), `text_loading` (`true` until the
first answer, and while a refetch runs) and `text_error` (`""`, or the message of the `Err`), plus
`fn text_load()`. The component runs `text_load()` when it mounts; call it from a handler to refetch,
or from `effect||text_load deps||{url}` to refetch when a prop changes. The call is any host function
that takes a trailing callback (`fetch_text`, `post_text`, your own `js::` function); its answer is a
`string` unless you write `resource n: number = ...` (or `bool`).

## Cache and polling for `resource`

```tint
resource users = fetch_text("/api/users") cache::30000 every::5000
```

`cache::ms` keeps the (string) result for the life of the page under the name and the first
argument: a component that mounts later shows it at once and asks again only if it is older than
`ms` (`cache::0`: always asks, but shows the old value meanwhile). `every::ms` asks again on that
interval while the component is on the page, ignoring the cache age. `poll||h every||ms` does the
same for any handler on any node.
