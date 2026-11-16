// Prototype B: a living splat sculpture.
// The particles live in anikit's pointCloud() (WebAssembly springs + morphs);
// this file wires scroll, pointer and captions to it and draws one WebGL canvas.
import { ready, engine, pointCloud, animate, timeline, stagger, set, htmlCanvas } from '../../packages/anikit/dist/anikit.js';
import { SplatRenderer } from './splat-gl.js';
import * as S from './shapes.js';

const q = new URLSearchParams(location.search);
const PERF = q.has('perf');
const reduced = matchMedia('(prefers-reduced-motion: reduce)').matches;
const N = q.has('n') ? +q.get('n') : 131072;
const STAGES = 6;
const DUST = 6; // shape index of the waiting fog

const T = (window.__t = []);
const mark = (l, t0) => T.push(l + ' ' + Math.round(performance.now() - t0));
const canvas = document.getElementById('stage');
const caps = [...document.querySelectorAll('.cap')];

// ---------------------------------------------------------------------------
// Palettes per form, linear-ish RGB triples (a → b by height, c accents).
const hex = (h) => [parseInt(h.slice(1, 3), 16) / 255, parseInt(h.slice(3, 5), 16) / 255, parseInt(h.slice(5, 7), 16) / 255];
const PAL = [
  ['#f2553f', '#6a4cf0', '#ffb93a'], // lettering
  ['#e8306f', '#ff9a55', '#ffe08a'], // flower
  ['#2c48c8', '#1ba7b9', '#ff7a59'], // flock
  ['#155fd6', '#38c9d1', '#ffffff'], // wave
  ['#5233d1', '#ff6aa8', '#ffcf63'], // galaxy
  ['#262741', '#6a4cf0', '#f2553f'], // install
  ['#9fb4e8', '#e8b9d6', '#ffffff'], // dust
].map((p) => p.map(hex));

// Per-form motion feel. Flower is bouncier: that is the spring stage.
const FEEL = [
  { stiffness: 36, damping: 6.5, spread: 0.7, noise: 2.2, swirl: 70, frequency: 0.008 },
  { stiffness: 22, damping: 3.6, spread: 0.85, noise: 2.0, swirl: 40, frequency: 0.008 },
  { stiffness: 30, damping: 6, spread: 0.9, noise: 3.5, swirl: 90, frequency: 0.006 },
  { stiffness: 34, damping: 7, spread: 0.7, noise: 2.5, swirl: 60, frequency: 0.01 },
  { stiffness: 30, damping: 6, spread: 0.8, noise: 3, swirl: 110, frequency: 0.005 },
  { stiffness: 38, damping: 7, spread: 0.6, noise: 1.6, swirl: 60, frequency: 0.008 },
];

// ---------------------------------------------------------------------------
// Small matrix helpers (in place, column-major).
function perspective(o, fovy, aspect, near, far) {
  const f = 1 / Math.tan(fovy / 2), nf = 1 / (near - far);
  o.fill(0);
  o[0] = f / aspect; o[5] = f; o[10] = (far + near) * nf; o[11] = -1; o[14] = 2 * far * near * nf;
}
function lookAt(o, ex, ey, ez, tx, ty, tz, basis) {
  let fx = tx - ex, fy = ty - ey, fz = tz - ez;
  let l = Math.hypot(fx, fy, fz); fx /= l; fy /= l; fz /= l;
  let rx = fy * 0 - fz * 1, ry = fz * 0 - fx * 0, rz = fx * 1 - fy * 0; // f × up(0,1,0)
  l = Math.hypot(rx, ry, rz); rx /= l; ry /= l; rz /= l;
  const ux = ry * fz - rz * fy, uy = rz * fx - rx * fz, uz = rx * fy - ry * fx;
  o[0] = rx; o[1] = ux; o[2] = -fx; o[3] = 0;
  o[4] = ry; o[5] = uy; o[6] = -fy; o[7] = 0;
  o[8] = rz; o[9] = uz; o[10] = -fz; o[11] = 0;
  o[12] = -(rx * ex + ry * ey + rz * ez);
  o[13] = -(ux * ex + uy * ey + uz * ez);
  o[14] = fx * ex + fy * ey + fz * ez;
  o[15] = 1;
  basis[0] = rx; basis[1] = ry; basis[2] = rz;
  basis[3] = ux; basis[4] = uy; basis[5] = uz;
  basis[6] = fx; basis[7] = fy; basis[8] = fz;
}
function mul4(o, a, b) {
  for (let c = 0; c < 4; c++) for (let r = 0; r < 4; r++)
    o[c * 4 + r] = a[r] * b[c * 4] + a[4 + r] * b[c * 4 + 1] + a[8 + r] * b[c * 4 + 2] + a[12 + r] * b[c * 4 + 3];
}
const smooth = (a, b, x) => { const t = Math.min(1, Math.max(0, (x - a) / (b - a))); return t * t * (3 - 2 * t); };

