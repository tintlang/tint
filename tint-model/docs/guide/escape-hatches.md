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
