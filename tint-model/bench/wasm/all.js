// Tint compiled to wasm vs Node, both in V8. From tint-model/:
//   node bench/wasm/all.js [runs] [task ...]
// Builds the runtime module and the emit example, compiles each bench task, then runs
// main() `runs` times per side in fresh processes and prints the medians.
const { execFileSync, spawnSync } = require('child_process');
const fs = require('fs');
const path = require('path');

const root = path.resolve(__dirname, '../..');
const out = path.join(__dirname, 'out');
const [runsArg, ...rest] = process.argv.slice(2);
const runs = Number(runsArg) || 9;
const all = ['fib', 'loop', 'collatz', 'mandel', 'sieve', 'sieve-big', 'binary-trees', 'hashmap', 'strings', 'wordfreq', 'nbody', 'grid-bfs'];
const tasks = rest.length ? rest : all;

const sh = (cmd, args) => execFileSync(cmd, args, { cwd: root, stdio: 'inherit' });
fs.mkdirSync(out, { recursive: true });
sh('cargo', ['build', '--release', '-q', '-p', 'tint-wasmgen', '--example', 'emit']);
sh('cargo', ['build', '-q', '-p', 'tint-wasmrt', '--target', 'wasm32-unknown-unknown', '--profile', 'wasm']);
fs.copyFileSync(path.join(root, 'target/wasm32-unknown-unknown/wasm/tint_wasmrt.wasm'), path.join(out, 'rt.wasm'));

const median = (xs) => xs.sort((a, b) => a - b)[Math.floor((xs.length - 1) / 2)];
const time = (script, arg, re) => {
  const xs = [];
  for (let i = 0; i < runs; i++) {
    const r = spawnSync('node', [path.join(__dirname, script), arg], { cwd: root, encoding: 'utf8' });
    const m = re.exec(r.stderr);
    if (!m) throw new Error(`${script} ${arg}: no timing in\n${r.stderr}`);
    xs.push(Number(m[1]));
  }
  return median(xs);
};

console.log('task'.padEnd(13), 'wasm_ms'.padStart(9), 'node_ms'.padStart(9), 'ratio'.padStart(7));
for (const t of tasks) {
  sh(path.join(root, 'target/release/examples/emit'), [`bench/tasks/${t}/main.tn`, path.join(out, `${t}.wasm`)]);
  const w = time('run.js', t, /main ([\d.]+)/);
  const j = time('jsrun.js', path.join(root, `bench/tasks/${t}/main.js`), /js ([\d.]+)/);
  console.log(t.padEnd(13), w.toFixed(1).padStart(9), j.toFixed(1).padStart(9), (w / j).toFixed(2).padStart(7));
}