// ---------------------------------------------------------------------------
let gl;
try {
  const t0 = performance.now();
  if (!q.has('nogl')) gl = new SplatRenderer(canvas, N);
  mark('gl@' + Math.round(t0), t0);
} catch (e) {
  document.documentElement.classList.add('no-gl');
}

await ready;
await Promise.race([document.fonts.load('900 100px Display'), new Promise((r) => setTimeout(r, 700))]);

const narrow = () => innerWidth < 700;
const yieldTask = () => new Promise((r) => setTimeout(r, 0));
await yieldTask();
const cloud = pointCloud(N, { ...FEEL[0] });
// shape 0..5 are the forms; DUST holds the waiting fog
cloud.shape(DUST, S.dust(N));
await yieldTask();
{ const t0 = performance.now(); cloud.shape(0, S.lettering(N, 'anikit', 'Display', 900, 1000)); mark('letter', t0); }
cloud.snap(DUST);

// The other forms are built one per frame after first paint, so no long task.
// Each step stays well under 50 ms; a form is ready when its last step lands.
const gal = new Float32Array(N * 3);
const builders = [
  ...S.flowerSteps(N, (xyz) => cloud.shape(1, xyz)),
  () => cloud.shape(2, S.flock(N)),
  () => cloud.shape(3, S.wave(N)),
  () => S.galaxy(N, gal, 0, N >> 1),
  () => cloud.shape(4, S.galaxy(N, gal, N >> 1, N)),
  () => cloud.shape(5, S.lettering(N, 'npm i anikit', 'Mono', 400, 1000, 110)),
];
const FORM_AT = [0, 4, 5, 6, 8, 9]; // builders done when form k is ready
let built = 0;
function buildNext() {
  if (!builders.length) return;
  const t0 = performance.now();
  builders.shift()();
  mark('build' + built, t0);
  built++;
  setTimeout(buildNext, 30);
}

// ---------------------------------------------------------------------------
// State, all preallocated.
const st = {
  s: 0,             // displayed scroll stage (springs toward target)
  intro: reduced ? 1 : 0,
  glow: 0, shadow: 0, swell: 0, cut: N,
  spin: 0, sway: 0,
  px: 0.5, py: 0.5, // pointer, 0..1
  pxs: 0.5, pys: 0.5,
  pdx: 0, pdy: 0, moved: false, inside: false,
  ox: 0, oy: 0, js: 0, noDraw: false, vs: 0,
};
const f = {
  viewProj: new Float32Array(16), model: new Float32Array(9),
  a: new Float32Array(3), b: new Float32Array(3), c: new Float32Array(3),
  focal: 1, radial: 0, time: 0, floor: -400, wave: 0, size: 2.2, alpha: 0.5, cut: N, glow: 0, shadow: 0, dpr: 1,
};
const proj = new Float32Array(16), view = new Float32Array(16), basis = new Float32Array(9);
const eye = new Float32Array(3);
const FOV = 0.62;

// ---------------------------------------------------------------------------
// Quality governor: degrades glow, then shadow, then splat count, then resolution.
const governor = {
  level: q.has('q') ? +q.get('q') : 4, locked: q.has('q'),
  dprScale: 1, ring: new Float32Array(120), sorted: new Float32Array(120), n: 0, i: 0,
  coolUntil: 0, goodSince: 0, budget: 16.7,
  p50: 0, p95: 0, max: 0,
};
function applyLevel() {
  const L = governor.level;
  animate(st, { glow: L >= 4 ? 1 : 0, shadow: L >= 3 ? 1 : 0, cut: L >= 2 ? N : N * 0.6 }, { duration: 0.9, ease: 'inOutSine' });
  const want = L >= 1 ? 1 : 0.7;
  if (want !== governor.dprScale) { const t0 = performance.now(); governor.dprScale = want; resize(); mark('resize@' + Math.round(t0), t0); }
}
function sample(dt, now) {
  const g = governor;
  g.ring[g.i] = dt; g.i = (g.i + 1) % g.ring.length; g.n = Math.min(g.n + 1, g.ring.length);
  if (g.i % 30 !== 0 || g.n < 60) return;
  g.sorted.set(g.ring);
  const v = g.sorted.subarray(0, g.n).sort();
  g.p50 = v[g.n >> 1]; g.p95 = v[Math.floor(g.n * 0.95)]; g.max = v[g.n - 1];
  // refresh interval: the fastest typical frame
  g.budget = Math.max(6.9, Math.min(16.7, v[Math.floor(g.n * 0.1)] * 1.02));
  if (g.locked || now < g.coolUntil) return;
  if (g.p95 > g.budget * 1.4 && g.level > 0) {
    g.level--; g.coolUntil = now + 2500; g.goodSince = now; applyLevel();
  } else if (g.p95 < g.budget * 1.1) {
    if (now - g.goodSince > 6000 && g.level < 4) { g.level++; g.coolUntil = now + 4000; g.goodSince = now; applyLevel(); }
  } else g.goodSince = now;
}

