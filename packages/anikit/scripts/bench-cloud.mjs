// Point-cloud + glow benchmark in real Chromium (Playwright).
// Per frame: pointCloud step (springs + turbulence), projection, additive
// splat + tone map + putImageData at 1080×675 (a 1440×900 viewport at 0.75
// resolution). Usage: node scripts/bench-cloud.mjs   (SIZES=50000,150000)
import { createRequire } from 'node:module';
import { readFileSync, existsSync } from 'node:fs';
import { join, dirname, extname } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const pkg = join(here, '..');
const require = createRequire(import.meta.url);
let playwright;
try { playwright = require('playwright'); } catch { playwright = require(join(process.execPath, '../../lib/node_modules/playwright')); }
if (!existsSync(join(pkg, 'dist/anikit.js'))) throw new Error('run `node scripts/build.mjs` first');

const sizes = (process.env.SIZES || '10000,50000,150000,300000').split(',').map(Number);
const browser = await playwright.chromium.launch();
const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
const types = { '.js': 'text/javascript', '.wasm': 'application/wasm' };
await page.route('http://bench.local/**', (route) => {
  const path = new URL(route.request().url()).pathname;
  if (path === '/') return route.fulfill({ status: 200, contentType: 'text/html', body: '<canvas id="c" style="width:1440px;height:900px"></canvas>' });
  const f = join(pkg, path);
  if (!existsSync(f)) return route.fulfill({ status: 404, body: '' });
  route.fulfill({ status: 200, contentType: types[extname(f)] || 'application/octet-stream', body: readFileSync(f) });
});
await page.goto('http://bench.local/');
const rows = await page.evaluate(async (sizes) => {
  const A = await import('/dist/anikit.js');
  await A.ready;
  A.engine.manual(true);
  const out = [];
  for (const n of sizes) {
    const cloud = A.pointCloud(n, {
      autoStep: false,
      shapes: [(i) => [Math.cos(i) * 400, Math.sin(i * 1.3) * 250, Math.sin(i) * 150], (i) => [((i * 7919) % 900) - 450, ((i * 104729) % 500) - 250, 0]],
    });
    cloud.morph(1);
    cloud.mix = 0.5;
    const scene = A.scene3d({ fov: 40, camera: { z: 1000 } });
    const g = A.glow('#c', { resolution: 0.75, trail: 0.5, budget: 0 });
    g.resize(1440, 900);
    const time = (f) => {
      for (let i = 0; i < 120; i++) f(); // let V8 tier up
      const t = performance.now();
      for (let i = 0; i < 60; i++) f();
      return (performance.now() - t) / 60;
    };
    const step = time(() => cloud.step(1 / 60));
    const render = time(() => g.render(scene, cloud, { size: 1.15, intensity: 0.3, fog: [700, 2600], lens: { x: 700, y: 400 } }));
    out.push({ n, step, render, total: step + render });
    cloud.destroy();
    g.destroy();
  }
  return out;
}, sizes);
await browser.close();
console.log('per frame (ms; render = project + splat + tone map + putImageData at 1080×675)\n');
console.log('particles     step    render    total   frame budget used @60fps');
for (const r of rows) {
  console.log(`${String(r.n).padStart(9)}  ${r.step.toFixed(2).padStart(7)}  ${r.render.toFixed(2).padStart(8)}  ${r.total.toFixed(2).padStart(7)}   ${Math.round((r.total / 16.67) * 100)}%`);
}
