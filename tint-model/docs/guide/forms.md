# Forms

A `form` block inside a `component` declares the fields of a form and expands, at parse
time, into plain state, derived values and handlers:

```tint
component Signup() {
    state sent = 0
    form signup save {
        field email = "" "required, email"
        field pw    = "" "required, min:8"
        field pw2   = "" "same:pw"
    }
    fn save() { sent = sent + 1  signup_done() }

    Column {
        TextArea { input||signup_email_set blur||signup_email_blur "{signup_email}" }
        Label { "{signup_email_msg}" }
        Button { click||signup_submit disabled||{signup_submitting} "Sign up" }
    }
}
```

`form NAME HANDLER { field x = init "rules" ... }`: `HANDLER` is a component `fn` called by
`NAME_submit()` when every field is valid.

Per field `x` of form `f`:

| Name | What |
| --- | --- |
| `f_x` | the value (a string; starts as `init`) |
| `f_x_set(v)` | handler for `input||` |
| `f_x_blur()` | handler for `blur||`; marks the field touched |
| `f_x_touched` | state, `true` after the first blur |
| `f_x_error` | the first broken rule's message, `""` when valid |
| `f_x_msg` | `f_x_error`, but only once the field is touched or the form was submitted |

For the form: `f_valid`, `f_submitted`, `f_submitting`, `f_submit()` (marks it submitted and calls
the handler when valid), `f_done()` (call it when the work ends: clears `f_submitting`) and
`f_reset()`.

Rules (comma separated): `required`, `email`, `min:N`, `max:N` (characters), `same:other_field`.
Messages are fixed English text; for your own, compare `f_x` yourself in a `derived`.
Forms live in components (a form in a bare `ui fn` is a parse error).
