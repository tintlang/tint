import { fileURLToPath } from "node:url";
import path from "node:path";
import { defineConfig } from "vite";
import wasm from "vite-plugin-wasm";
import tint from "../packages/tint-vite/index.js";

const r = (p) => fileURLToPath(new URL(p, import.meta.url));

export default defineConfig({
  plugins: [tint({ entry: "Smoke" }), wasm()],
  // The smoke fixture imports the package directly from this monorepo.
  // Published npm consumers do not need this because the package is inside
  // the application dependency tree.
  server: {
    fs: {
      allow: [path.resolve(r(".."))],
    },
  },
  build: {
    target: "esnext",
    rollupOptions: {
      input: r("./vite-smoke.html"),
    },
  },
});
