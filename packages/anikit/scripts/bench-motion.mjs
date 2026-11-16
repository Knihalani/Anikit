// Anikit vs GSAP vs Motion in Chromium (Playwright), real rAF frames.
// Cases: `stagger` (N boxes, staggered opacity + y entrance), `spring` (N boxes
// retargeted to a new x every 100 ms on a spring; GSAP has no spring physics,
// so it gets elastic.out with overwrite), `scroll` (N scroll-linked sections,
// the page scrolled 23 px per frame; Motion's scroll() and anikit-compositor
// use a native ScrollTimeline where they can).
// Reports set-up ms and main-thread ms per frame (script + style + layout, from
// Chrome's Performance metrics) over the window, median of RUNS fresh pages.
// Usage: node scripts/bench-motion.mjs   (CASES=stagger,spring,scroll RUNS=3)
import { build } from 'esbuild';
import { createRequire } from 'node:module';
import { mkdtempSync, writeFileSync, readFileSync, rmSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { tmpdir } from 'node:os';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const pkg = process.env.PKG || join(here, '..');
const dir = mkdtempSync(join(tmpdir(), 'anikit-motion-'));
await build({ entryPoints: [join(pkg, 'bench/motion-entry.js')], bundle: true, format: 'iife', minify: true, outfile: join(dir, 'b.js'), logLevel: 'error', nodePaths: [join(here, '../node_modules')] });
writeFileSync(join(dir, 'index.html'), '<!doctype html><meta charset="utf-8"><style>body{margin:0} .box{display:inline-block;width:14px;height:14px;margin:1px;background:#6ea8ff} section{height:60vh;position:relative;border-top:1px solid #ddd} .b{position:absolute;left:20px;top:20px;width:40px;height:40px;background:#6ea8ff}</style><div id="stage"></div><script src="b.js"></script>');
const require = createRequire(join(here, '../package.json'));
const { chromium } = require('playwright');
const RUNS = +(process.env.RUNS || 3);
const CASES = (process.env.CASES || 'stagger,spring,scroll').split(',');
const plan = {
  stagger: { sizes: [1000], libs: ['anikit', 'gsap', 'motion'], ms: 1500 },
  spring: { sizes: [200, 1000], libs: ['anikit', 'gsap', 'motion'], ms: 1500 },
  scroll: { sizes: [50, 200], libs: ['anikit', 'anikit-compositor', 'gsap', 'motion'], ms: 1500 },
};
const median = (a) => a.slice().sort((x, y) => x - y)[a.length >> 1];
const browser = await chromium.launch();
console.log(`median of ${RUNS} pages; set-up ms and main-thread ms per frame (lower is better)\n`);
console.log('case'.padEnd(8), 'N'.padStart(5), 'lib'.padEnd(18), 'set-up'.padStart(8), 'ms/frame'.padStart(9));
for (const c of CASES) {
  for (const n of plan[c].sizes) {
    for (const lib of plan[c].libs) {
      const setups = [], frames = [];
      for (let r = 0; r < RUNS; r++) {
        const page = await browser.newPage({ viewport: { width: 1000, height: 800 } });
        await page.route('http://bench.local/**', (route) => {
          const f = join(dir, new URL(route.request().url()).pathname);
          route.fulfill({ status: 200, contentType: f.endsWith('.js') ? 'text/javascript' : 'text/html', body: readFileSync(f) });
        });
        await page.goto(`http://bench.local/index.html?lib=${lib}&case=${c}&n=${n}`);
        const cdp = await page.context().newCDPSession(page);
        await cdp.send('Performance.enable');
        setups.push(await page.evaluate(() => window.setup()));
        await page.evaluate(() => window.drive(200)); // warm-up
        const m = async () => Object.fromEntries((await cdp.send('Performance.getMetrics')).metrics.map((x) => [x.name, x.value]));
        const a = await m();
        const fa = await page.evaluate(() => new Promise((r) => { let k = 0; const t0 = performance.now(); const f = () => (performance.now() - t0 < 1500 ? (k++, requestAnimationFrame(f)) : r(k)); requestAnimationFrame(f); window.drive(1500); }));
        const b = await m();
        const busy = (b.ScriptDuration - a.ScriptDuration) + (b.RecalcStyleDuration - a.RecalcStyleDuration) + (b.LayoutDuration - a.LayoutDuration);
        frames.push((busy * 1000) / Math.max(1, fa));
        if ((await page.evaluate(() => window.check())) < 2 && c !== 'stagger') throw new Error(`${lib} ${c}: nothing moved`);
        await page.close();
      }
      console.log(c.padEnd(8), String(n).padStart(5), lib.padEnd(18), median(setups).toFixed(2).padStart(8), median(frames).toFixed(3).padStart(9));
    }
  }
}
await browser.close();
rmSync(dir, { recursive: true, force: true });
