// A landscape film frame (WebAssembly, in Node): a 401×401 textured terrain
// (320k triangles, 4096² albedo), a coarse far terrain, a mirror lake,
// instanced trees, an emissive textured sky dome, camera motion blur with
// `prevCamera`, `depth()` read-back and the film grade, along a low flight.
// Prints ms per renderer3d pass (from r3.stats) and per full frame.
//   node scripts/bench-terrain.mjs [--frames 24] [--trees 12000] [--hash] [--aniso] [--order near] [--only sky,far,terrain,trees,water]
// --aniso turns on anisotropic filtering for the terrain textures.
// --hash prints an FNV hash of every rendered frame (colour and depth), to
// check that an optimisation leaves the output bit-identical.
import { performance } from 'node:perf_hooks';
// DIST=path/to/dist benchmarks another build (a saved baseline, for interleaved A/B runs).
const A = await import(process.env.DIST ? new URL(`file://${process.env.DIST.replace(/\/$/, '')}/anikit.js`).href : '../dist/anikit.js');
await A.ready;
const arg = (k, d) => { const i = process.argv.indexOf(k); return i > 0 ? +process.argv[i + 1] : d; };
const FRAMES = arg('--frames', 24), TREES = arg('--trees', 12000), HASH = process.argv.includes('--hash');
const textureFilter = process.argv.includes('--aniso') ? 'aniso' : undefined;
const W = 1920, H = 1080;

// Deterministic value noise.
const hash2 = (x, y) => { let h = (x * 374761393 + y * 668265263) | 0; h = Math.imul(h ^ (h >>> 13), 1274126177); return ((h ^ (h >>> 16)) >>> 0) / 4294967296; };
const smooth = (t) => t * t * (3 - 2 * t);
const noise = (x, y) => {
  const xi = Math.floor(x), yi = Math.floor(y), tx = smooth(x - xi), ty = smooth(y - yi);
  const a = hash2(xi, yi), b = hash2(xi + 1, yi), c = hash2(xi, yi + 1), d = hash2(xi + 1, yi + 1);
  return a + (b - a) * tx + (c - a) * ty + (a - b - c + d) * tx * ty;
};
const fbm = (x, y) => { let s = 0, a = 1, f = 1; for (let o = 0; o < 5; o++) { s += a * noise(x * f, y * f); a *= 0.5; f *= 2.03; } return s / 1.94; };

// Terrain: 401×401 vertices over 24 km, heights −500…1900 around a lake at y = 0.
const SIZE = 24000, N = 401;
const height = (x, z) => (fbm(x / 5200 + 7.3, z / 5200 + 1.9) - 0.42) * 2600 + Math.max(0, Math.abs(x) / SIZE - 0.2) * 3000;
function grid(n, size, h, uvRepeat) {
  const pos = new Float32Array(n * n * 3), nrm = new Float32Array(n * n * 3), uv = new Float32Array(n * n * 2), idx = new Uint32Array((n - 1) * (n - 1) * 6);
  const step = size / (n - 1);
  for (let j = 0; j < n; j++) for (let i = 0; i < n; i++) {
    const x = -size / 2 + i * step, z = -size / 2 + j * step, k = j * n + i;
    pos.set([x, h(x, z), z], k * 3);
    const dx = h(x + step, z) - h(x - step, z), dz = h(x, z + step) - h(x, z - step), l = Math.hypot(dx, 2 * step, dz);
    nrm.set([-dx / l, (2 * step) / l, -dz / l], k * 3);
    uv.set([(i / (n - 1)) * uvRepeat, (j / (n - 1)) * uvRepeat], k * 2);
  }
  let t = 0;
  for (let j = 0; j < n - 1; j++) for (let i = 0; i < n - 1; i++) {
    const a = j * n + i, b = a + 1, c = a + n, d = c + 1;
    idx.set([a, c, b, b, c, d], t); t += 6;
  }
  return A.mesh.custom(pos, nrm, idx, uv);
}
let t0 = performance.now();
const terrain = grid(N, SIZE, height, 1);
// Coarse far terrain: mountains on a 120 km ring, sunk under the near terrain.
const FAR = 120000;
const farTerrain = grid(101, FAR, (x, z) => { const r = Math.max(Math.abs(x), Math.abs(z)); return r < SIZE * 0.55 ? -900 : (fbm(x / 9000, z / 9000) - 0.3) * 9000 * Math.min(1, (r - SIZE * 0.55) / 15000); }, 6);
const tGeo = performance.now() - t0;

