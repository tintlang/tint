# RuneUI Syntax

RuneUI is a declarative, strict, WebGPU-first UI DSL — no HTML, CSS, JSX, or DOM
patterns. It rests on one rule:

| Syntax | Used for |
|---|---|
| `{}` | styles, modifiers, geometry, animation, Rune2D |
| `=` | logic, events, bindings, conditions, data |

## Entry point

```
ui fn App() {
    <Column spacing{20} padding{20}>
        <Text> "Hello Rune!" </Text>
    </Column>
}
```

## The logic layer — `<Block>`

`<Block>` never renders anything itself; it just drives `if`/`for`/`match` inside
the UI tree.

```
<Block if=visible>
    <Text> "Visible!" </Text>
</Block>

<Block for=item in items>
    <Panel padding{12}>
        <Text> "{item.name}" </Text>
    </Panel>
</Block>

<Block match=status>
    <case ready>   <Text>"OK"</Text> </case>
    <case error>   <Text>"Error"</Text> </case>
    <case loading> <Text>"Loading…"</Text> </case>
</Block>
```

Full details: `ui/blocks.md`.

## The panels layer — components that render

### `<Panel>`

```
<Panel
    padding{16}
    spacing{12}
    radius{12px}
    color{gray-900{20%}}
>
    <Text> "Hello Rune" </Text>
</Panel>
```

Full details: `ui/panels.md`.

### `<Text>`

```
<Text style=text{xl, bold}>
    "Rune UI"
</Text>
```

```
text{sm}    text{xl, bold}    text{24px}    text{lg, italic}    color{blue-400}
```

### `<Button>`

```
<Button onClick=increment>
    "+"
</Button>

<Button
    onClick=increment
    onHover{scale: 1 -> 1.05}
    shadow{0px 4px 16px black{20%}}
>
    "Hover me"
</Button>
```

### Layout — `<Row>` / `<Column>`

```
<Row spacing{12}>
    <Text> "A" </Text>
    <Text> "B" </Text>
</Row>

<Column spacing{16} padding{20}>
    ...
</Column>
```

Layout modifiers: `gap{12}`, `padding{20}`, `spacing{24}`, `width{100%}`,
`height{240px}`.

### `<Icon>` / `<Image>`

```
<Icon size{24}>
    RunePath"M10 2 L14 12..."
</Icon>

<Image source=myTexture radius{12px} />
```

## State and binding

```
ui Counter {
    state count = 0;

    <Column spacing{16}>
        <Text> "Count: {count}" </Text>

        <Button onClick=increment>
            "+"
        </Button>
    </Column>
}

fn increment() {
    count += 1;
}
```

## Animation

```
<Panel
    animate{opacity: 0 -> 1, y: 12px -> 0px}
    speed{normal}
>

<Button
    onHover{scale: 1 -> 1.1}
    onLeave{scale: 1.1 -> 1}
>
```

Mount and change triggers:

```
animate:onMount{opacity: 0 -> 1}
animate:onChange(count){scale: 1 -> 1.15}
```

Full details: `ui/animations.md`.

## Effects (RuneStyle)

```
shadow+inner{0px 2px 12px black{18%}}      // combo

shadow{4px 8px 20px black{20%}}
inner{0px 0px 10px blue-300{40%}}
glow{blue-500{50%}}
blur{20px}
```

Full details: `ui/styling.md`.

## Rune2D (GPU immediate mode)

```
<Canvas>
    rune2d {
        fillRect 0 0 200 40;
        strokePath line { width=2px }
    }
</Canvas>
```

Full details: `guide/rune2d.md`.

## Full example

```
ui App {
    state count = 0;

    <Column spacing{24} padding{24}>

        <Text style=text{3xl, bold}>
            "Rune UI Framework"
        </Text>

        <Button
            onClick=increment
            onHover{scale: 1 -> 1.1}
            shadow{0px 4px 16px black{20%}}
        >
            "+"
        </Button>

        <Block if=(count > 0)>
            <Text style=text{xl}> "Count: {count}" </Text>
        </Block>

        <Panel padding{12} radius{8px} color{gray-900{10%}}>
            <Text> "Static card" </Text>
        </Panel>

    </Column>
}

fn increment() {
    count += 1;
}
```

## Principles

All visual parameters go through `{}`, all logical parameters through `=`.
`<Block>` drives logic but never renders; `<Panel>` and the rest render through
WebGPU. The UI is fully declarative, string interpolation `{}` works inside logic,
animation is built into the language, typing is complete, and there is zero
HTML/CSS underneath any of it.
