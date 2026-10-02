// UI benchmark: Tint vs React vs Svelte vs vanilla JS, in headless Chromium.
//   cd bench/ui/web && npm install && npx vite build   (once)
//   cd bench/ui && tint build tint/app.tn -o tint/index.html
//   NODE_PATH=$(npm root -g) node run.js [--runs 7] [--apps tint,react,svelte,vanilla]
const http = require('http'), fs = require('fs'), path = require('path'), zlib = require('zlib');
const { chromium } = require('playwright');

const args = process.argv.slice(2);
const opt = (k, d) => { const i = args.indexOf('--' + k); return i >= 0 ? args[i + 1] : d; };
const RUNS = +opt('runs', 7);
const APPS = opt('apps', 'tint,tint-render,react,react-stack,svelte,vanilla').split(',');
const URLS = { tint: '/tint/index.html', 'tint-render': '/tint/render.html', react: '/web/dist/react.html', 'react-stack': '/web/dist/react-stack.html', svelte: '/web/dist/svelte.html', vanilla: '/web/dist/vanilla.html' };
const ROOT = __dirname;
const MIME = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.wasm': 'application/wasm' };

const server = http.createServer((req, res) => {
  const f = path.join(ROOT, decodeURIComponent(req.url.split('?')[0]));
  if (!f.startsWith(ROOT) || !fs.existsSync(f) || fs.statSync(f).isDirectory()) { res.writeHead(404); return res.end(); }
  res.writeHead(200, { 'content-type': MIME[path.extname(f)] || 'application/octet-stream' });
  fs.createReadStream(f).pipe(res);
});

// Ordered scenario on one page; [label, button text, rows expected afterwards | null].
const STEPS = [
  ['create 1,000 rows', 'Create 1,000 rows', 1000],
  ['replace 1,000 rows', 'Create 1,000 rows', 1000],
  ['update every 10th', 'Update every 10th row', 1000],
  ['select row', 'Select row', 1000],
  ['swap rows', 'Swap rows', 1000],
  ['remove row', 'Remove row', 999],
  ['append 1,000 rows', 'Append 1,000 rows', 1999],
  ['clear 2,000 rows', 'Clear', 0],
  ['create 10,000 rows', 'Create 10,000 rows', 10000],
  ['update every 10th (10k)', 'Update every 10th row', 10000],
  ['clear 10,000 rows', 'Clear', 0],
];

async function measureApp(browser, app) {
  const page = await browser.newPage();
  page.on('pageerror', (e) => console.error(app, 'pageerror', e.message));
  const t0 = Date.now();
  await page.goto(`http://127.0.0.1:${server.address().port}${URLS[app]}`);
  await page.waitForFunction(() => [...document.querySelectorAll('button')].some((b) => b.textContent.includes('Create 1,000')));
  const startup = Date.now() - t0;
  const out = {};
  for (const [label, text, expect] of STEPS) {
    const r = await page.evaluate(async ([text, expect]) => {
      const btn = [...document.querySelectorAll('button')].find((b) => b.textContent.trim() === text);
      const count = () => document.querySelectorAll('.table > *, [data-tag=Table] > *').length;
      await new Promise((r) => setTimeout(r, 50)); // settle
      const t = performance.now();
      btn.click();
      await Promise.resolve();
      document.body.offsetHeight; // force style + layout
      const work = performance.now() - t;
      await new Promise((r) => requestAnimationFrame(() => setTimeout(r, 0)));
      const frame = performance.now() - t;
      return { work, frame, rows: count(), ok: count() === expect };
    }, [text, expect]);
    if (!r.ok) console.error(`  ${app}: "${label}" expected ${expect} rows, got ${r.rows}`);
    out[label] = r;
  }
  const dom = await page.evaluate(() => document.querySelectorAll('*').length);
  await page.close();
  return { startup, steps: out, dom };
}