// 4096² albedo (grass / rock / sand bands with detail) and a 4096×2048 sky.
t0 = performance.now();
function makeTexture(w, h, fn) {
  const data = new Uint8ClampedArray(w * h * 4);
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) { const k = (y * w + x) * 4; fn(x, y, data, k); data[k + 3] = 255; }
  return A.texture({ width: w, height: h, data });
}
const albedo = makeTexture(4096, 4096, (x, y, d, k) => {
  const n = hash2(x, y) * 0.35 + noise(x / 37, y / 37) * 0.65, band = noise(x / 300, y / 300);
  d[k] = (70 + 90 * band) * (0.6 + 0.6 * n); d[k + 1] = (95 + 40 * band) * (0.6 + 0.6 * n); d[k + 2] = (45 + 50 * band) * (0.6 + 0.5 * n);
});
const skyTex = makeTexture(4096, 2048, (x, y, d, k) => {
  const v = y / 2048, cl = noise(x / 160, y / 60) * Math.max(0, 0.5 - Math.abs(v - 0.35)) * 2;
  d[k] = 255 * Math.min(1, 0.25 + v * 0.9 + cl * 0.6); d[k + 1] = 255 * Math.min(1, 0.4 + v * 0.6 + cl * 0.5); d[k + 2] = 255 * Math.min(1, 0.75 + v * 0.2 + cl * 0.3);
});
const tTex = performance.now() - t0;

// Trees: cones (24 sides + base, 48 triangles), on land only.
function cone(sides) {
  const pos = [0, 1, 0], nrm = [0, 1, 0], uv = [0.5, 0], idx = [];
  for (let s = 0; s <= sides; s++) {
    const a = (s / sides) * Math.PI * 2, c = Math.cos(a), sn = Math.sin(a), l = Math.hypot(1, 0.35);
    pos.push(c * 0.35, 0, sn * 0.35, c * 0.35, 0, sn * 0.35);
    nrm.push(c / l, 0.35 / l, sn / l, 0, -1, 0);
    uv.push(s / sides, 1, s / sides, 1);
  }
  const base = pos.length / 3;
  pos.push(0, 0, 0); nrm.push(0, -1, 0); uv.push(0.5, 1);
  for (let s = 0; s < sides; s++) { const a = 1 + s * 2, b = a + 2; idx.push(0, b, a, base, a + 1, b + 1); }
  return A.mesh.custom(pos, nrm, idx, uv);
}
let seed = 12345;
const rnd = () => ((seed = (Math.imul(seed, 1103515245) + 12345) >>> 0) / 4294967296);
const spots = [];
while (spots.length < TREES) {
  const x = (rnd() - 0.5) * SIZE * 0.95, z = (rnd() - 0.5) * SIZE * 0.95, y = height(x, z);
  if (y > 40 && y < 1300) spots.push([x, y, z]);
}
const trees = A.instances(TREES, (i) => { const [x, y, z] = spots[i], s = 30 + (i % 7) * 6; return { x, y: y - 4, z, sx: s, sz: s, sy: s * 3.2, ry: i * 37, color: [0.12, 0.25 + (i % 5) * 0.03, 0.1] }; });

const NAMES = ['sky', 'far', 'terrain', 'trees', 'water'];
const only = process.argv.includes('--only') ? process.argv[process.argv.indexOf('--only') + 1].split(',') : NAMES;
const items = [
  { mesh: A.mesh.sphere(32, 48), instances: A.instances(1, () => ({ scale: 150000 })), cull: false, material: { ambient: 0, diffuse: 0, specular: 0, rim: 0, texture: skyTex, textureMode: 'emissive', fog: 0 } },
  { mesh: farTerrain, instances: A.instances(1, () => ({ color: [0.6, 0.62, 0.66] })), material: { texture: albedo, textureFilter, specular: 0.05, rim: 0.1 } },
  { mesh: terrain, material: { texture: albedo, textureFilter, specular: 0.1, shininess: 20, rim: 0.15 } },
  { mesh: cone(24), instances: trees, material: { specular: 0.15, rim: 0.2 } },
  { mesh: A.mesh.plane(1), instances: A.instances(1, () => ({ y: 0, sx: FAR, sz: FAR, color: [0.03, 0.06, 0.08] })), mirror: true, material: { specular: 1.2, shininess: 90, rim: 0.4 } },
].filter((_, i) => only.includes(NAMES[i]));
// --order near: opaque items nearest first (trees, terrain, far, sky) instead of sky first.
if (process.argv.includes('--order') && process.argv[process.argv.indexOf('--order') + 1] === 'near') items.reverse();
const env = { background: '#8fb4d8', fog: [3000, 70000], fogColor: [0.62, 0.72, 0.85], sky: [0.35, 0.5, 0.8], horizon: [0.9, 0.85, 0.8], ground: [0.1, 0.12, 0.1], key: [0.5, 0.45, -0.6], keyColor: [1.2, 1.05, 0.9], mirror: { y: 0, strength: 0.6 }, motionBlur: { shutter: 0.5 } };
console.log(`setup: geometry ${tGeo.toFixed(0)} ms, textures ${tTex.toFixed(0)} ms; terrain ${terrain.triangles} triangles, far ${farTerrain.triangles}, ${TREES} trees`);

