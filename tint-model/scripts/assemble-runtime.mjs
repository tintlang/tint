import { mkdtemp, copyFile, rm } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";

const scriptsDir = path.dirname(fileURLToPath(import.meta.url));
const modelDir = path.resolve(scriptsDir, "..");
const packageDir = path.join(modelDir, "packages", "tint-runtime");
const outputDir = await mkdtemp(path.join(os.tmpdir(), "tint-runtime-"));

function run(command, args) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      cwd: modelDir,
      stdio: "inherit",
    });
    child.on("error", reject);
    child.on("exit", (code) => {
      if (code === 0) resolve();
      else reject(new Error(`${command} exited with code ${code}`));
    });
  });
}

try {
  await run("wasm-pack", [
    "build",
    "crates/tint-wasm",
    "--release",
    "--target",
    "web",
    "--out-dir",
    outputDir,
  ]);

  for (const file of ["tint_wasm.js", "tint_wasm_bg.wasm", "tint_wasm.d.ts"]) {
    await copyFile(path.join(outputDir, file), path.join(packageDir, file));
  }

  console.log(`assembled Tint runtime in ${packageDir}`);
} finally {
  await rm(outputDir, { recursive: true, force: true });
}
