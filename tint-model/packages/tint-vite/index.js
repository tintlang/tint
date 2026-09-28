import { readFile } from "node:fs/promises";
import path from "node:path";

function cleanId(id) {
  return id.split("?", 1)[0];
}

function queryPart(id) {
  const marker = id.indexOf("?");
  return marker === -1 ? "" : id.slice(marker);
}

function isTintFile(id) {
  return cleanId(id).endsWith(".tn");
}

function entryName(id) {
  const stem = path.basename(cleanId(id), ".tn");
  return stem || "App";
}

/**
 * Makes `.tn` files importable from any Vite application.
 *
 * The first version intentionally emits source text. That keeps dev/HMR
 * transparent and uses the same runtime API as a manually loaded source.
 * Production bytecode emission can be added later without changing the
 * module shape: the module will continue to export `source` and `entry`.
 */
export default function tint(options = {}) {
  const defaultEntry = options.entry;

  return {
    name: "tint",
    enforce: "pre",

    resolveId(id, importer) {
      if (!importer || !isTintFile(id)) return null;
      const resolved = path.isAbsolute(cleanId(id))
        ? cleanId(id)
        : path.resolve(path.dirname(cleanId(importer)), cleanId(id));
      return resolved + queryPart(id);
    },

    async load(id) {
      if (!isTintFile(id)) return null;

      const filename = cleanId(id);
      const source = await readFile(filename, "utf8");
      const entry = defaultEntry ?? entryName(filename);
      const encodedSource = JSON.stringify(source);
      const encodedEntry = JSON.stringify(entry);

      return `const source = ${encodedSource};
export { source };
export const entry = ${encodedEntry};
export default source;
`;
    },
  };
}
