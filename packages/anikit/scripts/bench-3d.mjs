// 3D projection + painter's depth sort: Anikit (WASM: projection + radix sort)
// vs an optimised plain-JS implementation (typed arrays + index sort).
import { scene3d, ready, engine } from '../dist/anikit.js';
await ready;
engine.manual(true);
const median = (a) => a.sort((x, y) => x - y)[a.length >> 1];
const time = (fn, reps = 20) => {
  for (let i = 0; i < 3; i++) fn(i);
  const r = [];
  for (let k = 0; k < 5; k++) {
    const t = performance.now();
    for (let i = 0; i < reps; i++) fn(i);
    r.push((performance.now() - t) / reps);
  }
  return median(r);
};
console.log('points    Anikit (WASM)    plain JS     speed-up');
for (const n of [10_000, 100_000, 500_000]) {
  const pts = new Float32Array(n * 3);
  for (let i = 0; i < pts.length; i++) pts[i] = Math.random() * 400 - 200;
  const scene = scene3d({ width: 1280, height: 720, fov: 50 });
  const ak = time((i) => {
    scene.rotation.y = i;
    scene.project(pts);
  });
  // Plain JS: rotation about Y + perspective + sort by depth.
  const out = new Float32Array(n * 4);
  const idx = new Uint32Array(n);
  const f = 1 / Math.tan((50 * Math.PI) / 360);
  const js = time((i) => {
    const a = (i * Math.PI) / 180;
    const c = Math.cos(a), s = Math.sin(a);
    for (let k = 0; k < n; k++) {
      const x = pts[k * 3], y = pts[k * 3 + 1], z = pts[k * 3 + 2];
      const rx = c * x + s * z, rz = -s * x + c * z;
      const w = 800 - rz;
      out[k * 4] = 640 + (rx * f * 360) / w;
      out[k * 4 + 1] = 360 - (y * f * 360) / w;
      out[k * 4 + 2] = w;
      out[k * 4 + 3] = (f * 360) / w;
      idx[k] = k;
    }
    idx.sort((p, q) => out[q * 4 + 2] - out[p * 4 + 2]);
  }, n > 100000 ? 3 : 20);
  console.log(`${String(n).padStart(7)}   ${ak.toFixed(3).padStart(9)} ms   ${js.toFixed(3).padStart(9)} ms   ${(js / ak).toFixed(1).padStart(6)}x`);
  scene.destroy();
}