// A low flight over the lake towards the hills, banking slightly.
const camAt = (f, c = A.scene3d({ fov: 55, width: W, height: H, near: 5, far: 400000 })) => {
  const s = f / 60, x = -6000 + s * 9000, z = 8000 - s * 13000;
  c.camera = { x, y: Math.max(height(x, z), 0) + 260 + 120 * Math.sin(s * 3), z };
  c.target = { x: x + 1500 + 600 * Math.sin(s * 2), y: c.camera.y - 380, z: z - 4000 };
  return c;
};
const r3 = A.renderer3d({ width: W, height: H });
const r3b = A.renderer3d({ width: W, height: H });
const fnv = (u8) => { let h = 0x811c9dc5; for (let i = 0; i < u8.length; i++) h = Math.imul(h ^ u8[i], 16777619); return (h >>> 0).toString(16).padStart(8, '0'); };

const sum = {};
let total = 0, depthMs = 0;
const cam = A.scene3d({ fov: 55, width: W, height: H, near: 5, far: 400000 });
const warm = 3;
const hashes = [];
for (let f = -warm; f < FRAMES; f++) {
  camAt(f, cam);
  const prevCamera = camAt(f - 1).viewProjection().matrix;
  const t = performance.now();
  const px = r3.render(cam, items, { ...env, prevCamera });
  const t1 = performance.now();
  const dist = r3.depth(cam);
  const t2 = performance.now();
  if (HASH && f >= 0) hashes.push(`${fnv(px)}/${fnv(new Uint8Array(dist.buffer, dist.byteOffset, dist.byteLength))}`);
  if (f < 0) continue;
  total += t1 - t;
  depthMs += t2 - t1;
  for (const [k, v] of Object.entries(r3.stats)) sum[k] = (sum[k] ?? 0) + v;
}
const pad = (s) => s.padEnd(30);
console.log(`${pad('render (one renderer3d)')} ${(total / FRAMES).toFixed(1).padStart(6)} ms/frame  (${r3.triangles} triangles)`);
console.log('  per pass: ' + Object.entries(sum).filter(([, v]) => v / FRAMES >= 0.05).map(([k, v]) => `${k} ${(v / FRAMES).toFixed(1)}`).join(' · ') + ' ms');
console.log(`${pad('depth(cam) read-back')} ${(depthMs / FRAMES).toFixed(1).padStart(6)} ms`);
if (HASH) console.log('hash ' + fnv(new TextEncoder().encode(hashes.join(','))) + '  (' + hashes.slice(0, 3).join(' ') + ' …)');

// The full film frame: a composition whose scene renders the terrain, reads
// depth and goes through the film grade (two renderers during a transition).
const post = { aa: 1, bloom: 0.5, bloomThreshold: 0.75, bloomRadius: 40, rays: 0.5, flares: 0.3, vignette: 0.35, grain: 2.5, contrast: 1.05, saturation: 1.08 };
const cv = { width: W, height: H, getContext: () => ({ fillRect() {}, drawImage() {}, putImageData() {}, save() {}, restore() {}, setTransform() {}, clearRect() {} }) };
for (const [name, two] of [['full frame (render + grade)', false], ['transition (2 renders + grade)', true]]) {
  const c = A.composition({ width: W, height: H, fps: 30, duration: (FRAMES + warm) / 30, post });
  let px = null, f = 0;
  const camB = A.scene3d({ fov: 55, width: W, height: H, near: 5, far: 400000 });
  c.scene('s', 0, c.duration, { draw() {
    camAt(f, cam);
    const prevCamera = camAt(f - 1).viewProjection().matrix;
    // The second renderer first: its render may grow memory and detach earlier views.
    if (two) { camAt(f + 200, camB); r3b.render(camB, items, { ...env, prevCamera: camAt(f + 199).viewProjection().matrix }); }
    px = r3.render(cam, items, { ...env, prevCamera });
    r3.depth(cam);
    f++;
  } });
  let n = 0, ts = 0;
  const tm = { draw: 0, read: 0, post: 0 };
  await c.render({ canvas: cv, preroll: 0, readPixels: () => px, onFrame: (fr) => {
    if (++n === warm) ts = performance.now();
    if (n > warm) for (const k in tm) tm[k] += fr.timing[k];
  } });
  const k = n - warm;
  console.log(`${pad(name)} ${((performance.now() - ts) / k).toFixed(1).padStart(6)} ms/frame  (draw ${(tm.draw / k).toFixed(1)} · read ${(tm.read / k).toFixed(1)} · grade ${(tm.post / k).toFixed(1)} ms)`);
}
