// Scroll-linked updates in Chromium (Playwright): N sections, each with a box
// whose animation follows the scroll position, through scrollTrigger (scrub)
// or scroll(). Reports ms per scroll event spent in the page's 'scroll'
// listeners (reads, progress, style writes and any layout they force),
// median of 3 runs x 200 events after warm-up.
// Usage: node scripts/bench-scroll.mjs   (SIZES=10,50,200 MODES=trigger,scroll)
import { build } from 'esbuild';
import { createRequire } from 'node:module';
import { mkdtempSync, writeFileSync, readFileSync, rmSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { tmpdir } from 'node:os';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const pkg = join(here, '..');
const dir = mkdtempSync(join(tmpdir(), 'anikit-scroll-'));
await build({ entryPoints: [join(pkg, 'bench/scroll-entry.js')], bundle: true, format: 'iife', minify: true, outfile: join(dir, 'bundle.js'), logLevel: 'error' });
writeFileSync(join(dir, 'index.html'), '<!doctype html><meta charset="utf-8"><style>body{margin:0} section{height:60vh;position:relative;border-top:1px solid #ddd} .b{position:absolute;left:20px;top:20px;width:40px;height:40px;background:#6ea8ff}</style><div id="stage"></div><script src="bundle.js"></script>');
const require = createRequire(import.meta.url);
let playwright;
try { playwright = require('playwright'); } catch { playwright = require(join(process.execPath, '../../lib/node_modules/playwright')); }

const sizes = (process.env.SIZES || '10,50,200').split(',').map(Number);
const modes = (process.env.MODES || 'trigger,scroll').split(',');
const browser = await playwright.chromium.launch();
console.log('ms per scroll event in the scroll listeners (lower is better)\n\nmode        N    ms/event');
for (const mode of modes) {
  for (const n of sizes) {
    const page = await browser.newPage({ viewport: { width: 1000, height: 800 } });
    await page.route('http://bench.local/**', (route) => {
      const f = join(dir, new URL(route.request().url()).pathname);
      route.fulfill({ status: 200, contentType: f.endsWith('.js') ? 'text/javascript' : 'text/html', body: readFileSync(f) });
    });
    await page.goto(`http://bench.local/index.html?mode=${mode}&n=${n}`);
    await page.evaluate(() => window.start());
    const runs = [];
    for (let r = 0; r < 3; r++) runs.push(await page.evaluate(() => window.run(200, 37)));
    runs.sort((a, b) => a - b);
    console.log(`${mode.padEnd(8)} ${String(n).padStart(5)}   ${runs[1].toFixed(3).padStart(8)}`);
    await page.close();
  }
}
await browser.close();
rmSync(dir, { recursive: true, force: true });
