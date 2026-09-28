# React and Svelte integration

Tint uses one browser runtime for framework integrations. React and Svelte
adapters only manage the mount lifecycle; they do not compile a second runtime
or create a second renderer.

## React

Install the runtime, Vite plugin, and React adapter:

```sh
npm install @tintlang/runtime @tintlang/vite @tintlang/react
```

Import the `.tn` module and render the adapter component:

```tsx
import source, { entry } from "./App.tn";
import { Tint } from "@tintlang/react";

export function App() {
  return <Tint source={source} entry={entry} />;
}
```

React owns the wrapper element. Tint owns the DOM subtree inside it. When the
source changes, the adapter calls `TintApp.reload` instead of remounting the
whole React tree. `onError` receives compile and runtime errors.

## Svelte

Install the Svelte adapter:

```sh
npm install @tintlang/runtime @tintlang/vite @tintlang/svelte
```

Use the `tint` action on an element:

```svelte
<script lang="ts">
  import source from "./App.tn";
  import { tint } from "@tintlang/svelte";
</script>

<div use:tint={{ source, entry: "App" }}></div>
```

The action reloads the existing Tint session on source updates and unmounts it
when the element is destroyed.

## Ownership boundary

The framework owns the mount element and surrounding layout. Tint owns the
children it renders inside that element. Do not let React or Svelte reconcile
Tint-owned descendants directly. Use the `@tintlang/runtime` API for explicit
escape hatches such as `dispatch`, `reload`, and `unmount`.
