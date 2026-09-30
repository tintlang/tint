// Per-click split of Tint's time: program eval vs DOM mount (needs a tint-render build).
//   cd bench/ui && NODE_PATH=$(npm root -g) node timing.js [repeats]
// Prints the median of `repeats` (default 7) fresh runs of each step.
const { chromium } = require('playwright');
const http = require('http'), fs = require('fs'), path = require('path');
const REPEATS = +(process.argv[2] || 7);
const STEPS = ['Create 1,000 rows', 'Create 1,000 rows', 'Update every 10th row', 'Select row', 'Swap rows', 'Remove row', 'Append 1,000 rows'];
const med = (a) => a.slice().sort((x, y) => x - y)[Math.floor(a.length / 2)];
const min = (a) => Math.min(...a);
const srv = http.createServer((q, r) => {
  r.writeHead(200, { 'content-type': 'text/html' });
  fs.createReadStream(path.join(__dirname, 'tint/render.html')).pipe(r);
}).listen(0, async () => {
  const b = await chromium.launch();
  const acc = STEPS.map(() => ({ eval: [], mount: [] }));
  for (let run = 0; run < REPEATS; run++) {
    const p = await b.newPage();
    const logs = [];
    p.on('console', (m) => { const t = m.text(); if (/^TINT-TIMING \S+ eval /.test(t)) logs.push(t.slice(12)); });
    await p.goto('http://127.0.0.1:' + srv.address().port + '/');
    await p.evaluate(() => document.documentElement.setAttribute('data-tint-timing', ''));
    await p.waitForFunction(() => [...document.querySelectorAll('button')].some((x) => x.textContent.includes('Create 1,000')));
    for (const t of STEPS) {
      await p.evaluate(async (t) => { await new Promise((r) => setTimeout(r, 50)); [...document.querySelectorAll('button')].find((x) => x.textContent.trim() === t).click(); }, t);
      await p.waitForTimeout(150);
    }
    logs.forEach((l, i) => { const m = l.match(/(\S+) eval ([\d.]+) mount ([\d.]+)/); if (m && acc[i]) { acc[i].eval.push(+m[2]); acc[i].mount.push(+m[3]); acc[i].name = m[1]; } });
    await p.close();
  }
  console.log('step (median / min of ' + REPEATS + ')    eval       mount');
  acc.forEach((a) => console.log(String(a.name).padEnd(22), (med(a.eval).toFixed(1) + '/' + min(a.eval).toFixed(1)).padStart(10), (med(a.mount).toFixed(1) + '/' + min(a.mount).toFixed(1)).padStart(12)));
  await b.close(); srv.close();
});
