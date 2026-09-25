# UI Modifiers — the `{}` DSL

Every visual parameter in RuneLang — color, geometry, layout, typography, effects,
animation, Rune2D — is written with the same `{}` modifier syntax. This document
defines that syntax's grammar and rules once; semantics for specific modifiers live
in `ui/styling.md` and `ui/animations.md`.

Modifiers `{}` never execute code and never call functions. Logical/behavioral
attributes use `=` instead — see `02-core-model.md`.

```
padding{20}
padding.x{16}
text{bold}
animate{opacity: 0->1}
effect:onHover{scale:1->1.05}
color{gray-500}

onClick=submit
if=(x > 0)
bind=text
```

## Grammar

A modifier is `identifier "{" arguments "}"`:

```
modifier        = modifier_head "{" modifier_body "}"
modifier_head   = identifier | identifier ("+" identifier)*   # "+" chains combo effects
modifier_body   = argument ("," argument)*
argument        = literal | token | nested_modifier
nested_modifier = identifier "{" modifier_body "}"
literal         = number | percent | string
token           = identifier ("-" identifier)*
identifier      = ascii_letter (ascii_letter | digit)*
```

No arithmetic operators (`+ - * /`) inside `{}`, except the `+` used to chain combo
effects in the modifier's head: `shadow+inner{...}`.

```
padding{10 + 5}     // error
scale{1 * 2}        // error
```

An empty block is not allowed: `padding{}` is an error.

## Categories

- **Layout** — `margin{} padding{} gap{}`
- **Offset** — post-layout visual translation
- **Visual** — `radius{} shadow{} opacity{} background{}`
- **Typography** — `text{} font{} lineHeight{}`
- **Color** — `color{} background{}` (see `ui/styling.md`)
- **Animation** — `animate{} effect:onX{}` (see `ui/animations.md`)
- **Event** — `onClick= onHover= ...` (via `=`, see `guide/events.md`)
- **2D drawing** — `rune2d { ... }` (see `guide/rune2d.md`)
- **Geometry** — `width{} height{} frame{} clip{}`

## Axis selectors

Layout, offset, and radius modifiers share one axis-suffix model:

```
.t top       .b bottom     .l left      .r right
.x horizontal   .y vertical    .all all sides
```

Radius additionally supports corners: `.tl .tr .bl .br`.

```
padding.t{12}
gap.x{8}
offset.y{-6}
radius.tl{6}
```

## Layout modifiers

