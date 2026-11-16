// Real-browser DOM benchmark (Chromium via Playwright). For each library and
// size, animates N boxes for a few seconds and reports per-frame main-thread
// costs from Chrome's own metrics (script / style recalc / layout).
// Usage: node scripts/bench-dom.mjs
import { build } from 'esbuild';
import { createRequire } from 'node:module';
import { readFileSync, existsSync } from 'node:fs';
import { join, dirname, extname } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const pkg = join(here, '..');
await build({ entryPoints: [join(pkg, 'bench/dom-entry.js')], bundle: true, format: 'iife', minify: process.env.BENCH_MINIFY !== '0', outfile: join(pkg, 'bench/dom-bundle.js'), logLevel: 'error' });
const require = createRequire(import.meta.url);
let playwright;
try { playwright = require('playwright'); } catch { playwright = require(join(process.execPath, '../../lib/node_modules/playwright')); }

// MANUAL=1: scripted frames instead of rAF (anikit and GSAP only): the time
// of the library's frame function alone, median of 3 x 100 frames after
// warm-up, style recalc forced between frames outside the timed region.
// Much less noisy than the rAF metrics, which also move with the frame rate.
const MANUAL = process.env.MANUAL === '1';
const libs = (process.env.LIBS || (MANUAL ? 'anikit,gsap' : 'anikit,anikit-compositor,gsap,motion')).split(',');
const sizes = (process.env.SIZES || '500,2000,5000').split(',').map(Number);
const SECONDS = 3;
const browser = await playwright.chromium.launch();
const types = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm' };
if (MANUAL) console.log('scripted frames: ms per frame in the library\'s frame function (lower is better)\n\nlib                   N    frame ms');
else {
  console.log(`per-frame main-thread time over ${SECONDS}s (ms; lower is better)\n`);
  console.log('lib                   N      script   style+layout   fps');
}
for (const n of sizes) {
  for (const lib of libs) {
    const page = await browser.newPage({ viewport: { width: 1100, height: 900 } });
    await page.route('http://bench.local/**', (route) => {
      const f = join(pkg, new URL(route.request().url()).pathname);
      if (!existsSync(f)) return route.fulfill({ status: 404, body: '' });
      route.fulfill({ status: 200, contentType: types[extname(f)] || 'application/octet-stream', body: readFileSync(f) });
    });
    await page.goto(`http://bench.local/bench/dom.html?lib=${lib}&n=${n}&typed=${lib === 'anikit-typed' ? 1 : 0}`);
    if (MANUAL) {
      await page.evaluate(() => window.start());
      const ms = await page.evaluate(() => window.manualFrames(100));
      const readyAt = await page.evaluate(() => window.readyAt);
      console.log(`${lib.padEnd(20)} ${String(n).padStart(5)}   ${ms.toFixed(3).padStart(8)}${lib.startsWith('anikit') ? `   (core ready ${readyAt.toFixed(1)} ms after navigation)` : ''}`);
      await page.close();
      continue;
    }
    const cdp = await page.context().newCDPSession(page);
    await cdp.send('Performance.enable');
    await page.evaluate(() => window.start());
    await page.waitForTimeout(500); // warm-up
    const get = async () => Object.fromEntries((await cdp.send('Performance.getMetrics')).metrics.map((m) => [m.name, m.value]));
    const a = await get();
    const frames = await page.evaluate((s) => new Promise((res) => {
      let f = 0;
      const t0 = performance.now();
      const loop = () => { f++; if (performance.now() - t0 < s * 1000) requestAnimationFrame(loop); else res(f); };
      requestAnimationFrame(loop);
    }), SECONDS);
    const b = await get();
    const per = (k) => ((b[k] - a[k]) * 1000) / frames;
    const script = per('ScriptDuration');
    const sl = per('RecalcStyleDuration') + per('LayoutDuration');
    console.log(`${lib.padEnd(20)} ${String(n).padStart(5)}   ${script.toFixed(2).padStart(7)}   ${sl.toFixed(2).padStart(10)}   ${(frames / SECONDS).toFixed(0).padStart(5)}`);
    await page.close();
  }
}
await browser.close();