// ---------------------------------------------------------------------------
let cssW = 0, cssH = 0;
function resize() {
  cssW = innerWidth; cssH = innerHeight;
  const dpr = Math.min(devicePixelRatio || 1, 2) * governor.dprScale;
  f.dpr = dpr;
  gl?.resize(Math.round(cssW * dpr), Math.round(cssH * dpr));
}
addEventListener('resize', resize);
resize();

// Scroll → stage, smoothed by a spring so wheel steps never jump.
const target = { s: 0 };
function onScroll() {
  const max = document.documentElement.scrollHeight - innerHeight;
  const t = max > 0 ? (scrollY / max) * (STAGES - 1) : 0;
  if (Math.abs(t - target.s) < 1e-4) return;
  target.s = t;
}
addEventListener('scroll', onScroll, { passive: true });

// Pointer: a brush along the view ray.
addEventListener('pointermove', (e) => {
  const x = e.clientX / cssW, y = e.clientY / cssH;
  st.pdx += x - st.px; st.pdy += y - st.py;
  st.px = x; st.py = y; st.moved = true; st.inside = true;
}, { passive: true });
document.addEventListener('pointerleave', () => { st.inside = false; });
const ray = new Float32Array(6); // model-space origin, direction
function pointerRay(nx, ny) {
  // nx, ny in NDC, with the lens shift removed
  const tanH = Math.tan(FOV / 2), aspect = cssW / cssH;
  const sx = (nx - st.ox) * tanH * aspect, sy = (ny - st.oy) * tanH;
  let dx = basis[6] + basis[0] * sx + basis[3] * sy;
  let dy = basis[7] + basis[1] * sx + basis[4] * sy;
  let dz = basis[8] + basis[2] * sx + basis[5] * sy;
  const l = Math.hypot(dx, dy, dz); dx /= l; dy /= l; dz /= l;
  const m = f.model; // model-space = Mᵀ · world (rotation only)
  ray[0] = m[0] * eye[0] + m[1] * eye[1] + m[2] * eye[2];
  ray[1] = m[3] * eye[0] + m[4] * eye[1] + m[5] * eye[2];
  ray[2] = m[6] * eye[0] + m[7] * eye[1] + m[8] * eye[2];
  ray[3] = m[0] * dx + m[1] * dy + m[2] * dz;
  ray[4] = m[3] * dx + m[4] * dy + m[5] * dz;
  ray[5] = m[6] * dx + m[7] * dy + m[8] * dz;
}
function worldToModel(o, x, y, z) {
  const m = f.model;
  o[0] = m[0] * x + m[1] * y + m[2] * z;
  o[1] = m[3] * x + m[4] * y + m[5] * z;
  o[2] = m[6] * x + m[7] * y + m[8] * z;
}
const sweep = new Float32Array(3);
let camDist = 1500;

addEventListener('pointerdown', (e) => {
  if (e.target.closest('a,button')) return;
  pointerRay((e.clientX / cssW) * 2 - 1, 1 - (e.clientY / cssH) * 2);
  const t = -(ray[0] * ray[3] + ray[1] * ray[4] + ray[2] * ray[5]);
  cloud.explode(ray[0] + ray[3] * t, ray[1] + ray[4] * t, ray[2] + ray[5] * t, reduced ? 300 : 1500);
  // let the burst bloom: springs loosen, then tighten back in two steps
  const base = FEEL[feelStage];
  cloud.set({ stiffness: base.stiffness * 0.35, damping: base.damping * 0.35 });
  clearTimeout(burstT);
  burstT = setTimeout(() => {
    cloud.set({ stiffness: base.stiffness * 0.7, damping: base.damping * 0.7 });
    burstT = setTimeout(() => cloud.set(FEEL[feelStage]), 700);
  }, 650);
});
let burstT = 0;

