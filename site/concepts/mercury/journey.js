// Mercury: a scroll journey through a liquid-metal world.
// One render loop (anikit's engine.onFrame). Scroll progress is smoothed by a critically
// damped anikit spring; that one value seeks an anikit timeline which drives every beat
// (morph, staggered burst, rain, mist, install), and a spline camera.
import { engine, ready, spring, world, timeline, stagger, smoothScroll, pointCloud, glow, scene3d, animate } from '../../../packages/anikit/dist/anikit.lean.js';
import { vert, frag, MAX_DROPS } from './shader3d.js';

const $ = (s) => document.querySelector(s);
const reduced = matchMedia('(prefers-reduced-motion: reduce)').matches;
const fine = matchMedia('(hover: hover) and (pointer: fine)').matches;
const small = innerWidth < 700;
const clamp01 = (v) => (v < 0 ? 0 : v > 1 ? 1 : v);
const sstep = (a, b, x) => { const t = clamp01((x - a) / (b - a)); return t * t * (3 - 2 * t); };
const lerp = (a, b, t) => a + (b - a) * t;
const TAU = Math.PI * 2;
let vw = innerWidth, vh = innerHeight;

// ---------------------------------------------------------------------------
// WebGL
// ---------------------------------------------------------------------------
const canvas = $('#metal');
const gl = canvas.getContext('webgl2', { antialias: false, alpha: false, depth: false, stencil: false, powerPreference: 'high-performance' });
const U = {};
let glOk = !!gl, tex = null;
function setupGL() {
  const sh = (type, src) => { const s = gl.createShader(type); gl.shaderSource(s, src); gl.compileShader(s); if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s)); return s; };
  const prog = gl.createProgram();
  gl.attachShader(prog, sh(gl.VERTEX_SHADER, vert));
  gl.attachShader(prog, sh(gl.FRAGMENT_SHADER, frag));
  gl.bindAttribLocation(prog, 0, 'aPos');
  gl.linkProgram(prog);
  if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(prog));
  gl.useProgram(prog);
  gl.bindBuffer(gl.ARRAY_BUFFER, gl.createBuffer());
  gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 3, -1, -1, 3]), gl.STATIC_DRAW);
  gl.enableVertexAttribArray(0);
  gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0);
  for (const n of ['uRes', 'uTime', 'uCam', 'uCamM', 'uFov', 'uD', 'uK', 'uTex', 'uTC', 'uTB', 'uSph', 'uMorph', 'uRip', 'uLight']) U[n] = gl.getUniformLocation(prog, n);
  tex = gl.createTexture();
  gl.bindTexture(gl.TEXTURE_2D, tex);
  for (const [k, v] of [[gl.TEXTURE_MIN_FILTER, gl.LINEAR], [gl.TEXTURE_MAG_FILTER, gl.LINEAR], [gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE], [gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE]]) gl.texParameteri(gl.TEXTURE_2D, k, v);
  gl.texImage2D(gl.TEXTURE_2D, 0, gl.R16F, 1, 1, 0, gl.RED, gl.FLOAT, new Float32Array([1000]));
  gl.uniform1i(U.uTex, 0);
}
if (glOk) { try { setupGL(); } catch (e) { console.warn(e); glOk = false; } }
document.documentElement.classList.add(glOk ? 'gl' : 'no-gl');

// Resolution: full device pixels (up to 2×, 1.5× on phones), calibrated once, then locked.
const qParam = +new URLSearchParams(location.search).get('q') || 0;
const qMax = qParam || Math.min(devicePixelRatio || 1, small ? 1.5 : 2);
const qMin = qParam || qMax * 0.75;
let q = qMax, qLocked = !!qParam, calib = 0, winSum = 0, winN = 0, frames = 0;
function sizeCanvas() {
  const w = Math.max(2, Math.round(vw * q)), h = Math.max(2, Math.round(vh * q));
  if (canvas.width !== w || canvas.height !== h) { canvas.width = w; canvas.height = h; }
}
sizeCanvas();

