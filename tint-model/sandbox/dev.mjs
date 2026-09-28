import { spawn } from "node:child_process";
import { mkdtemp, readdir, rename, rm, stat } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const sandboxDir = path.dirname(fileURLToPath(import.meta.url));
const modelDir = path.resolve(sandboxDir, "..");
const cratesDir = path.join(modelDir, "crates");
let building = false;
let pending = false;
let previousSnapshot = new Map();

async function runWasmBuild() {
  if (building) {
    pending = true;
    return;
  }
  building = true;
  const stagingDir = await mkdtemp(path.join(os.tmpdir(), "tint-sandbox-wasm-"));
  const packageDir = path.join(sandboxDir, "pkg-web");
  const oldPackageDir = `${packageDir}.previous`;
  const child = spawn("wasm-pack", [
    "build", "crates/tint-wasm", "--dev", "--target", "web", "--out-dir", stagingDir,
  ], { cwd: modelDir, stdio: "inherit" });
  child.on("exit", (code) => {
    building = false;
    void (async () => {
      if (code !== 0) {
        await rm(stagingDir, { recursive: true, force: true });
        console.error(`WASM build failed with exit code ${code}`);
      } else {
        // Vite watches pkg-web while it imports the generated ESM glue. Build
        // elsewhere first, then replace the complete directory in one rename
        // so Vite never observes a half-written package.json/wasm pair.
        await rm(oldPackageDir, { recursive: true, force: true });
        try {
          await rename(packageDir, oldPackageDir);
        } catch (error) {
          if (error.code !== "ENOENT") throw error;
        }
        await rename(stagingDir, packageDir);
        await rm(oldPackageDir, { recursive: true, force: true });
      }
    })().catch((error) => {
      console.error(`WASM output swap failed: ${error.message}`);
    });
    if (pending) {
      pending = false;
      void runWasmBuild();
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
    if (changed) void runWasmBuild();
  } catch (error) {
    console.error(`Rust watch failed: ${error.message}`);
  }
}

previousSnapshot = await snapshot(cratesDir);
void runWasmBuild();
const pollTimer = setInterval(pollRustSources, 1000);

const vite = spawn("npm", ["run", "dev", "--", "--host"], { cwd: sandboxDir, stdio: "inherit" });
const stop = () => {
  clearInterval(pollTimer);
  vite.kill();
  process.exit();
};
process.on("SIGINT", stop);
process.on("SIGTERM", stop);
