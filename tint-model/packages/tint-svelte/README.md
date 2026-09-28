# `@tintlang/svelte`

Svelte action for the shared `@tintlang/runtime` package.

```sh
npm install @tintlang/runtime @tintlang/vite @tintlang/svelte
```

```svelte
<script lang="ts">
  import source from "./App.tn";
  import { tint } from "@tintlang/svelte";
</script>

<div use:tint={{ source, entry: "App" }}></div>
```

The action mounts Tint into the element, reloads the existing session when the
source changes, and unmounts it when Svelte destroys the element. Use
`onError` to surface compile or runtime errors.