// ---------------------------------------------------------------------------
// Wordmark distance field (render text, exact two-pass EDT)
// ---------------------------------------------------------------------------
function edt(grid, W, H) {
  const INF = 1e20, n = Math.max(W, H);
  const f = new Float64Array(n), d = new Float64Array(n), v = new Int32Array(n), z = new Float64Array(n + 1);
  const pass = (off, stride, len) => {
    for (let i = 0; i < len; i++) f[i] = grid[off + i * stride];
    let k = 0; v[0] = 0; z[0] = -INF; z[1] = INF;
    for (let i = 1; i < len; i++) {
      let s = ((f[i] + i * i) - (f[v[k]] + v[k] * v[k])) / (2 * i - 2 * v[k]);
      while (s <= z[k]) { k--; s = ((f[i] + i * i) - (f[v[k]] + v[k] * v[k])) / (2 * i - 2 * v[k]); }
      k++; v[k] = i; z[k] = s; z[k + 1] = INF;
    }
    k = 0;
    for (let i = 0; i < len; i++) { while (z[k + 1] < i) k++; d[i] = (i - v[k]) * (i - v[k]) + f[v[k]]; }
    for (let i = 0; i < len; i++) grid[off + i * stride] = d[i];
  };
  for (let x = 0; x < W; x++) pass(x, W, H);
  for (let y = 0; y < H; y++) pass(y * W, 1, W);
}
const text = { ready: false, W: 1, H: 1, pad: 0 };
function buildText() {
  const W = 1024, pad = 48, str = 'anikit';
  const c = document.createElement('canvas');
  const ctx = c.getContext('2d', { willReadFrequently: true });
  const font = (fs) => `900 ${fs}px "Inter Display", Inter, system-ui, sans-serif`;
  ctx.font = font(200);
  ctx.letterSpacing = '-9px';
  let m = ctx.measureText(str);
  const fs = (200 * (W - 2 * pad)) / (m.actualBoundingBoxRight + m.actualBoundingBoxLeft);
  ctx.font = font(fs); ctx.letterSpacing = `${-0.045 * fs}px`;
  m = ctx.measureText(str);
  const H = Math.ceil(m.actualBoundingBoxAscent + m.actualBoundingBoxDescent + 2 * pad);
  c.width = W; c.height = H;
  ctx.font = font(fs); ctx.letterSpacing = `${-0.045 * fs}px`; ctx.fillStyle = '#fff';
  const left = (W - (m.actualBoundingBoxRight + m.actualBoundingBoxLeft)) / 2 + m.actualBoundingBoxLeft;
  ctx.fillText(str, left, pad + m.actualBoundingBoxAscent);
  const px = ctx.getImageData(0, 0, W, H).data;
  const inside = new Uint8Array(W * H);
  for (let i = 0; i < W * H; i++) inside[i] = px[i * 4 + 3] > 127 ? 1 : 0;
  const a = new Float64Array(W * H), b = new Float64Array(W * H);
  for (let i = 0; i < W * H; i++) { a[i] = inside[i] ? 0 : 1e20; b[i] = inside[i] ? 1e20 : 0; }
  edt(a, W, H); edt(b, W, H);
  const sd = new Float32Array(W * H);
  for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
    const i = y * W + x, o = (H - 1 - y) * W + x;
    sd[o] = inside[i] ? -(Math.sqrt(b[i]) - 0.5) : Math.sqrt(a[i]) - 0.5;
  }
  gl.bindTexture(gl.TEXTURE_2D, tex);
  gl.texImage2D(gl.TEXTURE_2D, 0, gl.R16F, W, H, 0, gl.RED, gl.FLOAT, sd);
  Object.assign(text, { ready: true, W, H, pad, gh: H - 2 * pad });
}

