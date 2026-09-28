#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const version = (await readFile(join(root, "VERSION"), "utf8")).trim();

if (!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(version)) {
  throw new Error(`Invalid semantic version in VERSION: ${version}`);
}

const packageFiles = [
  "vscode-tint/package.json",
  "tint-model/sandbox/package.json",
  "sandbox/pkg-web/package.json",
  "tint-model/crates/tint-wasm/sandbox/pkg-web/package.json",
  "tint-model/sandbox/pkg-web/package.json",
];

const lockFiles = [
  "vscode-tint/package-lock.json",
  "tint-model/sandbox/package-lock.json",
];

const cargoWorkspacePath = join(root, "tint-model/Cargo.toml");
const cargoWorkspaceSource = await readFile(cargoWorkspacePath, "utf8");
const cargoWorkspacePattern = /(^\[workspace\.package\]\nversion\s*=\s*")[^"]+("\s*)/m;
const cargoWorkspaceUpdated = cargoWorkspaceSource.replace(
  cargoWorkspacePattern,
  `$1${version}$2`,
);

if (!cargoWorkspacePattern.test(cargoWorkspaceSource)) {
  throw new Error("Could not update version in tint-model/Cargo.toml");
}

await writeFile(cargoWorkspacePath, cargoWorkspaceUpdated);

for (const relativePath of packageFiles) {
  const path = join(root, relativePath);
  let source;
  try {
    source = await readFile(path, "utf8");
  } catch (error) {
    if (error.code === "ENOENT") {
      continue;
    }
    throw error;
  }
  const updated = source.replace(
    /(^\s*"version":\s*")[^"]+("\s*,?)/m,
    `$1${version}$2`,
  );

  if (!/(^\s*"version":\s*")[^"]+("\s*,?)/m.test(source)) {
    throw new Error(`Could not update package version in ${relativePath}`);
  }

  await writeFile(path, updated);
}

for (const relativePath of lockFiles) {
  const path = join(root, relativePath);
  const source = await readFile(path, "utf8");
  const matches = [...source.matchAll(/(^\s*"version":\s*")[^"]+("\s*,?)/gm)];

  if (matches.length < 2) {
    throw new Error(`Could not find project versions in ${relativePath}`);
  }

  let updated = source;
  for (const match of matches.slice(0, 2).reverse()) {
    const start = match.index;
    updated = `${updated.slice(0, start)}${match[1]}${version}${match[2]}${updated.slice(start + match[0].length)}`;
  }

  await writeFile(path, updated);
}

console.log(`Synchronized project version: ${version}`);
