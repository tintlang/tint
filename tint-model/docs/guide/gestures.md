# Pointer gestures

Three handler attributes on any node:

| attribute | handler is called |
|---|---|
| `pan||h` | `h(dx, dy)` on every move of a press that started on the node (px from where it started) |
| `swipe||h` | `h(dir)` on release after a quick move of 40 px or more: `"left"`, `"right"`, `"up"`, `"down"` |
| `long_press||h` | `h()` after the pointer rested on the node for 500 ms |

```tn
fn panned(dx: number, dy: number) { offset = dx }
fn swiped(dir: string) { if dir == "left" { next() } }
fn held() { menu = true }

ui fn App() {
    Card { pan||panned swipe||swiped long_press||held touch-action::none "..." }
}
```

On touch screens give the node `touch-action::none` (or `pan-y` / `pan-x` to keep scrolling in one direction), or the
browser takes the gesture for scrolling. Moving more than 8 px cancels a long press. For dragging a node around see
`drag` in [Animations](../ui/animations.md).

## Hotkeys

```tint
Button { hotkey||open_search keys||"mod+k" "Search" }
Button { hotkey||close keys||"esc, ctrl+w" "Close" }
```

`keys` is a combo or a comma-separated list. `mod` is Cmd on macOS and Ctrl elsewhere; `shift`, `alt`, `ctrl`, `meta` work too. The handler is a `fn` with no arguments. A hotkey is ignored while the user types in an input, unless it has a modifier or is `esc`.

## URL parameters

`query_get("name")` returns the query parameter as a string (`""` if missing). `query_set("name", value)` updates it with `history.replaceState`, with no reload and no new history entry.

```tint
state tab = query_get("tab")
fn pick() { query_set("tab", "b"); tab = "b" }
```

## Derived values

`derived total = price * qty` (in a `ui fn` or a `component`, next to `state`) is a name for an expression. It is recomputed on every use, has no storage of its own, and cannot be assigned.

## blur, scroll and sortable lists

`blur||h` runs `h()` when focus leaves the node (or something inside it).

`scroll||h` runs `h(top, height)` when the node scrolls: its `scrollTop` and visible height in px.
Together with `slice` this makes a **virtual list**: render only the rows in view and stand in for
the rest with two spacers (they must not shrink, hence `flex-shrink::0`).

```tint
state top = 0
state rows = []                       // thousands of strings
derived first = ((top - top % 32) / 32) as i32
derived rest  = rows.len() * 32 - top - 256
derived vis   = rows.slice(first, first + 8)
fn on_scroll(t: number, h: number) { top = t }

Column {
    scroll||on_scroll layout::{ height::200 } overflow::auto
    Row { height::{ top + 0 } flex-shrink::0 }
    Column { for{ r in vis } Row { layout::{ height::32 } "{r}" } }
    Row { height::{ rest + 0 } flex-shrink::0 }
}
```

`sortable||h` on a container lets the user drag its children into a new order (vertical in a
column, horizontal in a row). On release the host calls `h(from, to)` with the child's old and new
index; the app moves the item in its list. Children should have a `key::` so the DOM follows the move.
A press on a button, link or field inside a child does not start a drag.

```tint
fn reorder(from: number, to: number) { /* move items[from] to index `to` */ }
Column { sortable||reorder for{ it in items } Row { key::it "{it}" } }
```

## persist

`persist state name = "x"` is a string state kept in storage under its name: it starts from the
stored value (`x` when there is none) and every assignment to it in a handler is saved.
Other types: use `storage_get_or` / `storage_set` yourself.