// ---------------------------------------------------------------------------
// The journey: one timeline over scroll progress 0..1 (in timeline seconds).
// ---------------------------------------------------------------------------
const N = small ? 28 : 40;
const S = { morph: 0, rain: 0, dive: 0, mist: 0, cond: 0, install: 0 };
const burst = Array.from({ length: N }, () => ({ p: 0 }));
const tl = timeline({ autoplay: false, defaults: { ease: 'inOutCubic' } });
tl.add(S, { morph: [0, 1] }, { duration: 0.13 }, 0.13)
  .add(burst, { p: [0, 1] }, { duration: 0.07, ease: 'outCubic', delay: stagger(0.0022, { from: 'center' }) }, 0.285)
  .add(S, { rain: [0, 1] }, { duration: 0.035, ease: 'inQuad' }, 0.455)
  .add(S, { dive: [0, 1] }, { duration: 0.06 }, 0.62)
  .add(S, { mist: [0, 1] }, { duration: 0.025, ease: 'linear' }, 0.655)
  .add(S, { cond: [0, 2] }, { duration: 0.12, ease: 'inOutSine' }, 0.69)
  .add(S, { mist: [1, 0] }, { duration: 0.03, ease: 'linear' }, 0.835)
  .add(S, { install: [0, 1] }, { duration: 0.09 }, 0.86);
tl.add(S, { install: 1 }, { duration: 0.001 }, 0.999);

// Camera spline: [progress, position, target].
const KEYS = [
  [0.0, [0, 0.42, 5.2], [0, 1.15, 0]],
  [0.1, [0.6, 0.5, 4.8], [0, 1.15, 0]],
  [0.2, [4.2, 1.5, 2.2], [0, 1.1, 0]],
  [0.29, [4.2, 2.6, -2.6], [0, 1.1, 0]],
  [0.36, [2.1, 1.35, -0.6], [-1, 1.2, 0.4]],
  [0.43, [-1.6, 1.5, 2.4], [0, 1.0, 0]],
  [0.5, [0.0, 1.5, 6.4], [0, 1.1, 0]],
  [0.6, [0.4, 1.0, 5.2], [0, 0.8, 0]],
  [0.66, [0.1, 0.25, 1.4], [0, 0, -0.2]],
  [0.7, [0, 0.12, 0.6], [0, 0, -0.4]],
  [0.84, [0, 2.0, 3.0], [0, 0, 0]],
  [0.92, [0, 5.6, 3.2], [0, 0, 0.2]],
  [1.0, [0, 6.0, 3.0], [0, 0, 0.25]],
];
const cr = (p0, p1, p2, p3, u) => { const u2 = u * u, u3 = u2 * u; return 0.5 * ((2 * p1) + (-p0 + p2) * u + (2 * p0 - 5 * p1 + 4 * p2 - p3) * u2 + (-p0 + 3 * p1 - 3 * p2 + p3) * u3); };
function camAt(s, out, which) {
  let k = 0;
  while (k < KEYS.length - 2 && s > KEYS[k + 1][0]) k++;
  const a = KEYS[Math.max(0, k - 1)], b = KEYS[k], c = KEYS[k + 1], d = KEYS[Math.min(KEYS.length - 1, k + 2)];
  const u = clamp01((s - b[0]) / (c[0] - b[0]));
  for (let j = 0; j < 3; j++) out[j] = cr(a[which][j], b[which][j], c[which][j], d[which][j], u);
}

// ---------------------------------------------------------------------------
// Drops
// ---------------------------------------------------------------------------
let seed = 9;
const rnd = () => ((seed = (seed * 1664525 + 1013904223) >>> 0) / 4294967296);
const drops = Array.from({ length: N }, (_, i) => ({ r: 0.07 + rnd() * 0.08, ph: rnd() * TAU, x: 0, y: 1.1, z: 0, rad: 0 }));
const D = new Float32Array(MAX_DROPS * 4);
const SPH = { x: 0, y: 1.15, z: 0, r0: 0.78 };

