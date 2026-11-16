// A typical animated landing page built three times (bench/page/{anikit,gsap,
// motion}.js on bench/page/common.js): split-word hero reveal, 6 sections of
// staggered cards on scroll, a pinned scrubbed section, magnetic buttons and
// a marquee. Each is one minified bundle (anikit-lean: the lean build plus
// anikit.wasm streamed and compiled off the main thread), loaded in Chromium with the CPU
// throttled 4x (a mid-range phone). Measured per fresh page:
//   ready     ms from navigation start to the end of the page's set-up script
//   TBT       total blocking time (sum of long-task time over 50 ms) from
//             navigation through the scroll-through
//   long      number of long tasks
//   heap      JS heap after a forced GC at the end (MB)
//   p95/max   frame interval (ms) while scrolling top to bottom at 20 px per frame
//             with the pointer moving over the buttons first
//   busy      main-thread ms per frame (script + style + layout) during the scroll
// Median of RUNS pages. Usage: node scripts/bench-page.mjs (RUNS=5 CPU=4)
import { build } from 'esbuild';
import { createRequire } from 'node:module';
import { mkdtempSync, writeFileSync, readFileSync, rmSync, statSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { tmpdir } from 'node:os';
import { gzipSync } from 'node:zlib';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const pkg = join(here, '..');
const dir = mkdtempSync(join(tmpdir(), 'anikit-page-'));
const libs = (process.env.LIBS || 'anikit,anikit-lean,gsap,motion').split(',');
const RUNS = +(process.env.RUNS || 5);
const CPU = +(process.env.CPU || 4);
const size = {};
for (const lib of libs) {
  await build({ entryPoints: [join(pkg, `bench/page/${lib}.js`)], bundle: true, format: 'iife', minify: true, outfile: join(dir, `${lib}.js`), logLevel: 'error' });
  size[lib] = gzipSync(readFileSync(join(dir, `${lib}.js`))).length / 1024;
  // anikit-lean streams the WebAssembly core as a separate file.
  if (lib === 'anikit-lean') size[lib] += gzipSync(readFileSync(join(pkg, 'dist/anikit.wasm'))).length / 1024;
}
const require = createRequire(join(pkg, 'package.json'));
const { chromium } = require('playwright');
const browser = await chromium.launch();
const median = (a) => a.slice().sort((x, y) => x - y)[a.length >> 1];
const rows = {};
for (let r = 0; r < RUNS; r++) {
  for (const lib of libs) {
    const page = await browser.newPage({ viewport: { width: 1200, height: 800 } });
    const cdp = await page.context().newCDPSession(page);
    await cdp.send('Emulation.setCPUThrottlingRate', { rate: CPU });
    await cdp.send('Performance.enable');
    await page.addInitScript(() => {
      window.__long = [];
      new PerformanceObserver((l) => { for (const e of l.getEntries()) window.__long.push(e.duration); }).observe({ type: 'longtask', buffered: true });
    });
    await page.route('http://bench.local/**', (route) => {
      const name = new URL(route.request().url()).pathname.slice(1);
      if (name === 'index.html') return route.fulfill({ status: 200, contentType: 'text/html', body: `<!doctype html><meta charset="utf-8"><body><script src="${lib}.js"></script>` });
      if (name === 'anikit.wasm') return route.fulfill({ status: 200, contentType: 'application/wasm', body: readFileSync(join(pkg, 'dist/anikit.wasm')) });
      route.fulfill({ status: 200, contentType: 'text/javascript', body: readFileSync(join(dir, name)) });
    });
    await page.goto('http://bench.local/index.html');
    await page.waitForFunction(() => window.__ready, null, { timeout: 30000 });
    const ready = await page.evaluate(() => window.__ready);
    await page.waitForTimeout(1200); // hero reveal
    for (let i = 0; i < 20; i++) await page.mouse.move(300 + i * 30, 420 + (i % 3) * 10);
    const m = async () => Object.fromEntries((await cdp.send('Performance.getMetrics')).metrics.map((x) => [x.name, x.value]));
    const a = await m();
    const frames = await page.evaluate(() => new Promise((res) => {
      const out = [];
      let last = performance.now();
      const max = document.documentElement.scrollHeight - innerHeight;
      let y = 0;
      const f = (t) => {
        out.push(t - last);
        last = t;
        y += 20;
        window.scrollTo(0, y);
        if (y < max) requestAnimationFrame(f);
        else res(out);
      };
      requestAnimationFrame((t) => { last = t; requestAnimationFrame(f); });
    }));
    const b = await m();
    const busy = ((b.ScriptDuration - a.ScriptDuration) + (b.RecalcStyleDuration - a.RecalcStyleDuration) + (b.LayoutDuration - a.LayoutDuration)) * 1000 / frames.length;
    await page.waitForTimeout(300);
    const long = await page.evaluate(() => window.__long);
    await cdp.send('HeapProfiler.collectGarbage');
    const heap = (await cdp.send('Runtime.getHeapUsage')).usedSize / 1048576;
    const sorted = frames.slice(2).sort((x, y) => x - y);
    const row = (rows[lib] ??= { ready: [], tbt: [], long: [], heap: [], p95: [], max: [], busy: [] });
    row.ready.push(ready);
    row.tbt.push(long.reduce((s, d) => s + Math.max(0, d - 50), 0));
    row.long.push(long.length);
    row.heap.push(heap);
    row.p95.push(sorted[Math.floor(sorted.length * 0.95)]);
    row.max.push(sorted[sorted.length - 1]);
    row.busy.push(busy);
    await page.close();
  }
}
await browser.close();
rmSync(dir, { recursive: true, force: true });
console.log(`landing page, CPU ${CPU}x slower, median of ${RUNS} pages (lower is better)\n`);
console.log('lib'.padEnd(8), 'gzip KB'.padStart(8), 'ready ms'.padStart(9), 'TBT ms'.padStart(7), 'long'.padStart(5), 'heap MB'.padStart(8), 'p95 ms'.padStart(7), 'max ms'.padStart(7), 'busy ms'.padStart(8));
for (const lib of libs) {
  const x = rows[lib];
  console.log(lib.padEnd(8), size[lib].toFixed(1).padStart(8), median(x.ready).toFixed(0).padStart(9), median(x.tbt).toFixed(0).padStart(7), String(median(x.long)).padStart(5), median(x.heap).toFixed(2).padStart(8), median(x.p95).toFixed(1).padStart(7), median(x.max).toFixed(1).padStart(7), median(x.busy).toFixed(2).padStart(8));
}
