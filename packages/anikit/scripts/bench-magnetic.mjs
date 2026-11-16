// magnetic() in Chromium (Playwright): M magnetic buttons in a grid, the
// pointer sweeping across them (E synthetic pointermove events, each followed
// by one engine frame and a forced style + layout). Reports ms per event +
// frame and the JS heap kept per event (after a garbage collection, through
// the DevTools protocol) while the springs are still moving. Median of RUNS
// fresh pages.
// Usage: node scripts/bench-magnetic.mjs   (M=20,200 E=600 RUNS=5 DIST=path/to/dist)
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

const Ms = (process.env.M || '20,200').split(',').map(Number);
const E = +(process.env.E || 600);
const RUNS = +(process.env.RUNS || 5);
const median = (a) => a.sort((x, y) => x - y)[a.length >> 1];
const html = `<!doctype html><meta charset="utf-8"><style>
body{margin:0} #g{display:flex;flex-wrap:wrap;gap:20px;padding:20px;width:960px} .m{width:40px;height:40px;background:#6ea8ff}
</style><div id="g"></div>
<script type="module">import * as ak from './anikit.js'; window.ak = ak; window.ok = true;</script>`;

const browser = await playwright.chromium.launch();
console.log(`median of ${RUNS} fresh pages, ${E} pointer moves (lower is better)\n`);
console.log('M'.padStart(5), 'ms/move+frame'.padStart(14), 'heap B/move'.padStart(12));
for (const m of Ms) {
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
    const cdp = await pg.context().newCDPSession(pg);
    const heap = async () => {
      await cdp.send('HeapProfiler.collectGarbage');
      return (await cdp.send('Runtime.getHeapUsage')).usedSize;
    };
    await pg.evaluate(async (m) => {
      const ak = window.ak;
      await ak.ready;
      ak.engine.manual(true);
      document.getElementById('g').innerHTML = '<div class="m"></div>'.repeat(m);
      ak.magnetic('.m', { radius: 120 });
      let x = 0;
      window.sweep = (n) => {
        const t = performance.now();
        for (let i = 0; i < n; i++) {
          x = (x + 7) % 960;
          window.dispatchEvent(new PointerEvent('pointermove', { clientX: x, clientY: 60 + ((x * 3) % 400), bubbles: true }));
          ak.engine.tick(16);
          void document.body.offsetHeight;
        }
        return (performance.now() - t) / n;
      };
      window.sweep(200); // warm-up
    }, m);
    const h0 = await heap();
    const ms = await pg.evaluate((e) => window.sweep(e), E);
    const h1 = await heap();
    const moved = await pg.evaluate(() => [...document.querySelectorAll('.m')].some((e) => e.style.transform && e.style.transform !== 'none'));
    if (!moved) throw new Error('nothing moved');
    res.push([ms, (h1 - h0) / E]);
    await pg.close();
  }
  console.log(String(m).padStart(5), median(res.map((x) => x[0])).toFixed(3).padStart(14), Math.max(0, median(res.map((x) => x[1]))).toFixed(0).padStart(12));
}
await browser.close();