// Physics: a vertical slab (the z = 0 plane), 100 px per world unit, floor at y = 0.
const PX = 100, X0 = 5, Y0 = 6;
let phys = null, physOn = false, grabbed = -1;
const toPx = (x, y) => [(x + X0) * PX, (Y0 - y) * PX];
function makeWorld() {
  phys = world({ bounds: [0, -3000, 2 * X0 * PX, Y0 * PX], gravity: [0, 1400], substeps: 4, iterations: 3, restitution: 0.42, friction: 0.04, damping: 0.004, autoStep: true });
  for (const d of drops) phys.add(X0 * PX, -500, d.r * PX, d.r * d.r * 100);
  phys.pause();
}
function physicsToggle(on) {
  if (on === physOn || !phys) return;
  physOn = on;
  if (on) {
    drops.forEach((d, i) => { const [x, y] = toPx(d.x, d.y); phys.setPosition(i, x, y); phys.setVelocity(i, -Math.sin(d.ph) * 120, -200 * rnd()); });
    phys.play();
  } else phys.pause();
}

// Pointer: picking bodies in the physics slab, ripples on the wordmark, light parallax.
const ptr = { x: 0, y: 0, at: -1e9 };
const tilt = { x: 0, y: 0 };
const rip = { s: 0 };
const cam = { s: 0 };
const camPos = [0, 0, 0], camTgt = [0, 0, 0];
const M = new Float32Array(9);
const FOV = Math.tan((small ? 50 : 38) * Math.PI / 360);
function rayToPlaneZ(cx, cy, zPlane, out) {
  // Ray through a screen point, intersected with the plane z = zPlane.
  const sx = ((cx * 2 - vw) / vh) * FOV, sy = (-(cy * 2 - vh) / vh) * FOV;
  const dx = M[0] * sx + M[3] * sy + M[6], dy = M[1] * sx + M[4] * sy + M[7], dz = M[2] * sx + M[5] * sy + M[8];
  const t = (zPlane - camPos[2]) / dz;
  out[0] = camPos[0] + dx * t; out[1] = camPos[1] + dy * t;
  return t > 0;
}
const hitP = [0, 0];
let lastGrab = [0, 0, 0], grabV = [0, 0];
addEventListener('pointermove', (e) => {
  ptr.x = e.clientX; ptr.y = e.clientY; ptr.at = performance.now();
  if (!reduced) {
    spring(tilt, { x: (e.clientX / vw) * 2 - 1, y: (e.clientY / vh) * 2 - 1 }, { stiffness: 40, damping: 12.6 });
    spring(rip, { s: 1 }, { stiffness: 120, damping: 22 });
  }
  if (grabbed >= 0 && rayToPlaneZ(e.clientX, e.clientY, 0, hitP)) {
    const [x, y] = toPx(hitP[0], Math.max(0.1, hitP[1]));
    const dt = Math.max(1, e.timeStamp - lastGrab[2]) / 1000;
    grabV = [(x - lastGrab[0]) / dt, (y - lastGrab[1]) / dt];
    lastGrab = [x, y, e.timeStamp];
    phys.setPosition(grabbed, x, y);
  }
}, { passive: true });
canvas.addEventListener('pointerdown', (e) => {
  if (!physOn) return;
  if (!rayToPlaneZ(e.clientX, e.clientY, 0, hitP)) return;
  const [x, y] = toPx(hitP[0], hitP[1]);
  let best = -1, bd = 1e9;
  drops.forEach((d, i) => { const dd = Math.hypot(phys.x(i) - x, phys.y(i) - y) - d.r * PX; if (dd < bd) { bd = dd; best = i; } });
  if (bd < 40) { grabbed = best; lastGrab = [x, y, e.timeStamp]; grabV = [0, 0]; canvas.setPointerCapture(e.pointerId); e.preventDefault(); }
});
addEventListener('pointerup', () => { if (grabbed >= 0) { phys.setVelocity(grabbed, grabV[0], grabV[1]); grabbed = -1; } });

