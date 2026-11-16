// splitText() and flip() in Chromium (Playwright), against GSAP's SplitText
// and Flip. Every timed region ends with a forced layout (`offsetHeight`), so
// the layout the library causes is counted whether it reads it or not.
//
// - split: P paragraphs of ~60 words into lines, words and chars;
// - re-split: `split()` again on the same result (what autoSplit does on a
//   width change);
// - flip: N boxes, capture -> toggle a grid/list class -> start the animation.
//
// Median of RUNS fresh pages per case, ms (lower is better).
// Usage: node scripts/bench-text.mjs   (P=20,100 N=200,1000 RUNS=5 DIST=path/to/dist)
// PROFILE=split|flip SIZE=100 prints a CPU profile of one Anikit run instead.
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

const Ps = (process.env.P || '20,100').split(',').map(Number);
const Ns = (process.env.N || '200,1000').split(',').map(Number);
const RUNS = +(process.env.RUNS || 5);
const median = (a) => a.sort((x, y) => x - y)[a.length >> 1];

const WORDS = 'the quick brown fox jumps over a lazy dog while seven bright wizards quietly vex nimble jackdaws'.split(' ');
const html = `<!doctype html><meta charset="utf-8"><style>
body{margin:0;font:16px/1.4 sans-serif;width:900px} p{margin:0 0 8px}
#grid{display:flex;flex-wrap:wrap;gap:4px;width:900px} #grid.list{display:block}
#grid .b{width:40px;height:40px;background:#6ea8ff} #grid.list .b{width:300px;height:10px;margin:2px}
</style><div id="stage"></div>
<script src="gsap.min.js"></script><script src="SplitText.min.js"></script><script src="Flip.min.js"></script>
<script type="module">
import * as ak from './anikit.js';
window.ak = ak;
window.para = (p) => {
  const words = ${JSON.stringify(WORDS)};
  let s = '';
  for (let i = 0; i < p; i++) {
    s += '<p>';
    for (let w = 0; w < 60; w++) s += words[(i * 7 + w) % words.length] + (w % 13 === 5 ? ' <em>' + words[w % words.length] + '</em> ' : ' ');
    s += '</p>';
  }
  document.getElementById('stage').innerHTML = s;
  void document.body.offsetHeight;
};
window.boxes = (n) => {
  let s = '<div id="grid">';
  for (let i = 0; i < n; i++) s += '<div class="b"></div>';
  document.getElementById('stage').innerHTML = s + '</div>';
  void document.body.offsetHeight;
};
window.ok = true;
</script>`;

const browser = await playwright.chromium.launch();
async function page() {
  const pg = await browser.newPage({ viewport: { width: 1000, height: 800 } });
  await pg.route('http://bench.local/**', (route) => {
    const name = new URL(route.request().url()).pathname.slice(1);
    if (name === 'index.html') return route.fulfill({ status: 200, contentType: 'text/html', body: html });
    const f = name.endsWith('.min.js') ? join(gsapDir, name) : join(dist, name);
    route.fulfill({ status: 200, contentType: f.endsWith('.wasm') ? 'application/wasm' : 'text/javascript', body: readFileSync(f) });
  });
  await pg.goto('http://bench.local/index.html');
  await pg.waitForFunction(() => window.ok);
  await pg.evaluate(async () => {
    await window.ak.ready;
    window.gsap.registerPlugin(window.SplitText, window.Flip);
    window.ak.setWarnings(false);
  });
  return pg;
}

