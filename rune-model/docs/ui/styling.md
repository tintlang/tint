# RuneStyle & RuneColor

A type-safe, GPU-first styling system. Every visual parameter goes through a `{}`
modifier — there is no CSS and no `=` form for visual properties.

```
✔ all visual parameters → {}
✔ all logical parameters → =
✔ no visual props via =
✔ effects are ordinary modifiers, no separate `effect=` syntax
```

## Colors — RuneColor

```
color{#fff}
color{#112233}
color{#fff{40%}}          // hex + opacity

color{blue-400}            // palette token
color{gray-700}
color{gray-400{50%}}       // palette token + opacity
color{white-900{80%}}
```

## Text style

```
text{xl}
text{24px, bold}
text{lg, italic}
text{5xl, bold, tracking-tight}
```

## Radius

```
radius{12px}
radius{full}
radius{4px 8px 8px 4px}
```

## Common visual modifiers

```
opacity{80%}
rotate{15deg}
scale{1.1}
size{48px}
size{24px 64px}
spacing{12}
padding{16}
margin{auto}
align{center}
gap{20}
width{100%}
height{240px}
```

## GPU effects

**Single effects:**

```
shadow{4px 8px 20px black{30%}}
inner{0px 0px 12px blue-400{40%}}
blur{20px}
glow{blue-500{60%}}
```

**Combo effects (chained with `+`):**

```
shadow+inner{0px 2px 8px black{20%}}
shadow+glow{10px blue-400{40%}}
shadow+inner+glow{6px black{25%}}
```

```
combo_effect  = effect_chain "{" args "}"
effect_chain  = identifier ("+" identifier)*
identifier    = shadow | inner | glow | blur | opacity
```

### How arguments are distributed across a combo

The group of arguments inside `{}` is divided among the chained effects, left to
right:

```
shadow+inner{A}              → shadow{A}, inner{A}                # one arg group → applies to all
shadow+inner{A B}             → shadow{A}, inner{B}                 # one group per effect
shadow+inner+glow{A B C}       → shadow{A}, inner{B}, glow{C}
shadow+inner+glow{A}            → shadow{A}, inner{A}, glow{A}       # fewer groups → last one repeats
shadow{A B C}                    → error: shadow only takes 1 argument group
```

Examples:

```
shadow+inner{0px 2px 12px black{18%}}
→ shadow{0px 2px 12px black{18%}}
→ inner{0px 2px 12px black{18%}}

shadow+inner{
    4px 8px 24px black{25%}
    0px 2px 12px black{18%}
}

shadow+inner+glow{
    0px 2px 8px black{20%}
    0px 0px 6px black{15%}
    8px purple-300{40%}
}

shadow+inner{soft strong}        // semantic tokens work too
→ shadow{soft}
→ inner{strong}
```

```
<Panel
    shadow+inner{
        4px 8px 24px black{25%}
        0px 2px 12px black{18%}
    },
    glow{blue-400{40%}}
>
    <Text> "Rune 2.0 UI Effects" </Text>
</Panel>
```

This gives compact, readable, strictly-parseable effect chains — combinations that
plain CSS/Tailwind can't express declaratively.

## Combining with animation

```
animate{opacity: 0 -> 1}
onHover{scale: 1 -> 1.05}
onLeave{blur{20px}}

onHover{shadow+inner{0px 4px 12px black{30%}}, scale: 1 -> 1.1}   // combined transition
```

Full animation syntax: `ui/animations.md`.

## Example: a styled panel

```
<Panel
    padding{20}
    radius{12px}
    color{gray-900{20%}}
    shadow{0px 2px 8px black{20%}}
>
    <Text text{20, bold}>
        "Card"
    </Text>
</Panel>
```

## Principles

Everything visual goes through `{}`; nothing visual goes through `=`; effects are a
plain visual DSL; the system composes cleanly with `RuneAnimations` and `Rune2D`;
there is no CSS or HTML underneath any of it.
