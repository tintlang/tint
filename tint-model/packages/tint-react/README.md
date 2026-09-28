# `@tintlang/react`

React adapter for the shared `@tintlang/runtime` package.

```sh
npm install @tintlang/runtime @tintlang/vite @tintlang/react
```

```tsx
import source, { entry } from "./App.tn";
import { Tint } from "@tintlang/react";

export function App() {
  return <Tint source={source} entry={entry} />;
}
```

The adapter owns the Tint mount lifecycle and reloads the existing Tint
session when `source` changes. React owns the wrapper element; Tint owns the
DOM inside it. Use `onError` to surface compile or runtime errors.