const median = (a) => a.slice().sort((x, y) => x - y)[Math.floor(a.length / 2)];
const fmt = (ms) => (ms >= 100 ? ms.toFixed(0) : ms >= 10 ? ms.toFixed(1) : ms.toFixed(2)) + ' ms';

(async () => {
  await new Promise((r) => server.listen(0, '127.0.0.1', r));
  const browser = await chromium.launch();
  const results = {};
  for (const app of APPS) {
    console.log('running', app);
    const runs = [];
    await measureApp(browser, app); // warm-up run (page cache, JIT)
    for (let i = 0; i < RUNS; i++) runs.push(await measureApp(browser, app));
    const steps = {};
    for (const [label] of STEPS) steps[label] = { work: median(runs.map((r) => r.steps[label].work)), frame: median(runs.map((r) => r.steps[label].frame)) };
    const file = path.join(ROOT, URLS[app]);
    // Everything a cold load fetches: the page, and for the Vite apps the scripts and
    // stylesheets it references plus the chunks those import (React, router, CSS ...).
    const files = new Set([file]);
    const queue = [file];
    while (queue.length) {
      const f = queue.pop();
      if (!URLS[app].startsWith('/web/') || !/\.(html|js|css)$/.test(f)) continue;
      const text = fs.readFileSync(f, 'utf8');
      for (const m of text.matchAll(/(?:src|href)="([^"]+\.(?:js|css))"|(?:from|import)\s*["']([^"']+\.(?:js|css))["']/g)) {
        const rel = m[1] || m[2];
        if (/^https?:/.test(rel)) continue;
        const p = path.resolve(path.dirname(f), rel);
        if (fs.existsSync(p) && !files.has(p)) { files.add(p); queue.push(p); }
      }
    }
    let bytes = 0, gz = 0, br = 0;
    for (const f of files) { const b = fs.readFileSync(f); bytes += b.length; gz += zlib.gzipSync(b, { level: 9 }).length; br += zlib.brotliCompressSync(b).length; }
    results[app] = { startup: median(runs.map((r) => r.startup)), steps, bytes, gz, br, dom: runs[0].dom };
  }
  await browser.close(); server.close();

  const lines = [`# UI benchmark (headless Chromium, median of ${RUNS} fresh-page runs)`, '',
    'Time = click() until script + forced style/layout are done (no paint). Same DOM shape in every app.', '',
    '| step | ' + APPS.join(' | ') + ' |', '|---|' + '---|'.repeat(APPS.length)];
  for (const [label] of STEPS) lines.push(`| ${label} | ` + APPS.map((a) => fmt(results[a].steps[label].work)).join(' | ') + ' |');
  lines.push('', '### Until the next frame (includes paint scheduling)', '', '| step | ' + APPS.join(' | ') + ' |', '|---|' + '---|'.repeat(APPS.length));
  for (const [label] of STEPS) lines.push(`| ${label} | ` + APPS.map((a) => fmt(results[a].steps[label].frame)).join(' | ') + ' |');
  lines.push('', '### Startup and size', '', '| | ' + APPS.join(' | ') + ' |', '|---|' + '---|'.repeat(APPS.length));
  lines.push('| load to first content | ' + APPS.map((a) => fmt(results[a].startup)).join(' | ') + ' |');
  lines.push('| everything fetched (raw / gzip / brotli) | ' + APPS.map((a) => `${(results[a].bytes / 1024).toFixed(0)} / ${(results[a].gz / 1024).toFixed(0)} / ${(results[a].br / 1024).toFixed(0)} KiB`).join(' | ') + ' |');
  const report = lines.join('\n') + '\n';
  console.log('\n' + report);
  fs.mkdirSync(path.join(ROOT, 'results'), { recursive: true });
  fs.writeFileSync(path.join(ROOT, 'results/latest.md'), report);
  fs.writeFileSync(path.join(ROOT, 'results/latest.json'), JSON.stringify(results, null, 2));
})();