// ---------------------------------------------------------------------------
// Captions: one visible at a time; lines enter with a stagger.
let shown = -1;
// Each caption is rasterised into the splat context once it has fully entered
// (htmlCanvas → a texture in our WebGL2 context); on exit it dissolves as splats.
const capFx = caps.map(() => ({ hc: null, ready: false, x: 0, y: 0, w: 0, h: 0, cols: 0, rows: 0, cell: 1, p: 1, live: false }));
const capDraw = { tex: null, x: 0, y: 0, w: 0, h: 0, cols: 0, rows: 0, p: 0, time: 0, cell: 1, tint: null };
function rasterCaption(k) {
  const fx = capFx[k];
  if (!gl || fx.hc || shown !== k) return;
  const el = caps[k], r = el.getBoundingClientRect(), dpr = f.dpr;
  if (r.width < 4 || r.height < 4) return;
  const t0 = performance.now();
  fx.hc = htmlCanvas(el, { target: gl.gl, live: false, scale: Math.min(dpr, 2) });
  mark('raster@' + Math.round(t0), t0);
  fx.hc.ready.then(() => { fx.ready = true; }, () => {});
  fx.x = r.left * dpr; fx.y = r.top * dpr; fx.w = r.width * dpr; fx.h = r.height * dpr;
  const cellCss = 1.5;
  fx.cols = Math.min(420, Math.ceil(r.width / cellCss));
  fx.rows = Math.min(200, Math.ceil(r.height / cellCss));
  fx.cell = (r.width / fx.cols) * dpr * 0.75;
}
function dropRasters() {
  for (const fx of capFx) { fx.hc?.dispose?.(); fx.hc = null; fx.ready = false; fx.live = false; }
}
addEventListener('resize', dropRasters);

function showCaption(k) {
  if (k === shown) return;
  const prev = caps[shown], pfx = capFx[shown];
  shown = k;
  if (prev) {
    const p = prev;
    if (pfx.ready && pfx.hc?.texture && !reduced) {
      // swap the DOM caption for its splat twin, then let it drift apart
      set(p, { opacity: 0 });
      p.classList.remove('on');
      pfx.live = true;
      animate(pfx, { p: [0, 1] }, { duration: 1.5, ease: 'linear', onComplete: () => { pfx.live = false; } });
    } else {
      animate(p, { opacity: 0, y: -10, filter: 'blur(6px)' }, { duration: 0.35, ease: 'inQuad', onComplete: () => { if (caps[shown] !== p) p.classList.remove('on'); } });
    }
  }
  const c = caps[k];
  c.classList.add('on');
  set(c, { opacity: 1, y: 0, filter: 'blur(0px)' });
  animate(c.children, { opacity: [0, 1], y: [14, 0], filter: ['blur(8px)', 'blur(0px)'] }, {
    duration: 0.7, ease: 'outCubic', delay: stagger(0.08, { start: prev ? 0.35 : 0 }),
    onComplete: () => setTimeout(() => rasterCaption(k), 120),
  });
}

// The swell: a looping timeline, faded in and out with the wave stage.
const sea = { swell: 0 };
const swellTl = timeline({ repeat: Infinity, yoyo: true, autoplay: false })
  .add(sea, { swell: 64 }, { duration: 1.8, ease: 'inOutSine' })
  .add(sea, { swell: 24 }, { duration: 1.4, ease: 'inOutSine' });
let feelStage = 0;

// Copy button.
document.querySelector('.install')?.addEventListener('click', async (e) => {
  const b = e.currentTarget, label = b.querySelector('.copy');
  try { await navigator.clipboard.writeText('npm i anikit'); label.textContent = 'copied'; } catch { label.textContent = 'select'; }
  animate(b, { scale: [0.96, 1] }, { ease: 'spring(bounce: 0.4, duration: 0.5)' });
  setTimeout(() => { label.textContent = 'copy'; }, 1600);
});

// ---------------------------------------------------------------------------
// Perf overlay.
const perfEl = document.querySelector('.perf');
if (PERF) perfEl.hidden = false;
let perfTick = 0;

