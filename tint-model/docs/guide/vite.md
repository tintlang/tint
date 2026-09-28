# Vite integration

Tint provides a small Vite plugin for importing `.tn` files as JavaScript
modules. The browser runtime remains the prebuilt `@tintlang/runtime` package;
application authors do not run `wasm-pack`.

## Install

```sh
npm install @tintlang/runtime @tintlang/vite
```

Enable the plugin in `vite.config.ts`:

```ts
import { defineConfig } from "vite";
import tint from "@tintlang/vite";

export default defineConfig({
  plugins: [tint()],
});
```

## Import and mount a `.tn` file

The plugin exports the source as the default export, plus named `source` and
`entry` exports. The entry defaults to the file name, so `App.tn` maps to
`App`:

```ts
import source, { entry } from "./App.tn";
import { mount } from "@tintlang/runtime";

await mount(source, document.querySelector("#app")!, { entry });
```

Override the default entry for a project when needed:

```ts
// vite.config.ts
export default defineConfig({
  plugins: [tint({ entry: "Landing" })],
});
```

The current plugin emits Tint source text. This keeps development and HMR
transparent and uses the same runtime API as manual source loading. A future
production bytecode transform can keep the same `source`/`entry` module
contract.

## Links and browser tabs

Tint routes become normal DOM links. Use `target||"_blank"` for an external
link that should open in a new tab:

```tn
GithubLink {
    route||"https://github.com/tintlang/tint"
    target||"_blank"
    "GitHub"
}
```

The DOM renderer adds `rel="noopener noreferrer"` automatically for `_blank`.
