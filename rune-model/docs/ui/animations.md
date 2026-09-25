# RuneAnimations

A pure, declarative, type-safe animation system, using the same RuneStyle `{}`
modifiers as everything else visual, running entirely on the GPU.

Built from: `animate{ ... }` (the transition itself), `duration{...}` /
`speed{...}` (timing), `curve{...}` (the interpolation curve), and triggers
(`animate:onMount{}`, `animate:onChange(state){}`).

## Basic syntax

```
<Panel
    animate{ opacity: 0 -> 1 }
    duration{300ms}
>
    <Text> "Hello Rune" </Text>
</Panel>
```

Rune interpolates the values automatically.

## Multiple properties

```
animate{
    opacity: 0 -> 1,
    scale: 0.9 -> 1.0
}
```

## Speed instead of duration

```
animate{ opacity: 0 -> 1 }
speed{fast}
```

```
speed{slow}      // 600ms
speed{normal}    // 300ms
speed{fast}      // 150ms
speed{instant}   // 0ms
```

## Mount and change triggers

```
<Panel
    animate:onMount = { opacity: 0 -> 1, y: 12px -> 0px }
    speed{normal}
>

<Panel
    animate:onChange(count) = { scale: 1 -> 1.1 }
    duration{120ms}
>
    <Text> "{count}" </Text>
</Panel>
```

## Looping

```
animate{ rotate: 0deg -> 360deg }
speed{slow}
loop=true
```

`loop=true` is a logical attribute (`=`), not a visual one — it isn't a modifier.

## Semantic value tokens

```
animate{ opacity: hidden -> visible }   // same as: opacity: 0 -> 1
```

Available tokens: `visible, hidden, grow, shrink, enter, exit`.

## Easing / curves

```
animate{ y: 20px -> 0px }
curve{ease-out}
duration{250ms}
```

Supported: `linear, ease-in, ease-out, ease-in-out, spring`.

## Animating effects

```
<Panel
    effect=shadow{0px 2px 12px black{10%}}
    animate{ shadow: soft -> strong, opacity: 0.5 -> 1 }
    duration{300ms}
>
```

## Full example

```
<Panel
    padding{20}
    radius{12px}
    animate{
        opacity: 0 -> 1,
        y: 12px -> 0px
    }
    speed{normal}
>
    <Text style=text{xl, bold}>
        "Animated Card"
    </Text>

    <Button
        onClick=increment
        animate:onChange(count) = { scale: 1 -> 1.15 }
        duration{120ms}
    >
        "+"
    </Button>
</Panel>
```

## Timelines (sequenced animations)

```
animate{
    sequence(
        opacity: 0 -> 1 (300ms),
        scale: 0.9 -> 1 (150ms),
        glow{blue-400{40%}} (200ms)
    )
}
```

Equivalent per-property timing form:

```
animate{
    opacity{ from: 0 to: 1 duration: 300ms }
    scale{ from: 0.9 to: 1 duration: 150ms }
    glow{ color: blue-400 strength: 40% duration: 200ms }
}
```

Without explicit timing (falls back to `speed{}`):

```
animate{
    sequence(
        opacity: 0 -> 1,
        scale: 0.9 -> 1,
        blur{20px}
    )
}
speed{normal}
```

## Principles

No imperative animation code, no JavaScript, no CSS keyframes — a single
declarative `{}` syntax, GPU-native interpolation, transitions driven purely by
state, and a UI that stays predictable and deterministic throughout.

---

## Animatable properties, triggers, and frame dynamics

This section (from the extended modifier spec) covers what can be animated and how,
in more depth than the examples above.

### Animation format

```
animate{
    opacity: 0 -> 1
    scale: 0.9 -> 1
    x: 0px -> 12px
    background{ gradient.linear{ ... } }   // an animatable layer
}
```

Each property is an independent channel, animated in parallel.

### What can be animated

Visual: `opacity`, `radius` (any axis), `shadow` (blur, spread, color),
`background` (solid or gradient), `fill`, `stroke`.

Transforms: `scale`, `rotate`, `x`, `y` (offset), `translate.x`, `translate.y`.

Layout properties are **not** animatable — layout is not recomputed mid-animation,
so animating `width`, `height`, `padding`, or `margin` is an error.

### Transition form, `<from> -> <to>`

```
opacity: 0 -> 1
scale: 0.95 -> 1
x: -10px -> 0px
radius.t{2px -> 12px}
shadow{0px 0px 0px black{0%} -> 0px 8px 20px black{25%}}
```

Gradients animate too, either a single stop:

```
background{
    gradient.linear{
        from{blue-400 -> blue-600}
        to{blue-900 -> black}
    }
}
```

or the whole layer:

```
background{
    gradient.radial{
        radius{40px -> 120px}
        center.x{0.5 -> 0.6}
    }
}
```

### Triggers — `effect:onX{}`

```
effect:onHover{
    scale: 1 -> 1.05
    opacity: 0.9 -> 1
}
```

Available triggers: `onHover / onLeave`, `onPress / onRelease`, `onFocus / onBlur`,
`onMount / onDestroy`, `onChange(state)`.

### Multiple independent animations

```
animate{ opacity: 0 -> 1 }
animate:onHover{ scale: 1 -> 1.05 }
animate:onChange{ x: 0px -> 20px }
```

Re-triggering a channel overwrites only that channel.

### Timeline: duration / delay / easing

```
animate{
    duration{250ms}
    delay{20ms}
    easing{ease-in-out}
    opacity: 0 -> 1
}
```

Supported easings: `linear, ease-in, ease-out, ease-in-out`, and optionally
`spring{tension, damping}`. The timeline settings apply to every property in the
block.

### Errors

```
animate{ margin: 0 -> 10 }         // layout properties can't animate
animate{ width: 100 -> 200 }        // layout-only, forbidden
animate{ color{"blue" -> "red"} }    // no strings
animate{ opacity: 0 -> foo }          // no variables
animate{ x: 10px -> 10+5 }             // no expressions
animate=...                             // animate is a {} block, not a = attribute
```

### Frame-based physics (velocity / accel / decel / damping)

These extend the usual duration+easing model with per-frame physical simulation,
inside `animate{}` only:

```
velocity{0.0 -> 0.12}     // initial -> target velocity, added each frame
accel{0.005}                // positive acceleration, added to velocity each frame
decel{0.01}                  // negative acceleration, subtracted each frame
damping{0.90}                  // multiplicative decay per frame (0..1]
                                 // 0.90 = fast decay, 0.98 = gentle, near-lossless
```

Per-frame update, conceptually:

```
v += accel
v -= decel
v *= damping
value += v * frameDelta     // frameDelta keeps this framerate-independent
```

Combined with a normal transition:

```
animate{
    duration{300ms}
    easing{ease-out}

    opacity: 0 -> 1
    scale: 0.9 -> 1

    velocity{0.0 -> 0.04}
    accel{0.002}
    damping{0.92}
}
```

Hover example:

```
effect:onHover{
    scale: 1 -> 1.10
    velocity{0.0 -> 0.02}
    accel{0.005}
    damping{0.90}
}
```

Scroll-inertia example:

```
animate:onChange(scroll){
    x: old -> new
    velocity{scrollVelocity}
    decel{0.004}
    damping{0.96}
}
```

Errors: `velocity{"fast"}`, `accel{a + b}`, `damping{1.2}` (out of range),
`velocity{10px -> 20px}` (velocity is a number, not a length), `accel{50%}`.
