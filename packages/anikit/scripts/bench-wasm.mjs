// Core-in-WebAssembly micro-benchmarks (Node/V8, no JS write path): calls
// the `ak_*` exports of dist/anikit.wasm directly, so the numbers isolate the
// Rust core as the browser runs it. Each case warms up (V8 tiers wasm up
// lazily), then reports the median of RUNS timed batches.
// Usage: node scripts/bench-wasm.mjs [filter]   (build with scripts/build-wasm.mjs first;
// WASM=path/to/other.wasm benchmarks another build, e.g. a saved baseline for A/B runs)
import { readFileSync, existsSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const file = process.env.WASM || join(here, '..', 'dist/anikit.wasm');
if (!existsSync(file)) throw new Error('run `node scripts/build-wasm.mjs` first');
const { instance } = await WebAssembly.instantiate(readFileSync(file), {});
const w = instance.exports;
const filter = process.argv[2] || '';
const RUNS = 7;
const median = (a) => a.sort((x, y) => x - y)[a.length >> 1];

const f64 = (vals) => {
  const p = w.ak_f64_buf(vals.length);
  new Float64Array(w.memory.buffer, p, vals.length).set(vals);
};
const easing = (s) => {
  const b = new TextEncoder().encode(s);
  const p = w.ak_u8_buf(b.length);
  new Uint8Array(w.memory.buffer, p, b.length).set(b);
  return w.ak_easing_parse(b.length);
};

function bench(name, n, unit, setup, frame, warm = 120, iters = 60) {
  if (filter && !name.includes(filter)) return;
  const ctx = setup();
  for (let i = 0; i < warm; i++) frame(ctx);
  const runs = [];
  for (let r = 0; r < RUNS; r++) {
    const t = performance.now();
    for (let i = 0; i < iters; i++) frame(ctx);
    runs.push((performance.now() - t) / iters);
  }
  const ms = median(runs);
  console.log(`${name.padEnd(46)} ${String(n).padStart(8)} ${unit.padEnd(9)} ${ms.toFixed(3).padStart(8)} ms  ${((ms * 1e6) / n).toFixed(1).padStart(7)} ns/${unit.slice(0, -1).replace(/ie$/, 'y')}`);
  ctx?.done?.();
}

// --- Engine -----------------------------------------------------------------
// Encoding: delay, duration, iterations, direction, endDelay, timingEasing, flags, nTracks,
//   per track: kind, arg, channels, nKeyframes, hasOffsets, [offsets], values..., nEasings, easings...
const LONG = [0, 1e9, 1, 0, 0, -1, 3];
function anims(n, tracks) {
  const hs = [];
  for (let i = 0; i < n; i++) {
    const enc = [...LONG, tracks.length];
    for (const t of tracks) enc.push(...t(i));
    f64(enc);
    const h = w.ak_anim_create(enc.length);
    w.ak_play(h);
    hs.push(h);
  }
  return { done: () => { for (const h of hs) w.ak_release(h); w.ak_tick(0); } };
}
const tick = () => w.ak_tick(16);
const outCubic = easing('outCubic');
const inOutQuad = easing('inOutQuad');
const springE = easing('spring(200, 12)');
const bez = easing('cubic-bezier(0.25, 0.1, 0.25, 1)');
bench('tick: tween 1 number, outCubic', 100000, 'anims', () => anims(100000, [(i) => [0, 0, 1, 2, 0, 0, i, 1, outCubic]]), tick);
bench('tick: tween 1 number, cubic-bezier', 100000, 'anims', () => anims(100000, [(i) => [0, 0, 1, 2, 0, 0, i, 1, bez]]), tick);
bench('tick: x,y,opacity + OKLab colour', 50000, 'anims', () => anims(50000, [
  (i) => [0, 0, 2, 2, 0, 0, 0, i, 100, 1, inOutQuad],
  () => [0, 0, 1, 2, 0, 0, 1, 1, inOutQuad],
  () => [1, 2, 4, 2, 0, 255, 0, 0, 1, 0, 0, 255, 1, 0],
]), tick);
bench('tick: 8 keyframes, spring easing', 50000, 'anims', () => anims(50000, [() => [0, 0, 1, 8, 0, 0, 10, 20, 30, 40, 50, 60, 70, 1, springE]]), tick);

bench('springs: retarget 2 ch + tick (pointer follow)', 10000, 'springs', () => {
  const hs = [];
  for (let i = 0; i < 10000; i++) {
    f64([0, 0, 0, 0, i, 1]);
    hs.push(w.ak_spring_create(2, 170, 26, 1));
  }
  return { hs, k: 0, done: () => { for (const h of hs) w.ak_release(h); w.ak_tick(0); } };
}, (c) => {
  c.k++;
  for (let i = 0; i < c.hs.length; i++) {
    const p = w.ak_f64_buf(2) >>> 3;
    const F = new Float64Array(w.memory.buffer);
    F[p] = c.k + i;
    F[p + 1] = -c.k;
    w.ak_sim_target(c.hs[i], 2);
  }
  w.ak_tick(16);
});

// Instanced batch: [duration, iterations, direction, endDelay, easing, flags, count, ch, nkf,
//   hasOffsets, (offsets), nEases, eases..., hasDelays, (delays), values...]
for (const stagger of [false, true]) {
  const n = 100000;
  bench(`batch: 3 channels, outCubic${stagger ? ', staggered delays' : ''}`, n, 'instances', () => {
    const enc = [1000, 1, 0, 0, -1, 3, n, 3, 2, 0, 1, outCubic, stagger ? 1 : 0];
    if (stagger) for (let i = 0; i < n; i++) enc.push(i * 0.002);
    for (let i = 0; i < n; i++) enc.push(0, 0, 1, i, 300, 0);
    f64(enc);
    const h = w.ak_batch_create(enc.length);
    w.ak_play(h);
    return { h, t: 0, done: () => { w.ak_release(h); w.ak_tick(0); } };
  }, (c) => { w.ak_tick(c.t++ % 50 === 0 ? 0 : 1); });
}

// --- Physics ----------------------------------------------------------------
for (const [n, subs] of [[1000, 4], [5000, 4], [20000, 2]]) {
  bench(`world: ${subs} substeps (settled pile)`, n, 'bodies', () => {
    const side = 1600;
    const id = w.ak_world_create(0, 980, 0.05, subs, 2, 0.2, 0.3, 0.05);
    w.ak_world_bounds(id, 0, 0, side, side);
    const cols = Math.ceil(Math.sqrt(n));
    const sp = side / cols;
    const data = [];
    for (let i = 0; i < n; i++) {
      const r = sp * 0.3 * (0.8 + (0.4 * ((i * 7919) % 100)) / 100);
      data.push((i % cols) * sp + sp / 2, Math.floor(i / cols) * sp + sp / 2, r, 1);
    }
    f64(data);
    w.ak_world_add(id, n);
    for (let i = 0; i < 120; i++) w.ak_world_step(id, 1 / 60);
    return { id, done: () => w.ak_world_free(id) };
  }, (c) => w.ak_world_step(c.id, 1 / 60), 60, 30);
}

// --- Particles ----------------------------------------------------------------
function cloud(n) {
  const id = w.ak_cloud_create(n, 2, 3);
  let p = w.ak_cloud_shape(id, 0);
  let s = new Float32Array(w.memory.buffer, p, n * 3);
  for (let i = 0; i < n * 3; i++) s[i] = ((i * 7919) % 600) - 300;
  p = w.ak_cloud_shape(id, 1);
  s = new Float32Array(w.memory.buffer, p, n * 3);
  for (let i = 0; i < n * 3; i++) s[i] = ((i * 104729) % 500) - 250;
  w.ak_cloud_snap(id, 0);
  return id;
}
// perspective(40°, 1.6, 10, 6000) · lookAt([0,0,1000] → origin), column-major.
const f = 1 / Math.tan((40 * Math.PI) / 360);
const nf = 1 / (10 - 6000);
const VP = [f / 1.6, 0, 0, 0, 0, f, 0, 0, 0, 0, 6010 * nf, -1, 0, 0, 2 * 6000 * 10 * nf - 1000 * 6010 * nf, 1000];
const I = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1];
for (const n of (process.env.CLOUD_N || '150000').split(',').map(Number)) {
  bench('cloud: step', n, 'particles', () => ({ id: cloud(n) }), (c) => w.ak_cloud_step(c.id, 1 / 60, 0, 1, 0.5));
  for (const sort of [0, 1]) {
    bench(`project${sort ? ' + depth sort' : ''}`, n, 'points', () => {
      const id = cloud(n);
      w.ak_cloud_step(id, 1 / 60, 0, 1, 0.5);
      return { id, pid: w.ak_projector_create() };
    }, (c) => { f64([...I, ...VP]); w.ak_project(c.pid, 2, c.id, 3, 0, 1, 2, n, 1440, 900, sort); });
  }
}
for (const [n, W, H, scale] of [[150000, 1080, 675, 0.75], [120000, 1920, 1080, 1.0]]) {
  const setup = () => {
    const id = cloud(n);
    for (let i = 0; i < 30; i++) w.ak_cloud_step(id, 1 / 60, 0, 1, 0.5);
    const pid = w.ak_projector_create();
    f64([...I, ...VP]);
    w.ak_project(pid, 2, id, 3, 0, 1, 2, n, W / scale, H / scale, 0);
    const sid = w.ak_splat_create(W, H);
    const params = [1.15, 0.3, 700, 2600, scale, 700, 400, 150, 46, 5];
    return { pid, sid, params, done: () => { w.ak_splat_free(sid); w.ak_cloud_free(id); } };
  };
  bench(`splat: draw ${W}x${H}`, n, 'points', setup, (c) => { f64(c.params); w.ak_splat_draw(c.sid, c.pid, n); });
  bench(`splat: draw + resolve(trail 0.5) ${W}x${H}`, W * H, 'pixels', setup, (c) => { f64(c.params); w.ak_splat_draw(c.sid, c.pid, n); w.ak_splat_resolve(c.sid, 1, 0x07080b, 0.5); });
}
bench('emitter: step (turbulence + attractor)', 100000, 'particles', () => {
  const id = w.ak_emitter_create(100000, 7);
  f64([0, 1e9, 0, 0, 0, 0, 1, 200, 0, 0, 0, 1, 0, 200, 0.4, 0.6, 0, -300, 0, 0.4, 80, 0.01, 0, 0, 0, 2000, 1, 0]);
  w.ak_emitter_params(id);
  w.ak_emitter_burst(id, 100000);
  return { id, done: () => w.ak_emitter_free(id) };
}, (c) => w.ak_emitter_step(c.id, 1 / 60));