const CASES = {
  async split(pg, lib, p) {
    return pg.evaluate(([lib, p]) => {
      // Warm up the code paths on a small copy first.
      for (let i = 0; i < 3; i++) {
        window.para(2);
        const els = document.querySelectorAll('p');
        if (lib === 'ak') window.ak.splitText(els, { type: 'lines,words,chars' }).revert();
        else window.SplitText.create(els, { type: 'lines,words,chars' }).revert();
      }
      window.para(p);
      const els = document.querySelectorAll('p');
      let t = performance.now();
      const s = lib === 'ak' ? window.ak.splitText(els, { type: 'lines,words,chars' }) : window.SplitText.create(els, { type: 'lines,words,chars' });
      void document.body.offsetHeight;
      const split = performance.now() - t;
      if (s.chars.length < p * 200 || s.lines.length < p) throw new Error('split nothing');
      document.body.style.width = '700px';
      void document.body.offsetHeight;
      t = performance.now();
      if (lib === 'ak') s.split();
      else s.split({ type: 'lines,words,chars' });
      void document.body.offsetHeight;
      const resplit = performance.now() - t;
      document.body.style.width = '';
      s.revert();
      return [split, resplit];
    }, [lib, p]);
  },
  async flip(pg, lib, n) {
    return pg.evaluate(([lib, n]) => {
      const run = (k) => {
        window.boxes(k);
        const g = document.getElementById('grid');
        const t = performance.now();
        if (lib === 'ak') window.ak.flip('#grid .b', () => g.classList.toggle('list'), { duration: 0.5 });
        else {
          const st = window.Flip.getState('#grid .b');
          g.classList.toggle('list');
          window.Flip.from(st, { duration: 0.5 });
        }
        void document.body.offsetHeight;
        const ms = performance.now() - t;
        if (!document.querySelector('#grid .b').style.transform) throw new Error('flip did not start');
        return ms;
      };
      for (let i = 0; i < 3; i++) run(20);
      return [run(n)];
    }, [lib, n]);
  },
};

if (process.env.PROFILE) {
  const pg = await page();
  const cdp = await pg.context().newCDPSession(pg);
  await cdp.send('Profiler.enable');
  await cdp.send('Profiler.setSamplingInterval', { interval: 100 });
  await cdp.send('Profiler.start');
  await CASES[process.env.PROFILE](pg, 'ak', +(process.env.SIZE || 100));
  const { profile } = await cdp.send('Profiler.stop');
  const self = new Map();
  const byId = new Map(profile.nodes.map((n) => [n.id, n]));
  for (let i = 0; i < profile.samples.length; i++) {
    const n = byId.get(profile.samples[i]);
    const k = `${n.callFrame.functionName || '(anon)'} ${n.callFrame.url.split('/').pop()}:${n.callFrame.lineNumber}`;
    self.set(k, (self.get(k) || 0) + (profile.timeDeltas[i] || 0));
  }
  const tot = [...self.values()].reduce((x, y) => x + y, 0);
  for (const [k, v] of [...self].sort((x, y) => y[1] - x[1]).slice(0, 25)) console.log((100 * v / tot).toFixed(1).padStart(5), k);
  await browser.close();
  process.exit(0);
}

console.log(`median of ${RUNS} fresh pages, ms (lower is better)\n`);
console.log('case'.padEnd(10), 'size'.padStart(6), 'Anikit'.padStart(9), 'GSAP'.padStart(9), '  GSAP/Anikit');
const rows = [
  ...Ps.map((p) => ['split', p, 0]),
  ...Ps.map((p) => ['re-split', p, 1]),
  ...Ns.map((n) => ['flip', n, 0]),
];
const cache = new Map();
for (const [name, size, idx] of rows) {
  const key = `${name === 're-split' ? 'split' : name}:${size}`;
  if (!cache.has(key)) {
    const res = { ak: [], gs: [] };
    for (let r = 0; r < RUNS; r++) {
      for (const lib of ['ak', 'gs']) {
        const pg = await page();
        res[lib].push(await CASES[name === 're-split' ? 'split' : name](pg, lib, size));
        await pg.close();
      }
    }
    cache.set(key, res);
  }
  const res = cache.get(key);
  const A = median(res.ak.map((x) => x[idx]));
  const G = median(res.gs.map((x) => x[idx]));
  console.log(name.padEnd(10), String(size).padStart(6), A.toFixed(2).padStart(9), G.toFixed(2).padStart(9), `  ${(G / A).toFixed(2)}x`);
}
await browser.close();