// ---------------------------------------------------------------------------
let positions = cloud.positions;
let last = 0;
function frame(time, dtMs) {
  const t0 = performance.now();
  frameBody(time, dtMs);
  st.js = performance.now() - t0;
}
function frameBody(time, dtMs) {
  const dt = Math.min(dtMs, 50) / 1000;
  const now = performance.now();
  if (last) sample(now - last, now);
  last = now;
  f.time = time / 1000;

  // --- stage blend: scroll position followed by a damped spring (k 70, c 17),
  // integrated here so scrolling allocates nothing
  for (let h = dt, step; h > 0; h -= step) {
    step = Math.min(h, 1 / 120);
    st.vs += ((target.s - st.s) * 70 - st.vs * 17) * step;
    st.s += st.vs * step;
  }
  const s = Math.min(STAGES - 1, Math.max(0, st.s));
  const k = Math.min(STAGES - 1, Math.floor(s));
  const frac = s - k;
  const mix = smooth(0.18, 0.82, frac);
  // never aim at a form that is not built yet (only in the first ~0.3 s)
  let avail = 0;
  while (avail < STAGES - 1 && built >= FORM_AT[avail + 1]) avail++;
  if (st.intro < 1) {
    cloud.from = DUST; cloud.to = Math.min(k, avail); cloud.mix = st.intro;
  } else {
    cloud.from = Math.min(k, avail); cloud.to = Math.min(STAGES - 1, k + 1, avail); cloud.mix = mix;
  }
  const near = Math.round(s);
  const ns = Math.min(STAGES - 1, Math.max(0, near));
  if (ns !== feelStage && built >= FORM_AT[ns]) { feelStage = ns; cloud.set(FEEL[ns]); }
  if (st.intro >= 0.45) showCaption(ns);

  // weights of each form (for camera pose)
  const w3 = Math.max(0, 1 - Math.abs(s - 3)), w4 = Math.max(0, 1 - Math.abs(s - 4));
  const wSide = Math.min(1, Math.max(0, 1 - Math.abs(s - 2.5) / 2)); // forms 1..4 lean right
  if (w3 > 0.05 && swellTl.state !== 'running') swellTl.play();
  else if (w3 <= 0.05 && swellTl.state === 'running') swellTl.pause();
  f.wave = sea.swell * smooth(0.2, 0.9, w3);

  // --- palette
  const pa = PAL[cloud.from], pb = PAL[cloud.to];
  const pm = st.intro < 1 ? smooth(0, 1, st.intro) : mix;
  for (let i = 0; i < 3; i++) {
    f.a[i] = pa[0][i] + (pb[0][i] - pa[0][i]) * pm;
    f.b[i] = pa[1][i] + (pb[1][i] - pa[1][i]) * pm;
    f.c[i] = pa[2][i] + (pb[2][i] - pa[2][i]) * pm;
  }

  // --- model rotation: gentle sway, lean for wave/galaxy, galaxy spin
  st.sway = Math.sin(f.time * 0.23) * 0.16;
  st.pxs += (st.px - st.pxs) * (1 - Math.exp(-dt * 3));
  st.pys += (st.py - st.pys) * (1 - Math.exp(-dt * 3));
  st.spin += dt * 0.12 * w4;
  if (w4 < 0.6) { const nearest = Math.round(st.spin / (Math.PI * 2)) * Math.PI * 2; st.spin += (nearest - st.spin) * (1 - Math.exp(-dt * 1.5 * (1 - w4))); }
  const yaw = st.sway + st.spin + (st.pxs - 0.5) * 0.35;
  const pitch = 0.06 + 0.5 * w3 + 0.85 * w4 + (st.pys - 0.5) * 0.18;
  const cy = Math.cos(yaw), sy = Math.sin(yaw), cp = Math.cos(pitch), sp = Math.sin(pitch);
  // M = Rx(pitch) · Ry(yaw), column-major mat3
  const m = f.model;
  m[0] = cy; m[1] = sp * sy; m[2] = -cp * sy;
  m[3] = 0; m[4] = cp; m[5] = sp;
  m[6] = sy; m[7] = -sp * cy; m[8] = cp * cy;

  // --- camera: fit ~1150×760 units, slight lens shift to clear the captions
  const aspect = cssW / cssH, tanH = Math.tan(FOV / 2);
  const fitW = 1150 / 2 / (tanH * aspect * 0.94), fitH = 800 / 2 / (tanH * 0.86);
  const wide = aspect > 1.25;
  camDist = Math.max(fitW, fitH) * (wide ? 1 : 1.04) * (1 + 0.24 * w4);
  const oxT = wide ? 0.24 * wSide : 0;
  st.ox += (oxT - st.ox) * (1 - Math.exp(-dt * 2.5));
  eye[0] = 0; eye[1] = 140; eye[2] = camDist;
  lookAt(view, eye[0], eye[1], eye[2], 0, -10, 0, basis);
  perspective(proj, FOV, aspect, 10, 8000);
  mul4(f.viewProj, proj, view);
  const vp = f.viewProj, ox = st.ox;
  vp[0] += ox * vp[3]; vp[4] += ox * vp[7]; vp[8] += ox * vp[11]; vp[12] += ox * vp[15];
  const oy = st.oy = aspect < 0.8 ? 0.16 : 0;
  vp[1] += oy * vp[3]; vp[5] += oy * vp[7]; vp[9] += oy * vp[11]; vp[13] += oy * vp[15];
  f.focal = proj[5] * (gl ? gl.h : cssH) / 2;
  f.floor = -420;

  // --- brush
  if (st.moved && st.intro > 0.5 && !reduced) {
    pointerRay(st.px * 2 - 1, 1 - st.py * 2);
    const t = -(ray[0] * ray[3] + ray[1] * ray[4] + ray[2] * ray[5]);
    const wpp = (2 * Math.max(200, t) * tanH); // world units per viewport height
    const vx = (st.pdx * aspect) * wpp / Math.max(dt, 1 / 120), vy = -st.pdy * wpp / Math.max(dt, 1 / 120);
    // basis: right (0..2), up (3..5)
    worldToModel(sweep, basis[0] * vx + basis[3] * vy, basis[1] * vx + basis[4] * vy, basis[2] * vx + basis[5] * vy);
    const speed = Math.min(4000, Math.hypot(vx, vy));
    cloud.push(ray[0] + ray[3] * t, ray[1] + ray[4] * t, ray[2] + ray[5] * t, 100, 110 + speed * 0.18, sweep[0] * 0.1, sweep[1] * 0.1, sweep[2] * 0.1, ray[3], ray[4], ray[5]);
  }
  st.moved = false; st.pdx = 0; st.pdy = 0;

  // --- draw
  if (!gl || st.noDraw) return;
  if (positions.buffer.byteLength === 0) positions = cloud.positions; // memory grew
  gl.upload(positions);
  f.glow = st.glow * 0.9;
  f.shadow = st.shadow * (1 - 0.8 * Math.max(0, s - 4)); // the last form sits over the install button
  f.cut = st.cut;
  f.size = 2.1 * (1 + 0.2 * w4);
  f.radial = smooth(0.3, 0.9, w4);
  f.alpha = 0.55 * (0.5 + 0.5 * Math.min(1, st.intro * 1.5));
  gl.render(f);
  for (let i = 0; i < capFx.length; i++) {
    const fx = capFx[i];
    if (!fx.live || !fx.hc?.texture) continue;
    capDraw.tex = fx.hc.texture; capDraw.x = fx.x; capDraw.y = fx.y; capDraw.w = fx.w; capDraw.h = fx.h;
    capDraw.cols = fx.cols; capDraw.rows = fx.rows; capDraw.cell = fx.cell; capDraw.p = fx.p;
    capDraw.time = f.time; capDraw.tint = f.b;
    gl.drawCaption(capDraw);
  }

  if (PERF && ++perfTick % 15 === 0) {
    const g = governor;
    perfEl.textContent = `frame p50 ${g.p50.toFixed(1)}  p95 ${g.p95.toFixed(1)}  max ${g.max.toFixed(1)} ms  js ${st.js.toFixed(1)}\nquality ${g.level}${g.locked ? ' (locked)' : ''}  splats ${Math.round(st.cut).toLocaleString()}  dpr ${f.dpr.toFixed(2)}`;
  }
}

// Pre-warm: compile, upload, draw once offscreen-visible-at-0 before revealing.
if (q.has('test')) engine.lagSmoothing(0); // headless checks: let slow frames advance real time
engine.onFrame(frame);
onScroll();
st.s = target.s;
requestAnimationFrame(() => requestAnimationFrame(() => {
  applyLevel();
  animate(canvas, { opacity: [0, 1] }, { duration: 1.2, ease: 'outSine' });
  if (!reduced) animate(st, { intro: 1 }, { duration: 2.6, ease: 'inOutCubic' });
  setTimeout(buildNext, 120);
}));
if (PERF) Object.assign(window, { __st: st, __gov: governor, __cloud: cloud, __capFx: capFx, __engine: engine });
