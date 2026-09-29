# Collections and strings

Lists, strings, and maps have built-in methods. Methods that change a list or
map (`push`, `pop`, `remove`, `set`) write the result back to the variable they
were called on, so declare it with `let mut`.

```tn
fn main() {
    let mut todos = ["  buy milk ", "write code", "  "]
    todos.push("ship it")
    let clean = todos.map(|t| t.trim()).filter(|t| !t.is_empty())
    let text = clean.join(", ")
    println("{clean.len()} todos: {text}")
}
```

## List

| Method | Result |
| --- | --- |
| `len()` / `is_empty()` | number / bool |
| `push(x)` | appends, returns `()` |
| `pop()` | removes the last item, returns `Option` |
| `remove(i)` | removes and returns item `i` (error if out of range) |
| `contains(x)` | bool |
| `join(sep)` | string; items are joined by their display text |
| `map(f)` / `filter(f)` | new list; `f` is a lambda or named function |
| `find(f)` | first item where `f` is true, as an `Option` |
| `sort()` | new list, ascending; all numbers or all strings |
| `reverse()` | new list, reversed |
| `slice(start)` / `slice(start, end)` | new list of `[start, end)`, clamped to the length |

## String

`len()` (characters), `is_empty()`, `slice(start)` / `slice(start, end)`
(characters, same rules as lists), `trim()`, `to_upper()`, `to_lower()`,
`contains(s)`, `starts_with(s)`, `ends_with(s)`, `split(sep)` (list of strings;
an empty separator splits into characters), `replace(from, to)`.

## Map

`len()`, `is_empty()`, `has(key)`, `get(key)` (`Option`), `set(key, value)`,
`remove(key)` (`Option`), `keys()`, `values()`.

Maps are keyed by string (other keys are converted to their display text).
`keys()` and `values()` come back sorted by key so output is stable.

## Notes

- Only `push`, `pop`, `remove` and `set` change their receiver. `sort`,
  `reverse`, `slice`, `map` and `filter` return a new value, so write
  `xs = xs.sort()` to keep the result.
- A string literal is fine inside a `{...}` interpolation:
  `"{xs.join(", ")}"`.
- `+` between a string and a number or bool appends its text
  (`"n = " + n`); lists and structs are rejected by `tint check`.
- There is no `index_of`, `sort_by`, `concat` or `reduce` yet.
