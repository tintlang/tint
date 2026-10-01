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

Keyframes are declared in the `app` block (items are comma separated; `from`, `to` and `pNN` for NN%, `p33_5` for 33.5%):

```tn
app {
    keyframes.pulse::{
        from::{ opacity::1 },
        p50::{ opacity::0.4 },
        to::{ opacity::1 }
    }
}
// then: animation::"pulse 1.2s ease-in-out infinite"
``` `key::...`
is significant: the DOM renderer keeps that node alive across rerenders, so
the browser does not restart its animation every time game state changes.


## Spring

`spring::{ stiffness::170, damping::26, mass::1, props::"transform, opacity" }` is a transition with spring physics. The spring is simulated once when the style is resolved and handed to the browser as a CSS `linear(...)` easing, so there is no JavaScript per frame. A low `damping` overshoots and wobbles; `props` defaults to `all`.

```tn
Button {
    motion::{
        spring::{ stiffness::400, damping::14, props::"transform" },
        hover::{ scale::1.08 },
        tap::{ scale::0.9 }
    }
}
```

Needs a browser with CSS `linear()` easing (Chrome/Edge 113, Firefox 112, Safari 17.2).

## Gestures

- **hover** and **tap**: `hover::{...}` and `tap::{...}` style blocks (`tap` is the pressed state); `tap||handler` is a click handler.
- **drag**: `drag::both`, `drag::x`, `drag::y`, or `drag::{ both, back }`. The node follows the pointer (set as the CSS `translate` property; no Tint state is updated per move) and stays where it is dropped; `back` returns it to its place on release, animated if the node has a `spring` with `props::"translate"`. React to it with `pointer_up||`.