// ---------------------------------------------------------------------------
// Particles: the mist inside the mercury (overlay canvas)
// ---------------------------------------------------------------------------
const NP = reduced ? 30_000 : small ? 40_000 : 100_000;
let cloud = null, g = null, cloudScene = null;
const mistCanvas = $('#mist');
function idleJobs() {
  const jobs = [];
  jobs.push(() => { cloud = pointCloud(NP, { autoStep: false, stiffness: 26, damping: 6.5, spread: 0.8, noise: 1.4, swirl: 90, frequency: 0.01 }); return true; });
  const gens = [
    (i, a, o) => { const u = rnd() * 2 - 1, th = rnd() * TAU, r = 150 * Math.cbrt(rnd()), s = Math.sqrt(1 - u * u); a[o] = Math.cos(th) * s * r; a[o + 1] = u * r; a[o + 2] = Math.sin(th) * s * r; },
    (i, a, o) => { const th = rnd() * TAU, rr = 120 + 380 * Math.pow(rnd(), 0.7), arm = Math.floor(rnd() * 3) * TAU / 3; const ang = th * 0.15 + arm + rr * 0.008; a[o] = Math.cos(ang) * rr; a[o + 1] = (rnd() - 0.5) * 60 * (1 - rr / 600); a[o + 2] = Math.sin(ang) * rr; },
  ];
  gens.forEach((fn, k) => {
    let buf = null, i = 0;
    jobs.push((dl) => {
      buf ??= new Float32Array(NP * 3);
      const end = performance.now() + Math.min(6, Math.max(2, dl.timeRemaining() - 1));
      while (i < NP && performance.now() < end) { const stop = Math.min(NP, i + 2000); for (; i < stop; i++) fn(i, buf, i * 3); }
      if (i < NP) return false;
      cloud.shape(k, buf); if (k === 0) cloud.snap(0);
      return true;
    });
  });
  jobs.push(() => {
    g = glow(mistCanvas, { resolution: small ? 0.7 : 0.85, background: '#020203', trail: 0.3, budget: 5 });
    g.colors(NP, (j) => { const k = rnd(); const s = k < 0.025 ? 2.4 : 0.55 + k * 0.5; return (j % 11 === 0) ? [s, 0.8 * s, 0.6 * s] : [0.8 * s, 0.85 * s, 0.95 * s]; });
    cloudScene = scene3d({ fov: 45, camera: { z: 900 }, near: 10, far: 6000 });
    fitMist();
    return true;
  });
  // Short slices on a timer: progress is steady even when the page never goes idle.
  const idle = (f) => setTimeout(() => f({ timeRemaining: () => 6 }), 16);
  const run = (dl) => { if (!jobs.length) return; if (jobs[0](dl)) jobs.shift(); idle(run, { timeout: 300 }); };
  idle(run, { timeout: 300 });
}
function fitMist() { if (!g) return; const w = Math.round(vw), h = Math.round(vh); g.resize(w, h); cloudScene.resize(w, h, 1); }

// ---------------------------------------------------------------------------
// UI: labels and panels fade with the smoothed progress (opacity + a small lift).
// ---------------------------------------------------------------------------
const beats = [...document.querySelectorAll('[data-from]')].map((el) => ({ el, a: +el.dataset.from, b: +el.dataset.to, last: -1 }));
function ui(s) {
  for (const k of beats) {
    const o = Math.min(sstep(k.a - 0.025, k.a + 0.01, s), 1 - sstep(k.b - 0.01, k.b + 0.025, s));
    const r = Math.round(o * 200) / 200;
    if (r === k.last) continue;
    k.last = r;
    k.el.style.opacity = r;
    k.el.style.transform = `translate3d(0, ${((1 - r) * 14).toFixed(1)}px, 0)`;
    k.el.style.visibility = r === 0 ? 'hidden' : 'visible';
  }
}

