// Gaussian splat renderer in Chromium: frame time (draw + GPU finish) and
// depth-sort time for procedural sets of increasing size.
// Usage: node scripts/bench-splats.mjs [--gpu] [--sort-only]  (needs dist/: node scripts/build.mjs)   (default: SwiftShader, a
// software rasteriser: use the numbers as a ranking, not as GPU throughput)
import { build } from 'esbuild';
import { createRequire } from 'node:module';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const pkg = join(dirname(fileURLToPath(import.meta.url)), '..');
const { chromium } = createRequire(join(pkg, 'package.json'))('playwright');
const gpu = process.argv.includes('--gpu');

// 1. The core's depth sort on its own (Node, same wasm the worker runs).
{
  const { readFileSync } = await import('node:fs');
  const { instance } = await WebAssembly.instantiate(readFileSync(join(pkg, 'dist/anikit.wasm')), {});
  const W = instance.exports;
  for (const n of [100_000, 300_000, 1_000_000]) {
    const id = W.ak_gs_sorter_create();
    const ptr = W.ak_gs_sorter_centers(id, n, 0);
    const c = new Float32Array(W.memory.buffer, ptr, n * 3);
    for (let i = 0; i < c.length; i++) c[i] = Math.random() * 2 - 1;
    for (let i = 0; i < 20; i++) W.ak_gs_sort(id, 0.3, 0.2, 0.9, -3, 0); // warm up
    const t = [];
    for (let i = 0; i < 30; i++) { const t0 = performance.now(); W.ak_gs_sort(id, Math.sin(i), 0.2, Math.cos(i), -3, 0); t.push(performance.now() - t0); }
    t.sort((a, b) => a - b);
    console.log(`sort ${String(n).padStart(9)} splats: ${t[15].toFixed(2)} ms (median, Node)`);
    W.ak_gs_sorter_free(id);
  }
}
if (process.argv.includes('--sort-only')) process.exit(0);
// 2. Rendering in Chromium.
const res = await build({
  stdin: { contents: "import * as ak from './src/index'; window.ak = ak;", resolveDir: pkg, loader: 'ts' },
  bundle: true, format: 'iife', write: false, logLevel: 'error',
});
const browser = await chromium.launch({ args: gpu ? ['--enable-gpu', '--ignore-gpu-blocklist'] : ['--use-gl=angle', '--use-angle=swiftshader', '--enable-unsafe-swiftshader'] });
const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
await page.route('http://bench.local/**', (r) => r.fulfill({
  status: 200, contentType: 'text/html',
  body: `<!doctype html><body style="margin:0"><canvas id="c" style="width:1280px;height:720px"></canvas><script>${res.outputFiles[0].text}</script>`,
}));
await page.goto('http://bench.local/');
await page.evaluate((s) => { window.SPLAT_SIZE = s; return window.ak.ready; }, process.env.SPLAT_SIZE ?? null);
console.log(`renderer: ${await page.evaluate(() => { const gl = document.createElement('canvas').getContext('webgl2'); const e = gl.getExtension('WEBGL_debug_renderer_info'); return e ? gl.getParameter(e.UNMASKED_RENDERER_WEBGL) : 'unknown'; })}`);
for (const n of (process.env.SIZES ?? "100000,300000,1000000").split(",").map(Number)) {
  const r = await page.evaluate(async (n) => {
    const { splats } = window.ak;
    // Random points in a ball: no surface search, so setup stays quick.
    const pts = new Float32Array(n * 3);
    let s = 1;
    const rnd = () => ((s = (s * 1664525 + 1013904223) >>> 0) / 4294967296) * 2 - 1;
    for (let i = 0; i < n; i++) { let x, y, z; do { x = rnd(); y = rnd(); z = rnd(); } while (x * x + y * y + z * z > 1); pts.set([x, y, z], i * 3); }
    const set = splats.fromPoints(pts, { size: +(window.SPLAT_SIZE ?? 0.006), color: (i, x, y, z) => [0.5 + x / 2, 0.5 + y / 2, 0.5 + z / 2] });
    const layer = splats('#c', set, { autoRender: false, pixelRatio: 1, camera: { z: 2.6 } });
    const gl = layer.gl;
    const px = new Uint8Array(4);
    const frame = () => { layer.render(); gl.readPixels(0, 0, 1, 1, gl.RGBA, gl.UNSIGNED_BYTE, px); };
    layer.render();
    await layer.sorted;
    for (let i = 0; i < 5; i++) frame();
    const t0 = performance.now();
    const F = 20;
    for (let i = 0; i < F; i++) { layer.transform.rotateY = i * 3; frame(); }
    const ms = (performance.now() - t0) / F;
    // Sort cost: wait for a few worker sorts while the view keeps moving.
    const sorts = [];
    for (let i = 0; i < 6; i++) {
      const k = layer.stats.sorts;
      layer.transform.rotateY = 90 + i * 10;
      layer.render();
      while (layer.stats.sorts === k) await new Promise((r) => setTimeout(r, 2));
      sorts.push(layer.stats.sortMs);
    }
    layer.kill();
    sorts.sort((a, b) => a - b);
    return { ms, sort: sorts[sorts.length >> 1], worker: layer.stats.worker };
  }, n);
  console.log(`${String(n).padStart(9)} splats: ${r.ms.toFixed(1)} ms/frame (${(1000 / r.ms).toFixed(0)} fps), sort ${r.sort.toFixed(1)} ms in ${r.worker ? 'worker' : 'main thread'}`);
}
await browser.close();
