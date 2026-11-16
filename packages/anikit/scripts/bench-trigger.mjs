// scrollTrigger set-up and refresh in Chromium (Playwright), against GSAP's
// ScrollTrigger: N sections, each with a box whose animation is scrubbed by a
// trigger on the section (every 10th section also pinned with PIN=1).
// - create: N animations + N triggers, then a forced layout;
// - refresh: `refreshScrollTriggers()` / `ScrollTrigger.refresh()` (what a
//   resize or load does), then a forced layout.
// Median of RUNS fresh pages, ms (lower is better).
// Usage: node scripts/bench-trigger.mjs   (N=100,500 PIN=1 RUNS=5 DIST=path/to/dist)
import { createRequire } from 'node:module';
import { readFileSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const pkg = join(here, '..');
const dist = resolve(process.env.DIST || join(pkg, 'dist'));
const gsapDir = join(pkg, 'node_modules/gsap/dist');
const require = createRequire(import.meta.url);
let playwright;
try { playwright = require('playwright'); } catch { playwright = require(join(process.execPath, '../../lib/node_modules/playwright')); }

const Ns = (process.env.N || '100,500').split(',').map(Number);
const RUNS = +(process.env.RUNS || 5);
const PIN = process.env.PIN === '1';
const median = (a) => a.sort((x, y) => x - y)[a.length >> 1];

const html = `<!doctype html><meta charset="utf-8"><style>
body{margin:0} section{height:60vh;position:relative;border-top:1px solid #ddd}
.b{position:absolute;left:20px;top:20px;width:40px;height:40px;background:#6ea8ff}
</style><div id="stage"></div>
<script src="gsap.min.js"></script><script src="ScrollTrigger.min.js"></script>
<script type="module">
import * as ak from './anikit.js';
window.ak = ak;
window.ok = true;
</script>`;

const browser = await playwright.chromium.launch();
async function page() {
  const pg = await browser.newPage({ viewport: { width: 1000, height: 800 } });
  await pg.route('http://bench.local/**', (route) => {
    const name = new URL(route.request().url()).pathname.slice(1);
    if (name === 'index.html') return route.fulfill({ status: 200, contentType: 'text/html', body: html });
    const f = name.endsWith('.min.js') ? join(gsapDir, name) : join(dist, name);
    route.fulfill({ status: 200, contentType: 'text/javascript', body: readFileSync(f) });
  });
  await pg.goto('http://bench.local/index.html');
  await pg.waitForFunction(() => window.ok);
  await pg.evaluate(async () => {
    await window.ak.ready;
    window.gsap.registerPlugin(window.ScrollTrigger);
    window.ak.setWarnings(false);
    window.ak.engine.manual(true);
  });
  return pg;
}

async function run(pg, lib, n) {
  return pg.evaluate(([lib, n, pin]) => {
    const stage = document.getElementById('stage');
    const make = (k) => {
      stage.innerHTML = '';
      const out = [];
      for (let i = 0; i < k; i++) {
        const s = document.createElement('section');
        const b = document.createElement('div');
        b.className = 'b';
        s.appendChild(b);
        stage.appendChild(s);
        out.push([s, b]);
      }
      void document.body.offsetHeight;
      return out;
    };
    const create = (pairs) => {
      const list = [];
      pairs.forEach(([s, b], i) => {
        const p = pin && i % 10 === 5;
        if (lib === 'ak') {
          const anim = window.ak.animate(b, { x: [0, 300], rotate: [0, 90], opacity: [1, 0.5] }, { duration: 1, ease: 'linear', autoplay: false });
          list.push(window.ak.scrollTrigger({ trigger: s, start: 'top bottom', end: 'bottom top', scrub: true, animation: anim, pin: p }));
        } else {
          const anim = window.gsap.fromTo(b, { x: 0, rotate: 0, opacity: 1 }, { x: 300, rotate: 90, opacity: 0.5, duration: 1, ease: 'none', paused: true });
          list.push(window.ScrollTrigger.create({ trigger: s, start: 'top bottom', end: 'bottom top', scrub: true, animation: anim, pin: p }));
        }
      });
      return list;
    };
    const refresh = () => (lib === 'ak' ? window.ak.refreshScrollTriggers() : window.ScrollTrigger.refresh());
    // Warm-up on a small page.
    for (let i = 0; i < 3; i++) {
      const l = create(make(20));
      refresh();
      for (const t of l) t.kill();
    }
    window.scrollTo(0, 0);
    const pairs = make(n);
    let t = performance.now();
    const list = create(pairs);
    void document.body.offsetHeight;
    const c = performance.now() - t;
    window.scrollTo(0, 2000);
    void document.body.offsetHeight;
    t = performance.now();
    refresh();
    void document.body.offsetHeight;
    const r = performance.now() - t;
    const st = list[n >> 1];
    if (!(st.end > st.start) || !(st.start > 0)) throw new Error('trigger not measured');
    return [c, r];
  }, [lib, n, PIN]);
}

console.log(`median of ${RUNS} fresh pages, ms (lower is better)${PIN ? ', every 10th section pinned' : ''}\n`);
console.log('case'.padEnd(10), 'N'.padStart(6), 'Anikit'.padStart(9), 'GSAP'.padStart(9), '  GSAP/Anikit');
for (const n of Ns) {
  const res = { ak: [], gs: [] };
  for (let r = 0; r < RUNS; r++) {
    for (const lib of ['ak', 'gs']) {
      const pg = await page();
      res[lib].push(await run(pg, lib, n));
      await pg.close();
    }
  }
  ['create', 'refresh'].forEach((name, idx) => {
    const A = median(res.ak.map((x) => x[idx]));
    const G = median(res.gs.map((x) => x[idx]));
    console.log(name.padEnd(10), String(n).padStart(6), A.toFixed(2).padStart(9), G.toFixed(2).padStart(9), `  ${(G / A).toFixed(2)}x`);
  });
}
await browser.close();