// ---------------------------------------------------------------------------
// Frame
// ---------------------------------------------------------------------------
let tSec = 0, lastNow = performance.now(), frameAvg = 16.7, mistVis = -1;
const stats = { q: 0, w: 0, h: 0, ms: 0, s: 0 };
window.__mercury = stats;
const fwd = [0, 0, 0], right = [0, 0, 0], up = [0, 0, 0];
function basis() {
  for (let j = 0; j < 3; j++) fwd[j] = camTgt[j] - camPos[j];
  let l = Math.hypot(fwd[0], fwd[1], fwd[2]); for (let j = 0; j < 3; j++) fwd[j] /= l;
  // right = fwd × worldUp, up = right × fwd (with a fallback when looking straight down)
  right[0] = -fwd[2]; right[1] = 0; right[2] = fwd[0];
  l = Math.hypot(right[0], right[2]); if (l < 1e-4) { right[0] = 1; right[2] = 0; l = 1; }
  right[0] /= l; right[2] /= l;
  up[0] = right[1] * fwd[2] - right[2] * fwd[1]; up[1] = right[2] * fwd[0] - right[0] * fwd[2]; up[2] = right[0] * fwd[1] - right[1] * fwd[0];
  M[0] = right[0]; M[1] = right[1]; M[2] = right[2];
  M[3] = up[0]; M[4] = up[1]; M[5] = up[2];
  M[6] = fwd[0]; M[7] = fwd[1]; M[8] = fwd[2];
}

function progress() { const max = document.documentElement.scrollHeight - vh; return max > 0 ? clamp01(scrollY / max) : 0; }

