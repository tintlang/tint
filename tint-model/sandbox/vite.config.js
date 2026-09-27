import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import wasm from "vite-plugin-wasm";
import fs from "node:fs";
import path from "node:path";

const r = (p) => fileURLToPath(new URL(p, import.meta.url));

// Tint imports are source imports, not runtime fetches. Vite expands them
// before the source reaches the WASM parser, so the compiler remains
// filesystem-independent in the browser and native CLI callers can still
// provide an already-expanded source string.
function tintSourceImports() {
  return {
    name: "tint-source-imports",
    enforce: "pre",
    resolveId(source, importer) {
      if (!importer || !source.endsWith(".tn")) return null;
      return path.resolve(path.dirname(importer), source);
    },
    load(id) {
      if (!id.endsWith(".tn")) return null;
      const seen = new Set();
      const watched = new Set();
      const expand = (file) => {
        const absolute = path.resolve(file);
        if (seen.has(absolute)) throw new Error(`Tint import cycle: ${absolute}`);
        seen.add(absolute);
        watched.add(absolute);
        const source = fs.readFileSync(absolute, "utf8");
        const expanded = source.replace(/^\s*import\s+"([^\"]+\.tn)"\s*;?\s*$/gm, (_, specifier) => {
          return expand(path.resolve(path.dirname(absolute), specifier));
        });
        seen.delete(absolute);
        return expanded;
      };
      const expanded = expand(id);
      for (const file of watched) this.addWatchFile(file);
      return `export default ${JSON.stringify(expanded)};`;
    },
    handleHotUpdate({ file, server }) {
      if (!file.endsWith(".tn")) return;
      server.ws.send({ type: "full-reload", path: "*" });
      return [];
    },
  };
}

function tintWasmReload() {
  const wasmDir = path.resolve(r("../pkg-web"));
  return {
    name: "tint-wasm-reload",
    configureServer(server) {
      server.watcher.add(wasmDir);
      const onChange = (file) => {
        if (file.startsWith(wasmDir)) server.ws.send({ type: "full-reload", path: "*" });
      };
      server.watcher.on("change", onChange);
      server.watcher.on("add", onChange);
    },
  };
}

// Recent wasm-bindgen (`--target web`) emits an ESM import for the wasm
// binary. vite-plugin-wasm teaches Vite to load that import. The build target
// stays esnext, so the generated top-level await remains native browser ESM.
export default defineConfig({
  plugins: [tintSourceImports(), tintWasmReload(), wasm()],
  // The generated browser WASM glue uses native top-level await.
  build: {
    target: "esnext",
    // Three Tint-rendered pages: `index.html` (landing), `app.html`
    // (the sandbox workbench), `pacman.html`, and `pong.html`.
    // Without listing all three here, `vite build` only picks up
    // index.html and silently drops the others from dist/.
    rollupOptions: {
      input: {
        main: r("./index.html"),
        app: r("./app.html"),
        pacman: r("./pacman.html"),
        pong: r("./pong.html"),
      },
    },
  },
});
