# `@tintlang/runtime`

This package is the browser host for Tint applications. It ships the
prebuilt Rust/WASM Tint runtime and owns its initialization; an application
does not install Rust, run `wasm-pack`, or rebuild WASM.

## For application authors

Install the published package:

```sh
npm install @tintlang/runtime
```

Load Tint source and mount it into the page:

```ts
import { mount } from "@tintlang/runtime";
import source from "./App.tn?raw";

const app = await mount(source, document.querySelector("#app")!, {
  entry: "App",
});
```

The application author does not run `cargo`, `wasm-pack`, or any WASM build
command. Those steps belong to the Tint runtime release process.

Runtime assembly is a maintainer/release step. See the repository
[`CONTRIBUTING.md`](../../CONTRIBUTING.md) for the command.

The first public API is source-oriented:

```ts
import { mount } from "@tintlang/runtime";

const app = await mount(source, document.querySelector("#app")!, {
  entry: "App",
});

app.reload(nextSource);
app.dispatch("on_click");
app.unmount();
```

For a precompiled artifact, the same runtime can load Tint bytecode:

```ts
import { compile, mountBytecode } from "@tintlang/runtime";

const bytecode = await compile(source);
const app = await mountBytecode(bytecode, document.querySelector("#app")!);
```

Both modes use the same VM and session lifecycle. The current Vite integration
keeps `.tn` source loading transparent for development and HMR; see
[`docs/guide/vite.md`](../../docs/guide/vite.md).
