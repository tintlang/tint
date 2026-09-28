# `@tintlang/vite`

Vite integration for Tint source files.

Install it with the Tint runtime:

```sh
npm install @tintlang/runtime @tintlang/vite
```

Enable the plugin:

```ts
// vite.config.ts
import { defineConfig } from "vite";
import tint from "@tintlang/vite";

export default defineConfig({
  plugins: [tint()],
});
```

Then import a Tint file like a normal module:

```ts
import source, { entry } from "./App.tn";
import { mount } from "@tintlang/runtime";

await mount(source, document.querySelector("#app")!, { entry });
```

The current plugin emits source text so development and HMR use the direct
source path. The module contract is already stable for a later production
bytecode transform.

For the full setup guide, entry override, and external-link syntax, see
[`docs/guide/vite.md`](../../docs/guide/vite.md).
