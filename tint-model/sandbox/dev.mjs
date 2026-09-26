import { spawn } from "node:child_process";
import { readdir, stat } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const sandboxDir = path.dirname(fileURLToPath(import.meta.url));
const modelDir = path.resolve(sandboxDir, "..");
const cratesDir = path.join(modelDir, "crates");
let building = false;
let pending = false;
let previousSnapshot = new Map();

function runWasmBuild() {
  if (building) {
    pending = true;
    return;
  }
  building = true;
  const child = spawn("wasm-pack", [
    "build", "crates/tint-wasm", "--dev", "--target", "web", "--out-dir", "../../sandbox/pkg-web",
  ], { cwd: modelDir, stdio: "inherit" });
  child.on("exit", (code) => {
    building = false;
    if (code !== 0) console.error(`WASM build failed with exit code ${code}`);
    if (pending) {
      pending = false;
      runWasmBuild();
    }
  });
}

async function snapshot(dir, result = new Map()) {
  for (const entry of await readdir(dir, { withFileTypes: true })) {
    const file = path.join(dir, entry.name);
    if (entry.isDirectory()) await snapshot(file, result);
    else if (entry.name.endsWith(".rs")) result.set(file, (await stat(file)).mtimeMs);
  }
  return result;
}

async function pollRustSources() {
  try {
    const next = await snapshot(cratesDir);
    const changed = next.size !== previousSnapshot.size
      || [...next].some(([file, mtime]) => previousSnapshot.get(file) !== mtime);
    previousSnapshot = next;
    if (changed) runWasmBuild();
  } catch (error) {
    console.error(`Rust watch failed: ${error.message}`);
  }
}

previousSnapshot = await snapshot(cratesDir);
runWasmBuild();
const pollTimer = setInterval(pollRustSources, 1000);

const vite = spawn("npm", ["run", "dev", "--", "--host"], { cwd: sandboxDir, stdio: "inherit" });
const stop = () => {
  clearInterval(pollTimer);
  vite.kill();
  process.exit();
};
process.on("SIGINT", stop);
process.on("SIGTERM", stop);
