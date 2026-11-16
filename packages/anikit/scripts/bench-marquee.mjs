// marquee() in Chromium (Playwright): M marquees of 12 items down a long page
// (about 3 on screen). Reports the set-up time (M marquees, then a forced
// layout and one animation frame for the observers) and the ms per frame of
// `engine.tick` plus the style recalc and layout it causes (forced with
// `offsetHeight`), over 300 manual frames. Median of RUNS fresh pages.
// Usage: node scripts/bench-marquee.mjs   (M=20,100 RUNS=5 DIST=path/to/dist)
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

const Ms = (process.env.M || '20,100').split(',').map(Number);
const RUNS = +(process.env.RUNS || 5);
const median = (a) => a.sort((x, y) => x - y)[a.length >> 1];
const html = `<!doctype html><meta charset="utf-8"><style>
body{margin:0;font:16px sans-serif} .m{height:220px;border-top:1px solid #ddd} .i{padding:20px 40px;background:#eef;margin-right:8px;white-space:nowrap}
</style><div id="stage"></div>
<script type="module">import * as ak from './anikit.js'; window.ak = ak; window.ok = true;</script>`;

const browser = await playwright.chromium.launch();
console.log(`median of ${RUNS} fresh pages, ms (lower is better)\n`);
console.log('M'.padStart(5), 'set-up'.padStart(9), 'ms/frame'.padStart(10));
for (const m of Ms) {
  const res = [];
  for (let r = 0; r < RUNS; r++) {
    const pg = await browser.newPage({ viewport: { width: 1000, height: 700 } });
    await pg.route('http://bench.local/**', (route) => {
      const name = new URL(route.request().url()).pathname.slice(1);
      if (name === 'index.html') return route.fulfill({ status: 200, contentType: 'text/html', body: html });
      route.fulfill({ status: 200, contentType: 'text/javascript', body: readFileSync(join(dist, name)) });
    });
    await pg.goto('http://bench.local/index.html');
    await pg.waitForFunction(() => window.ok);
    res.push(await pg.evaluate(async (m) => {
      const ak = window.ak;
      await ak.ready;
      ak.engine.manual(true);
      const stage = document.getElementById('stage');
      let s = '';
      for (let i = 0; i < m; i++) {
        s += '<div class="m">';
        for (let k = 0; k < 12; k++) s += `<span class="i">item ${k}</span>`;
        s += '</div>';
      }
      stage.innerHTML = s;
      void document.body.offsetHeight;
      let t = performance.now();
      const list = [...document.querySelectorAll('.m')].map((el) => ak.marquee(el, { speed: 80 }));
      void document.body.offsetHeight;
      const setup = performance.now() - t;
      // Let the observers report (resize, visibility) before timing frames.
      for (let i = 0; i < 5; i++) await new Promise((r) => requestAnimationFrame(r));
      await new Promise((r) => setTimeout(r, 50));
      const setupAll = setup;
      for (let i = 0; i < 100; i++) { ak.engine.tick(16); void document.body.offsetHeight; }
      t = performance.now();
      for (let i = 0; i < 300; i++) { ak.engine.tick(16); void document.body.offsetHeight; }
      const frame = (performance.now() - t) / 300;
      if (!(list[0].offset > 0)) throw new Error('marquee did not move');
      return [setupAll, frame];
    }, m));
    await pg.close();
  }
  console.log(String(m).padStart(5), median(res.map((x) => x[0])).toFixed(2).padStart(9), median(res.map((x) => x[1])).toFixed(4).padStart(10));
}
await browser.close();
