// draggable() in Chromium (Playwright), against GSAP's Draggable: one box
// among N static siblings, pressed, then E synthetic pointermove events, each
// followed by a forced style + layout (`offsetHeight`), then released (with
// inertia for both). Reports µs per move event (handlers + the style and
// layout they cause) and ms for the release (the throw set-up). Median of
// RUNS fresh pages.
// FORCE=0 leaves out the forced layout per event (handler cost only; GSAP
// writes the transform on its next tick, Anikit inside the event).
// Usage: node scripts/bench-drag.mjs   (N=0,2000 E=500 RUNS=5 FORCE=0 DIST=path/to/dist)
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

const Ns = (process.env.N || '0,2000').split(',').map(Number);
const E = +(process.env.E || 500);
const FORCE = process.env.FORCE !== '0';
const RUNS = +(process.env.RUNS || 5);
const median = (a) => a.sort((x, y) => x - y)[a.length >> 1];
const html = `<!doctype html><meta charset="utf-8"><style>
body{margin:0} #box{position:absolute;left:100px;top:100px;width:80px;height:80px;background:#6ea8ff}
.s{display:inline-block;width:30px;height:30px;margin:2px;background:#eee}
</style><div id="stage"></div><div id="box"></div>
<script src="gsap.min.js"></script><script src="Draggable.min.js"></script><script src="InertiaPlugin.min.js"></script>
<script type="module">import * as ak from './anikit.js'; window.ak = ak; window.ok = true;</script>`;

const browser = await playwright.chromium.launch();
console.log(`median of ${RUNS} fresh pages, ${E} moves (lower is better)\n`);
console.log('siblings'.padStart(8), 'Anikit µs/move'.padStart(15), 'GSAP µs/move'.padStart(13), 'Anikit release ms'.padStart(18), 'GSAP release ms'.padStart(16));
for (const n of Ns) {
  const res = { ak: [], gs: [] };
  for (let r = 0; r < RUNS; r++) {
    for (const lib of ['ak', 'gs']) {
      const pg = await browser.newPage({ viewport: { width: 1000, height: 800 } });
      await pg.route('http://bench.local/**', (route) => {
        const name = new URL(route.request().url()).pathname.slice(1);
        if (name === 'index.html') return route.fulfill({ status: 200, contentType: 'text/html', body: html });
        const f = name.endsWith('.min.js') ? join(gsapDir, name) : join(dist, name);
        route.fulfill({ status: 200, contentType: 'text/javascript', body: readFileSync(f) });
      });
      await pg.goto('http://bench.local/index.html');
      await pg.waitForFunction(() => window.ok);
      res[lib].push(await pg.evaluate(async ([lib, n, E, force]) => {
        await window.ak.ready;
        window.ak.setWarnings(false);
        window.gsap.registerPlugin(window.Draggable, window.InertiaPlugin);
        document.getElementById('stage').innerHTML = '<span class="s"></span>'.repeat(n);
        const box = document.getElementById('box');
        if (lib === 'ak') window.ak.draggable(box, { inertia: true });
        else window.Draggable.create(box, { inertia: true });
        void document.body.offsetHeight;
        const ev = (type, x, y, t) => new PointerEvent(type, { clientX: x, clientY: y, pointerId: 1, pointerType: 'mouse', isPrimary: true, button: 0, buttons: type === 'pointerup' ? 0 : 1, bubbles: true, cancelable: true, timeStamp: t });
        const drag = (moves, timed) => {
          box.dispatchEvent(ev('pointerdown', 140, 140));
          let x = 140;
          const t0 = performance.now();
          for (let i = 0; i < moves; i++) {
            x += 1;
            (lib === 'ak' ? window : document).dispatchEvent(ev('pointermove', x, 140 + (i % 5)));
            if (force) void document.body.offsetHeight;
          }
          const t1 = performance.now();
          (lib === 'ak' ? window : document).dispatchEvent(ev('pointerup', x, 140));
          void document.body.offsetHeight;
          const t2 = performance.now();
          return timed ? [((t1 - t0) / moves) * 1000, t2 - t1] : null;
        };
        drag(100, false); // warm-up
        await new Promise((r) => setTimeout(r, 50));
        const out = drag(E, true);
        if (!box.style.transform || box.style.transform === 'none') throw new Error('box did not move');
        return out;
      }, [lib, n, E, FORCE]));
      await pg.close();
    }
  }
  const m = (lib, i) => median(res[lib].map((x) => x[i]));
  console.log(String(n).padStart(8), m('ak', 0).toFixed(1).padStart(15), m('gs', 0).toFixed(1).padStart(13), m('ak', 1).toFixed(2).padStart(18), m('gs', 1).toFixed(2).padStart(16));
}
await browser.close();
