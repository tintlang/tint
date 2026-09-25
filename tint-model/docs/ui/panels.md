# `<Panel>` — the Core Visual Container

`<Panel>` is Tint's primary visual container, rendered through WebGPU. It replaces
HTML's `<div>`, but behaves more like a Flutter-style GPU layout node. Every visual
property on a panel uses a TintStyle `{}` modifier.

## Basic use

```
<Panel
    padding{16}
    spacing{12}
>
    <Text> "Content" </Text>
</Panel>
```

## Properties

**Layout:**

```
padding{16}
padding{12 24}         // vertical horizontal
spacing{12}
align{center}
gap{16}
```

**Visual:**

```
radius{12px}
color{gray-900{20%}}
opacity{80%}
border{1px solid gray-700{40%}}
rotate{5deg}
scale{1.1}
```

**Effects:**

```
shadow{4px 8px 20px black{20%}}
shadow+inner{0px 2px 8px black{20%}}
glow{blue-400{40%}}
```

**Animation:**

```
animate{opacity: 0 -> 1}
duration{300ms}
speed{normal}
curve{ease-out}
```

## Nesting

```
<Panel padding{20} color{gray-800{25%}}>
    <Panel padding{12} radius{8px} color{gray-700{15%}}>
        <Text> "Inner Panel" </Text>
    </Panel>
</Panel>
```

## `<Panel>` vs `<Block>`

| Element | Renders | Purpose |
|---|---|---|
| `<Panel>` | yes | UI, visual structure, layout |
| `<Block>` | no | logic, conditions, loops, match patterns |

## Rules

A panel accepts visual modifiers `{}` (layout, effects, animation), events
(`onClick=`, `onHover=`, ...), and nested UI nodes. It does **not** accept
HTML-style attributes, CSS, or inline styles — it's a pure WebGPU UI element, and
the foundation the rest of the visual layer builds on.