function frame(dtMs) {
  const dt = Math.min(dtMs, 50) / 1000;
  if (!reduced) tSec += dt;
  const t = tSec;
  // Critically damped spring towards the scroll position.
  const target = progress();
  if (reduced) cam.s = target; else spring(cam, { s: target }, { stiffness: 55, damping: 14.8 });
  const s = cam.s;
  tl.seek(s);

  camAt(s, camPos, 1); camAt(s, camTgt, 2);
  // Pointer parallax and a slow breath, so the camera is never quite still.
  camPos[0] += tilt.x * 0.22 + Math.sin(t * 0.21) * 0.05;
  camPos[1] += -tilt.y * 0.12 + Math.sin(t * 0.17) * 0.03;
  basis();

  // Drops.
  const morph = S.morph;
  let avg = 0;
  for (let i = 0; i < N; i++) avg += burst[i].p;
  avg /= N;
  const physW = S.rain, inst = S.install;
  physicsToggle(physW > 0.02 && s < 0.66);
  for (let i = 0; i < N; i++) {
    const d = drops[i], p = burst[i].p;
    // Before the burst: a few satellites orbit the wordmark; the rest wait inside the sphere.
    const sat = i < 7 ? 1 - morph : 0;
    const oa = d.ph + t * (0.25 + i * 0.03);
    let x = Math.cos(oa) * (2.0 + i * 0.07), y = 1.15 + Math.sin(oa * 1.3) * 0.55, z = Math.sin(oa) * 0.9;
    x = lerp(SPH.x, x, sat); y = lerp(SPH.y, y, sat); z = lerp(SPH.z, z, sat);
    let r = d.r * (sat * 0.55);
    // Burst: out of the sphere, into a turning ring.
    const a = (i / N) * TAU + t * 0.22;
    const R = 2.0 + 0.18 * Math.sin(a * 3 + t * 0.7);
    const hx = Math.cos(a) * R, hy = 1.25 + 0.4 * Math.sin(a * 2 + t * 0.5), hz = Math.sin(a) * R;
    x = lerp(x, hx, p); y = lerp(y, hy, p); z = lerp(z, hz, p);
    r = lerp(r, d.r, sstep(0, 0.4, p));
    // Rain: the physics world takes over in the z = 0 slab.
    if (physW > 0 && phys) {
      const wx = phys.x(i) / PX - X0, wy = Y0 - phys.y(i) / PX;
      x = lerp(x, wx, physW); y = lerp(y, wy, physW); z = lerp(z, 0, physW);
    }
    // Install: the metal settles into a ring on the floor around the command.
    if (inst > 0) {
      const u = (i / N) * TAU + t * 0.05;
      const ix = Math.cos(u) * 1.9 * (vw / vh > 1 ? 1.25 : 0.75), iz = Math.sin(u) * 0.62 + 0.08;
      const e = sstep(0, 1, inst);
      x = lerp(x, ix, e); y = lerp(y, d.r * 0.55, e); z = lerp(z, iz, e);
      r = lerp(r, d.r * 0.62, e);
    }
    // Hidden while we're inside the mercury.
    r *= 1 - sstep(0.25, 1, S.mist) * (1 - inst);
    d.x = x; d.y = y; d.z = z;
    d.rad += (r - d.rad) * Math.min(1, dt * 12 + (reduced ? 1 : 0));
    D[i * 4] = x; D[i * 4 + 1] = y; D[i * 4 + 2] = z; D[i * 4 + 3] = d.rad;
  }
  for (let i = N; i < MAX_DROPS; i++) D[i * 4 + 3] = 0;

  // Mist overlay.
  const mv = S.mist;
  if (Math.abs(mv - mistVis) > 0.004) { mistVis = mv; mistCanvas.style.opacity = mv.toFixed(3); mistCanvas.style.visibility = mv > 0.001 ? 'visible' : 'hidden'; }
  if (mv > 0.001 && cloud && g) {
    const c = S.cond;
    // Atomise into a swirling disc, then condense back into a drop.
    if (c < 1) { cloud.from = 0; cloud.to = 1; cloud.mix = c; } else { cloud.from = 1; cloud.to = 0; cloud.mix = c - 1; }
    cloud.step(dt);
    const ang = t * 0.15 + s * 6;
    cloudScene.camera.x = Math.sin(ang) * 900 * (1 - sstep(1.3, 2, c)); cloudScene.camera.z = Math.cos(ang) * 900 * (1 - sstep(1.3, 2, c)) + 900 * sstep(1.3, 2, c); cloudScene.camera.y = 160 * (1 - sstep(1.3, 2, c));
    g.render(cloudScene, cloud, { size: 1.1, intensity: small ? 0.5 : 0.32, fog: [500, 2400] });
  }

  ui(s);
  if (glOk && !document.hidden) render(t, morph, avg);
  stats.s = s;
}

