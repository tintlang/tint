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

`drag` also takes `bounds::{ left::-80, right::80, top::0, bottom::0 }` (px from the start position; a side left out is free) and `elastic::0.3` (how much of the overshoot beyond a bound follows the pointer, `0` is a hard wall). On release the node settles on the bound; give it a `spring` on `translate` to animate that.

```tn
Box {
    drag::{ x, bounds::{ left::-60, right::60 }, elastic::0.25 }
    motion::{ spring::{ stiffness::300, damping::22, props::"translate" } }
}
```

## Enter, exit and layout

`enter`, `exit`, `view` and `layout` go inside `motion::{}`. The animation is the node's own `transition` or `spring` (200 ms ease when it has none; `layout` uses 300 ms).

- `enter::{ ... }` -- a new node starts from this style and animates to its own. Also on first render.
- `exit::{ ... }` -- a removed node (an `if{}` that turned false, a `for{}` row that left the list) stays in the page while it animates to this style, ignores the pointer, then goes. Only the node that is removed animates, not its descendants separately; give list rows a `key::`.
- `layout::all` / `layout::position` / `layout::size` -- when a re-render moves or resizes the node it glides from its old box instead of jumping (FLIP: only `translate` and `scale` are animated, so the node's own `transform` is untouched). Siblings of a removed node glide into the freed room once its `exit` is over. The content of a resized node is scaled with it, so text can look stretched while it resizes; use `layout::position` for such nodes.
- `view::{ ... }` -- this style is on while the node is in the viewport (15% visible); the node's own style is the "not in view" state. `view::{ once, ... }` keeps it once reached.

```tn
Box {
    key::"toast"
    motion::{
        spring::{ stiffness::300, damping::24, props::"transform, opacity" },
        enter::{ opacity::0, translate.y::20 },
        exit::{ opacity::0, scale::0.9 },
        layout::position
    }
    if{show}
    "Saved"
}

Box {
    paint::{ opacity::0 }
    motion::{ transition::"opacity .4s ease", view::{ once, opacity::1 } }
}
```

`layout` is also a modifier group (`layout::{ width::.. }`); inside `motion::{}` it is the flag above.

## Shared elements: `layout-id`

Two nodes with the same `layout-id` are one element for the animation: when a render removes one and adds the other,
the new one glides from where the old one was (position and size), like Framer's `layoutId`. A tab underline is the
usual case:

```tn
Tab { click||pick_a "A" Marker { motion::{ layout-id::"underline" } if{sel == 1} } }
Tab { click||pick_b "B" Marker { motion::{ layout-id::"underline" } if{sel == 2} } }
```

The glide takes its duration and easing from the node's `transition` (300 ms when it has none). A node with a `layout-id`
also has `layout::all`.

## Scroll-linked styles: `scroll`, `scroll-page`

```tn
Hero { motion::{ scroll::{ from::{ opacity::1, translate.y::0 }, to::{ opacity::0, translate.y::-80 } } } }
Progress { motion::{ scroll-page::{ from::{ width::0 }, to::{ width::320 } } } }
```

The style is on a line between `from` and `to`, set by scroll: `scroll` is the node's progress through the window (0 as its
top enters at the bottom, 1 as its bottom leaves at the top), `scroll-page` is the page's (0 at the top, 1 at the end).
Numbers are interpolated when both values have the same shape (`translateY(40px)` to `translateY(0px)`); anything else
switches halfway. Do not give the node a `transition` for the same property.

## Stagger

`stagger::80` on a parent delays the children that enter in the same render, one after another (80 ms each):

```tn
List { motion::{ stagger::80 } for{ item in items } Row { motion::{ enter::{ opacity::0, translate.x::-30 }, transition::"all .4s ease" } "{item}" } }
```

## Drag momentum

`drag::{ x, momentum, bounds::{ left::0, right::300 } }`: released while moving, the node keeps going and slows down
(about 5% per frame), stopping at its bounds. `momentum` is ignored with `back`.
