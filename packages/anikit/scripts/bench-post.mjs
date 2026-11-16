// Film-grade cost per 1080p frame in WebAssembly (in Node): each post effect on
// its own and the full grade used by film/film.mjs, on a rendered 3D frame
// (neon towers, a sun, a mirror floor — bright highlights for bloom and flares).
//   node scripts/bench-post.mjs [--hash]   (--hash: FNV-1a of the last graded frame per case, to check output identity)
import { performance } from 'node:perf_hooks';
// DIST=path/to/dist benchmarks another build (a saved baseline, for interleaved A/B runs).
const A = await import(process.env.DIST ? new URL(`file://${process.env.DIST.replace(/\/$/, '')}/anikit.js`).href : '../dist/anikit.js');
await A.ready;
const W = 1920, H = 1080;
// Input: one rendered frame.
const r3 = A.renderer3d({ width: W, height: H });
const cam = A.scene3d({ fov: 60, width: W, height: H, near: 8, far: 40000, camera: { x: 0, y: 120, z: 1400 }, target: { x: 0, y: 200, z: -4000 } });
const towers = A.instances(400, (i) => { const side = i % 2 ? 1 : -1, z = -Math.floor(i / 2) * 150, h = 300 + ((i * 7919) % 97) * 15; return { x: side * (350 + ((i * 104729) % 900)), y: -300 + h / 2, z, sx: 110, sy: h, sz: 110, color: [0.03, 0.035, 0.05] }; });
const floor = A.instances(1, () => ({ y: -300, z: -12000, sx: 14000, sz: 30000, color: [0.02, 0.02, 0.03] }));
const px = Uint8ClampedArray.from(r3.render(cam, [
  { mesh: A.mesh.plane(1), instances: floor, mirror: true, material: { lines: 1.2, lineColor: [0.3, 0.5, 1.5], grid: [60, 150], lineWidth: 0.03 } },
  { mesh: A.mesh.cube(), instances: towers, material: { lines: 1.4, pattern: 'hex', lineColor: [1.6, 0.3, 0.9], grid: [3, 12], lineWidth: 0.04 } },
], { background: '#03040a', fog: [7000, 24000], sky: [0.03, 0.03, 0.1], horizon: [0.95, 0.35, 0.7], ground: [0.01, 0.01, 0.02], key: [0, 0.2, -1], keyColor: [1, 0.75, 0.9], skybox: { sun: 2 }, mirror: { y: -300, strength: 0.45 } }));

const film = { aa: 1, bloom: 0.85, bloomThreshold: 0.5, bloomRadius: 46, streaks: 0.45, streakLength: 300, rays: 0.9, flares: 0.6, zoomBlur: 0.1, aberration: 0.5, vignette: 0.42, grain: 3.5, contrast: 1.07, saturation: 1.12, shadows: [0, 0.012, 0.035], highlights: [0.05, 0.015, -0.02] };
const cases = {
  'none (read + write)': undefined, 'aa (FXAA)': { aa: 1 }, bloom: { bloom: 0.85, bloomThreshold: 0.5 }, streaks: { streaks: 0.45, bloomThreshold: 0.5 },
  rays: { rays: 0.9, bloomThreshold: 0.5 }, 'lens flares': { flares: 0.6, bloomThreshold: 0.5 }, 'zoom blur': { zoomBlur: 0.1 }, aberration: { aberration: 0.5 },
  'grade + vignette + grain': { contrast: 1.07, saturation: 1.12, vignette: 0.42, grain: 3.5 }, 'full film grade': film,
};
const cv = { width: W, height: H, getContext: () => ({ fillRect() {}, drawImage() {}, putImageData() {} }) };
let base = 0;
const HASH = process.argv.includes('--hash');
for (const [name, post] of Object.entries(cases)) {
  const c = A.composition({ width: W, height: H, fps: 30, duration: 2, post });
  c.scene('s', 0, 2, { draw() {} });
  let n = 0, t0 = 0, hash = '';
  const warm = 15;
  await c.render({ canvas: cv, readPixels: () => px, onFrame: (fr) => {
    if (++n === warm) t0 = performance.now();
    if (HASH && n === 60) { const b = fr.rgba(); let h = 2166136261; for (let i = 0; i < b.length; i++) h = Math.imul(h ^ b[i], 16777619); hash = `  hash ${(h >>> 0).toString(16).padStart(8, '0')}`; }
  } });
  const ms = (performance.now() - t0) / (n - warm);
  if (!post) base = ms;
  console.log(`${name.padEnd(26)} ${(post ? ms - base : ms).toFixed(1).padStart(6)} ms${post ? '' : ' (baseline, subtracted below)'}${hash}`);
}
