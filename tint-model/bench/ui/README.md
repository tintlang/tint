# UI benchmark

The same table-of-rows app (create / replace / update every 10th / select / swap /
remove / append / clear, 1k and 10k rows) in Tint, React, Svelte and vanilla JS,
driven by headless Chromium. Every app renders the same DOM shape.

```bash
cd web && npm install && npx vite build && cd ..        # React, Svelte, vanilla
../../target/release/tint build tint/app.tn -o tint/index.html          # Tint, logic in Tint
python3 gen_tint_render.py
../../target/release/tint build tint/app-render.tn -o tint/render.html  # Tint, prebuilt lists (renderer only)
NODE_PATH=$(npm root -g) node run.js --runs 5           # needs playwright; writes results/latest.md
```

`tint` runs the list logic in the tree-walker VM the browser uses today, whose list
operations cost O(n) per access (two value representations converted on every
variable read), so 10k-row steps are quadratic. `tint-render` assigns pre-built list
literals instead and isolates the DOM renderer.

`web/react-stack.html` is the same app in React with Tailwind-style utility classes (the stack a
typical React project ships); `npx vite build` in `web/` builds it with the other variants.

Tint has no per-row event handlers with arguments, so "select" and "remove" act on
fixed row indices; the DOM work is the same.
