# UI motion

Tint currently exposes browser transitions and hover styles through the
`motion` group. The runtime resolves the base properties and the hover
properties separately; the browser performs the interpolation.

```tn
Button {
    paint::{ background::#6c5ce7, radius::full }
    motion::{
        transition::"transform .2s ease, background-color .2s ease",
        hover::{ scale::1.06, background::#8b7cf0 }
    }
}
```

`scale::1.06` is a typed transform shorthand. Other currently supported hover
values include `background`, `color`, `filter`, `shadow`, `text-decoration`,
and raw `transform` strings.

Animation timing is expressed in the CSS transition string. There is no
separate animation DSL in the current syntax.