`margin` (outside the element), `padding` (inside the container), `gap` (between a
container's children) — all axis-aware, all accept negative values:

```
margin.t{20}    margin.b{12}    margin.x{16}    margin.all{24}
padding.y{10}   padding.x{20}   padding.b{6}    padding.all{8}
gap.y{12}       gap.x{6}
```

A negative `margin` can pull an element outside its parent's bounds; a negative
`padding` visually collapses the inner space; a negative `gap` pulls children closer
together or overlaps them.

Values are any `Length`: `20`, `20px`, `2dp`, `-12`, `-4px`, `10%` (percentages only
resolve inside a bounded container). Axis modifiers combine and can overlap —
later/more specific ones win:

```
padding.all{10} + padding.x{20}  →  left=20 right=20 top=10 bottom=10
margin.all{12} + margin.b{-6}    →  top=12 left=12 right=12 bottom=-6
```

Computed in this order: `padding → layout → gap → margin`.

```
<Column padding.x{16} padding.y{12} gap.y{8}>
<Text margin.t{20}>"Rune"</Text>
<Button margin.b{-4} padding.b{6}>"Start"</Button>
</Column>
```

Errors: `padding=20` (no `=` form), `margin{10 + 5}` (no arithmetic),
`gap.y{"small"}` (no strings), `gap.all{percentVar}` (no variables in UI Mode).

## Offset modifiers

`offset` applies **after** the layout phase — it never affects measurement,
placement, or sibling layout. It's a pure visual translation from the position
layout already computed.

Same axis suffixes as layout modifiers: `offset.x{10} offset.y{-8} offset.t{6}
offset.all{-4}`.

Computed order: `measure → layout → gpu-geometry → offset → render`. Offset adds on
top of any transform animation. Percentages are not allowed for offset (it's a raw
pixel-space GPU transform).

```
<Button offset.y{-6}>"Hover me"</Button>
<Icon offset.x{4} offset.y{-2} />
<Text margin.t{12} offset.y{-4}>"Hello"</Text>   // 12px layout offset, then 4px visual nudge up
```

Errors: `offset=10`, `offset{"10px"}`, `offset.percent{10%}`, `offset.x{a + b}`.

## Visual modifiers

`radius{}`, `shadow{}`, `opacity{}`, `background{}` — applied after layout,
offset, and transform; none of them affect element size.

```
radius{12px}
radius.all{16}    radius.t{20}    radius.b{8}    radius.tl{6}    radius.tr{10px}

shadow{4px 8px 20px black{20%}}
shadow{0px 2px 6px gray-900{40%}}

opacity{0.5}
opacity{50%}

background{gray-800}
background{#FF00AA}
background{rgb(100, 200, 120)}
```

Order of application: `measure → layout → gap → margin → offset → transform →
visual → render` — visual modifiers are the last stage before the GPU draws.

Errors: `radius="12px"` (needs `{}`), `radius{10 + 5}` (no arithmetic),
`shadow{"big"}` (no strings), `background{someVar}` (no variables),
`radius{50%}` (radius is always a `Length`, never a percent).

## Gradient modifiers

A gradient is a `Color`-DSL subtype, usable anywhere a color is accepted
(`background{}`, `text{}`, `fill{}`, `stroke{}`, `shadow{ color{...} }`,
`rune2d{ fillRect / fillPath }`):

```
gradient.<type>{ ... }     // type ∈ { linear, radial, conic }
```

**Linear:**

```
background{
    gradient.linear{
        from{blue-400}
        to{blue-900}
    }
}
```

Fields: `from{Color}`, `to{Color}`, `stop{Percent Color}`, `angle{Angle}` (e.g.
`45deg`), `start.x{} start.y{}` / `end.x{} end.y{}` (0..1).

**Radial:**

```
background{
    gradient.radial{
        center.x{0.5}
        center.y{0.5}
        radius{80px}
        from{purple-300}
        to{purple-900}
    }
}
```

Fields: `center.x{} center.y{}`, `radius{Length}`, `from{} to{} stop{}`.

**Conic:**

```
background{
    gradient.conic{
        angle{0deg}
        stop{0% red}
        stop{120deg blue}
        stop{240deg green}
    }
}
```

Errors: `gradient="linear(...)"` (no strings), `gradient.linear{ from{a + b} }` (no
expressions), `gradient.radial{ percentVar }` (no variables), `stop{"red"}` (Color
DSL doesn't take strings), `angle{90}` (needs a `deg` unit).

## Rune2D modifiers

Inside `rune2d { ... }`, the same `{}` DSL applies in GPU immediate mode:

```
rune2d {
    fillRect 0 0 100 40
    strokeRect 0 0 100 40 color{gray-700}
    text { "Hi" size{20px} }
}
```

## Precedence, once more

```
padding → layout (measure, size) → gap → margin → offset → transform (animation)
→ visual (shadow, color, radius) → text (font, lineHeight) → rendering (rune2d)
```

Offset is the only modifier that runs after layout but before transform.

## Nesting

Modifiers can nest, as long as the nested value is semantically valid for its
parent:

```
color{gray-400{30%}}
shadow{0px 4px 12px black{20%}}
stroke{color{blue-400}, width{2px}}
```

## Errors, collected

```
padding=10               // wrong form — needs {}
offset=4                  // wrong form
padding{10 + 5}            // arithmetic not allowed
margin{a*b}                 // arithmetic not allowed
padding.q{12}                 // invalid axis
offset.xy{10}                  // invalid axis
padding{"large"}                // strings not allowed for layout values
gap.x{"big"}                     // strings not allowed
12color{red}                      // invalid identifier
```

An unrecognized modifier identifier is a compiler warning, not a hard error.