function render(t, morph, avg) {
  const now = performance.now(), ft = now - lastNow; lastNow = now;
  if (ft < 1000) frameAvg = frameAvg * 0.94 + ft * 0.06;
  if (!qLocked && ++frames > 20 && ft < 1000) {
    winSum += ft; winN++;
    if (winN >= 30) {
      const a = winSum / winN; winSum = 0; winN = 0; calib++;
      if (a > 20 && q > qMin) { q = Math.max(qMin, q * Math.max(0.75, Math.sqrt(16 / a))); sizeCanvas(); } else qLocked = true;
      if (calib >= 2) qLocked = true;
    }
  }
  gl.viewport(0, 0, canvas.width, canvas.height);
  gl.uniform2f(U.uRes, canvas.width, canvas.height);
  gl.uniform1f(U.uTime, t);
  gl.uniform3f(U.uCam, camPos[0], camPos[1], camPos[2]);
  gl.uniformMatrix3fv(U.uCamM, false, M);
  gl.uniform1f(U.uFov, FOV);
  gl.uniform4fv(U.uD, D);
  gl.uniform1f(U.uK, lerp(0.1, 0.075, S.rain));
  // Wordmark: hovers and breathes; its width fits the screen.
  const gw = Math.min(3.4, 3.4 * (vw / vh) / 1.5);
  const scale = text.ready ? gw / (text.W - 2 * text.pad) : 0;
  const breathe = 1 + Math.sin(t * 0.8) * 0.012;
  gl.uniform4f(U.uTC, 0, 1.15 + Math.sin(t * 0.6) * 0.04, 0, scale * breathe);
  gl.uniform4f(U.uTB, (text.W * scale) / 2 * breathe, (text.H * scale) / 2 * breathe, text.ready ? text.gh * scale * 0.1 : 0, text.ready ? 1 : 0);
  const sr = SPH.r0 * Math.cbrt(Math.max(0, 1 - avg * 0.97)) * sstep(0, 0.35, morph) * (1 - sstep(0.85, 1, avg));
  gl.uniform4f(U.uSph, SPH.x, SPH.y, SPH.z, sr);
  gl.uniform1f(U.uMorph, morph);
  // Ripple under the pointer, on the wordmark's plane.
  const recent = performance.now() - ptr.at < 1500;
  if (!recent && rip.s > 0 && !reduced) spring(rip, { s: 0 }, { stiffness: 30, damping: 11 });
  if (rayToPlaneZ(ptr.x, ptr.y, 0, hitP)) gl.uniform4f(U.uRip, hitP[0], hitP[1], 0, rip.s * (1 - morph));
  else gl.uniform4f(U.uRip, 0, 0, 0, 0);
  gl.uniform2f(U.uLight, tilt.x, tilt.y);
  gl.drawArrays(gl.TRIANGLES, 0, 3);
  stats.q = q; stats.w = canvas.width; stats.h = canvas.height; stats.ms = frameAvg;
}

// ---------------------------------------------------------------------------
// Start: a frame at once, then the core, the type and the loop.
// ---------------------------------------------------------------------------
camAt(0, camPos, 1); camAt(0, camTgt, 2); basis();
if (glOk) render(0, 0, 0);
if (reduced) engine.reducedMotion = 'always';
await ready;
await Promise.race([document.fonts.load('900 100px "Inter Display"'), new Promise((r) => setTimeout(r, 900))]);
if (glOk) buildText();
makeWorld();
if (fine && !reduced) smoothScroll({ smooth: 0.5 });
engine.onFrame((_t, dt) => frame(dt));
idleJobs();
let lastW = vw, lastH = vh;
addEventListener('resize', () => {
  if (innerWidth === lastW && Math.abs(innerHeight - lastH) < lastH * 0.2) return;
  lastW = vw = innerWidth; lastH = vh = innerHeight;
  sizeCanvas(); fitMist();
});

// Words of the hero line arrive once; the copy button.
const words = $('.lede');
if (words) animate(words, { opacity: [0, 1], y: [10, 0] }, { duration: 0.9, ease: 'outCubic', delay: 0.3 });
const copy = $('#copy');
copy?.addEventListener('click', async () => {
  try { await navigator.clipboard.writeText('npm install anikit'); } catch { /* blocked */ }
  copy.textContent = 'Copied';
  setTimeout(() => (copy.textContent = 'Copy'), 1600);
});
// Benchmarks count up when their panel first shows.
let counted = false;
const counts = [...document.querySelectorAll('.n-count')];
engine.onFrame(() => {
  if (counted || cam.s < 0.76) return;
  counted = true;
  counts.forEach((c, i) => { const d = +c.dataset.dec; animate(c, { textContent: [0, +c.dataset.to] }, { duration: 1.6, ease: 'outExpo', delay: i * 0.08, format: { minimumFractionDigits: d, maximumFractionDigits: d }, locale: 'en-US' }); });
  animate('.n-row u', { scaleX: [0, 1] }, { duration: 1.4, ease: 'spring(bounce: 0.15, duration: 1.2)', delay: stagger(0.12, { start: 0.2 }) });
});
