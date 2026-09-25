# Tint2D — GPU 2D Rendering

`tint2d { ... }` is Tint's declarative, GPU-first 2D API — a full replacement for
Canvas2D, SVG, and DOM-based rendering, running entirely on WebGPU. It's a separate
layer from UI components (`<Panel>`, `<Text>`) and callable from any TintLogic
function: shapes, paths, text, filters and effects, images, transforms — enough to
build 2D tools, games, editors, and visualizers.

## Basic syntax

```
tint2d {
    fill {
        rect(0, 0, 200, 120)
        color=blue-500
    }

    stroke {
        circle(100, 60, 40)
        width=4px
        color=white{80%}
    }

    text {
        "Tint!"
        size=24px
        at(20, 90)
    }
}
```

`tint2d` returns a `Texture`, which you can use in UI, pass to a GPU kernel, save to
a file, or compose with other textures.

## Primitives

```
rect(x, y, width, height)
roundRect(x, y, width, height, radius)

circle(cx, cy, radius)
ellipse(cx, cy, rx, ry)

line(x1, y1, x2, y2)
polyline([vec2(...), vec2(...)])

polygon([vec2(10,10), vec2(60,10), vec2(40,40)])

path {
    M 20 20
    L 180 40
    Q 200 80 100 140
    Z
}
```

The `path` block uses the same command letters as SVG paths.

## Fill / stroke

```
fill {
    rect(0,0,200,120)
    color=purple-500{70%}
    blur{4px}
}

stroke {
    circle(100,60,40)
    width=3px
    color=white
    join=round     // round | bevel | miter
    cap=butt        // butt | square | round
}
```

## Text

```
text {
    "Hello Tint"
    size=24px
    weight=bold
    color=white{90%}
    at(20, 80)
}
```

Planned: `font="Inter"`, `tracking={tight}`, `rotate{12deg}`.

## Images

```
image {
    source=avatar
    at=vec2(40,40)
    size=vec2(120,120)
    radius{12px}
}

image(avatar, at=vec2(0,0), size=vec2(200,200))   // short form
```

## Effects

Any `fill`/`stroke`/`text` block accepts TintStyle effects, including combos:

```
fill {
    rect(0,0,200,120)
    color=gray-800
    shadow{0px 8px 40px black{30%}}
    blur{20px}
}

stroke {
    circle(100,60,40)
    width=4px
    color=blue-500
    effect=shadow+glow{8px blue-400{40%}}
}
```

## Transforms

```
tint2d {
    transform {
        translate(40, 20)
        rotate(20deg)
        scale(1.2)
    }

    fill { rect(0,0,120,80) color=blue-300 }
}
```

Each `transform` block applies to the draw block that follows it.

## Using the result

As a returned texture:

```
fn drawCard() -> Texture {
    tint2d {
        fill { rect(0,0,300,180) color=gray-900 }
        text { "Tint2D" size=28px at(20,80) }
    }
}
```

Fed into a GPU kernel:

```
fn blurredCard() -> Texture {
    let base = tint2d { fill { rect(0,0,300,180) color=gray-900 } };
    return gpu.compute(blurKernel, base, 12.0);
}
```

In UI:

```
<Panel>
    <Image source=drawCard() />
</Panel>
```

Event-driven Tint2D (planned):

```
tint2d:onHover {
    stroke { rect(0,0,200,120) width=4px color=blue }
}
```

## Current limitations (v0.1)

No frame-by-frame game loop yet, no offscreen composition API (planned v0.2), no
text layout engine yet, no advanced path stroking yet (planned v0.3).

## Why Tint2D over Canvas2D

| Canvas2D | Tint2D |
|---|---|
| CPU-based | Full WebGPU |
| Weak anti-aliasing | Shader-based AA |
| No built-in effects | Any TintStyle effect |
| Imperative API | Declarative DSL |
| Basic paths | Full path engine |
| Ad-hoc color handling | Strict TintColor system |

## Summary

`tint2d {}` is a declarative, WebGPU-native 2D API — a real Canvas2D replacement,
and the eventual foundation for 2D games, editors, UI, and visualizations in Tint,
fully interoperable with TintLogic and GPU kernels.
