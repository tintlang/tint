# TextArea, Preview and program-level tools

These are the pieces the Tint site and sandbox use to be written entirely in
`.tn` (no HTML, JS or CSS files).

## `TextArea`

An editable multi-line text field. Its string child is the value.

```tn
TextArea {
    font-family::mono
    tab-size::2
    input||on_edit
    submit||run
    "{code}"
}
```

- `input||handler` — called with the new text (a `String`) on every edit.
- `submit||handler` — called with no arguments on Ctrl/Cmd+Enter.
- Tab inserts two spaces instead of moving focus.
- The value is only written to the DOM when it differs, so the caret stays put
  while the state round-trips.

A syntax-highlighted editor is a transparent `TextArea` over a layer of
coloured `Text` nodes; see `sandbox/src/editor/editor.tn`.

## Text natives

| function | result |
| --- | --- |
| `tint_highlight(src)` | `[[text, class], …]` covering `src` exactly, produced by the real Tint lexer. Classes: `plain`, `keyword`, `atom`, `string`, `number`, `type`, `fn`, `punct`, `op`, `opkw`, `comment` |
| `line_count(src)` | number of lines |
| `max_line_len(src)` | length of the longest line |
| `line_numbers(src)` | `"1\n2\n…"` for a gutter |

## `Preview`

Mounts a nested Tint session from source text.

```tn
Preview { entry||"App" "{code}" }
```

The child string is Tint source; `entry||` names the `ui fn` to mount (default
`App`). The preview re-renders when the source changes and keeps its state
between edits. A compile error is shown inside the preview. A nested session
has its own state and handlers; it does not receive `key_down||` events.

## Persistence

`storage_get(key)`, `storage_get_or(key, default)`, `storage_set(key, value)`
and `storage_remove(key)` are backed by `localStorage` (prefix `tint:`) in the
browser. Storage is loaded before `state` is initialised, so
`state theme = storage_get_or("theme", "dark")` survives reloads.

## `include_str`

`include_str("relative/path")` is replaced at build time by the file's text as
a string literal (braces are escaped, so its content is never interpolated):

```tn
state code = include_str("./demo-app.tn")
```

In string literals, `\{` and `\}` write literal braces.
