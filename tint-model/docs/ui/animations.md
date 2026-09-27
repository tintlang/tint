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

For time-independent sprite/UI animation, Tint also forwards CSS animation
properties through the same motion/style resolver:

```tn
Sprite {
    key::"player"
    motion::{
        animation::"walk-cycle 120ms steps(3) infinite",
        animation-play-state::running
    }
}
```

The keyframes are authored by the renderer stylesheet -- there is no Tint
keyframe block yet; that remains a future idea, not current syntax. `key::...`
is significant: the DOM renderer keeps that node alive across rerenders, so
the browser does not restart its animation every time game state changes.
