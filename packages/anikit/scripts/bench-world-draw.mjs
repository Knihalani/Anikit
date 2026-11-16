// world().draw() in Chromium (Playwright): N bodies settling in a 1600x1600
// box, drawn every frame on a 2D canvas with the default colour. Reports ms
// per frame for `step` and for `draw` (including a 1-pixel `getImageData`,
// which makes the canvas actually rasterise what was recorded), 200 frames
// after 100 warm-up frames. matter.js has no comparable built-in canvas
// drawing, so no competitor column. Median of RUNS fresh pages.
// Usage: node scripts/bench-world-draw.mjs   (N=1000,5000 RUNS=5 DIST=path/to/dist)
import { createRequire } from 'node:module';
import { readFileSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const pkg = join(here, '..');
const dist = resolve(process.env.DIST || join(pkg, 'dist'));
const require = createRequire(import.meta.url);
let playwright;
try { playwright = require('playwright'); } catch { playwright = require(join(process.execPath, '../../lib/node_modules/playwright')); }

const Ns = (process.env.N || '1000,5000').split(',').map(Number);
const RUNS = +(process.env.RUNS || 5);
const median = (a) => a.sort((x, y) => x - y)[a.length >> 1];
const html = `<!doctype html><meta charset="utf-8"><body style="margin:0"><canvas id="c" width="1600" height="1600"></canvas>
<script type="module">import * as ak from './anikit.js'; window.ak = ak; window.ok = true;</script>`;

const browser = await playwright.chromium.launch();
console.log(`median of ${RUNS} fresh pages, ms per frame (lower is better)\n`);
console.log('N'.padStart(6), 'step'.padStart(9), 'draw'.padStart(9));
for (const n of Ns) {
  const res = [];
  for (let r = 0; r < RUNS; r++) {
    const pg = await browser.newPage({ viewport: { width: 1000, height: 800 } });
    await pg.route('http://bench.local/**', (route) => {
      const name = new URL(route.request().url()).pathname.slice(1);
      if (name === 'index.html') return route.fulfill({ status: 200, contentType: 'text/html', body: html });
      route.fulfill({ status: 200, contentType: 'text/javascript', body: readFileSync(join(dist, name)) });
    });
    await pg.goto('http://bench.local/index.html');
    await pg.waitForFunction(() => window.ok);
    res.push(await pg.evaluate(async (n) => {
      const ak = window.ak;
      await ak.ready;
      ak.engine.manual(true);
      const ctx = document.getElementById('c').getContext('2d');
      const w = ak.world({ autoStep: false, bounds: [0, 0, 1600, 1600] });
      const cols = Math.ceil(Math.sqrt(n)), sp = 1600 / cols;
      w.addMany(Array.from({ length: n }, (_, i) => ({ x: (i % cols) * sp + sp / 2, y: Math.floor(i / cols) * sp + sp / 2, r: sp * 0.3 })));
      let ts = 0, td = 0;
      for (let f = 0; f < 300; f++) {
        const a = performance.now();
        w.step(1 / 60);
        const b = performance.now();
        w.draw(ctx);
        ctx.getImageData(0, 0, 1, 1);
        const c = performance.now();
        if (f >= 100) { ts += b - a; td += c - b; }
      }
      const px = ctx.getImageData(0, 0, 1600, 1600).data;
      let filled = 0;
      for (let i = 3; i < px.length; i += 4) filled += px[i] > 0;
      if (filled < 1000) throw new Error(`nothing drawn (${filled} px; first body at ${w.data[0]}, ${w.data[1]})`);
      return [ts / 200, td / 200];
    }, n));
    await pg.close();
  }
  console.log(String(n).padStart(6), median(res.map((x) => x[0])).toFixed(3).padStart(9), median(res.map((x) => x[1])).toFixed(3).padStart(9));
}
await browser.close();
