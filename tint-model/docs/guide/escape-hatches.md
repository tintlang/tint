# Escape hatches

Tint owns the UI subtree it renders, but applications can integrate external
DOM, Web Components, React, Svelte, and browser APIs through two small escape
hatches.

## Stable DOM references

Use `ref||"name"` to mark a node for host-side integration:

```tn
ChartHost {
    ref||"chart"
}
```

The DOM renderer emits `data-tint-ref="chart"`:

```ts
const host = root.querySelector('[data-tint-ref="chart"]');
mountExternalChart(host);
```

Tint owns the referenced element itself. External code should mount into that
element's children or treat it as a host boundary, and should clean up its own
resources before the Tint app is unmounted.

## JavaScript callbacks

Use `js||"name"` on a Tint node and provide callbacks to `mount`:

```tn
Button {
    js||open_settings
    "Settings"
}
```

```ts
const app = await mount(source, root, {
  entry: "App",
  callbacks: {
    open_settings: ({ event, element }) => {
      console.log("Tint callback", event.type, element);
      openSettingsDialog();
    },
  },
});
```

The runtime listens in capture phase on the mount element, so the callback is
still observed when Tint re-renders the clicked subtree. Callbacks are host
code, not Tint functions; use `click||handler` when the behavior belongs in
Tint's own state/runtime.

## JS and CSS files from Tint

`app { }` can name JavaScript modules and stylesheets next to the `.tn` file:

```tn
app {
    js::"./utils.js"
    css::"./theme.css"
}

ui fn App() {
    state greeting = shout("hello")   // calls utils.js

    Card {
        class||"card"                 // styled by theme.css
        Text { "{greeting}" }
    }
}
```

```js
// utils.js: every exported function is callable from Tint as `name(...)`
export function shout(text) { return text.toUpperCase() + "!"; }
```

- Paths resolve against the entry `.tn` file (`tint build`/`tint dev`) or against
  `baseUrl` (default: the page URL) when you use `mount()`.
- Calls are synchronous. Arguments and results are plain data: numbers, strings,
  booleans, lists (arrays) and maps (plain objects); `undefined`/`null` is unit.
  Functions, DOM nodes, class instances and Promises are rejected.
- A thrown JS error is logged with `console.error` and fails the call.
- `mount(source, el, { natives: { name: fn } })` adds or overrides functions
  without a file; these win over module exports of the same name.
- `tint build` embeds each file into the page, so the module is one blob: it
  cannot `import` other files. `mount()` loads modules with native `import()`,
  so relative imports and bundlers work there.
- The checker cannot see what a module exports, so once an app declares `js::`
  an unknown function name is no longer an error.
- `class||"a b"` sets CSS classes on a node. `ref||"name"` also works as a
  selector: `[data-tint-ref="name"]`.

## Host components (React and friends)

Mount a JS component into a Tint node with `component||"Name"` and optional
`props||{ expr }`. Keep the node childless; Tint owns the element, your
component owns what is inside it.

```tn
struct ChartProps { points: list, color: string }

Host {
    component||"Chart"
    props||{ ChartProps { points: data, color: "#6c5ce7" } }
    if{show_chart}
}
```

```ts
import { mount } from "@tintlang/runtime";
import { reactComponent } from "@tintlang/react";

await mount(source, root, {
  components: { Chart: reactComponent(Chart) },   // or (el, props) => ({ update, destroy })
});
```

- A component is `(element, props) => { update?(props); destroy?() } | void`.
  `update` runs when the props value changes (compared as canonical JSON, so an
  unchanged value does not call it); `destroy` when the node leaves the tree
  (`if{}`, route change), the name changes, or the app unmounts.
- The element is kept between renders, so React/Svelte state inside it survives
  Tint re-renders.
- `props` can be any value: a struct becomes an object, a list an array, a map an
  object. Functions are not passed; use `natives` or `callbacks` for events going
  back to Tint.
- Lifecycle runs in `mount()` only. `tint build` pages have no component host,
  and JSX needs your bundler anyway.
