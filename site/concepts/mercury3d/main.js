// A scroll journey through a liquid-metal world.
// Anikit drives everything that moves: a scroll-scrubbed timeline() with staggered easing,
// a critically damped spring() on the scroll position, a flightPath() for the camera,
// a physics world() for the droplets on the floor and a pointCloud() for the mist.
// WebGL only draws what those values say.
import { engine, ready, spring, timeline, stagger, world, pointCloud, flightPath, animate } from '../../../packages/anikit/dist/anikit.lean.js';
import { ND, quadVert, sceneFrag, pointsVert, pointsFrag, brightFrag, blurFrag, compositeFrag } from './shader.js';

const $ = (s) => document.querySelector(s);
const $$ = (s) => Array.from(document.querySelectorAll(s));
const reduced = matchMedia('(prefers-reduced-motion: reduce)').matches;
const fine = matchMedia('(hover: hover) and (pointer: fine)').matches;
const small = Math.min(innerWidth, innerHeight) < 600;
const clamp01 = (v) => (v < 0 ? 0 : v > 1 ? 1 : v);
const sstep = (a, b, x) => { const t = clamp01((x - a) / (b - a)); return t * t * (3 - 2 * t); };
const lerp = (a, b, t) => a + (b - a) * t;
const root = document.documentElement;
const params = new URLSearchParams(location.search);

// ---------------------------------------------------------------------------
// WebGL setup. Resolution is chosen once and only changes on a real width change.
// ---------------------------------------------------------------------------
const canvas = $('#gl');
const gl = canvas.getContext('webgl2', { antialias: false, alpha: false, depth: false, stencil: false, premultipliedAlpha: false, powerPreference: 'high-performance' });
if (!gl) { root.classList.add('no-gl'); throw new Error('WebGL2 unavailable'); }
const hdr = !!gl.getExtension('EXT_color_buffer_float');
gl.getExtension('OES_texture_float_linear');

function program(vs, fs, names) {
  const sh = (type, src) => { const s = gl.createShader(type); gl.shaderSource(s, src); gl.compileShader(s); if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s)); return s; };
  const p = gl.createProgram();
  gl.attachShader(p, sh(gl.VERTEX_SHADER, vs));
  gl.attachShader(p, sh(gl.FRAGMENT_SHADER, fs));
  gl.bindAttribLocation(p, 0, names.includes('aPos') ? 'aPos' : 'aPos');
  gl.linkProgram(p);
  if (!gl.getProgramParameter(p, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(p));
  const u = { p };
  for (const n of names) u[n] = gl.getUniformLocation(p, n);
  return u;
}
const COMMON = ['uRes', 'uTime', 'uCam', 'uEye', 'uFov'];
const S = program(quadVert, sceneFrag, [...COMMON, 'uD', 'uSq', 'uN', 'uTex', 'uTR', 'uTA', 'uSph', 'uPool', 'uRip', 'uLight', 'uK', 'uWipe', 'uEyeY', 'uCloudC', 'uOut']);
const PTS = program(pointsVert, pointsFrag, [...COMMON, 'uOff', 'uScale', 'uSpin', 'uSize', 'uWipe', 'uGain']);
const BR = program(quadVert, brightFrag, ['uSrc', 'uTexel']);
const BL = program(quadVert, blurFrag, ['uSrc', 'uTexel', 'uDir']);
const CO = program(quadVert, compositeFrag, ['uSrc', 'uBloom', 'uRes', 'uFade', 'uScaleIn']);

const quadVao = gl.createVertexArray();
gl.bindVertexArray(quadVao);
gl.bindBuffer(gl.ARRAY_BUFFER, gl.createBuffer());
gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 3, -1, -1, 3]), gl.STATIC_DRAW);
gl.enableVertexAttribArray(0);
gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0);

function makeTarget(w, h, float) {
  const tex = gl.createTexture();
  gl.bindTexture(gl.TEXTURE_2D, tex);
  if (float) gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA16F, w, h, 0, gl.RGBA, gl.HALF_FLOAT, null);
  else gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA8, w, h, 0, gl.RGBA, gl.UNSIGNED_BYTE, null);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
  const fb = gl.createFramebuffer();
  gl.bindFramebuffer(gl.FRAMEBUFFER, fb);
  gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, tex, 0);
  return { tex, fb, w, h };
}
const targets = { scene: null, a: null, b: null };
let W = 0, H = 0, lastCssW = 0;
function chooseResolution() {
  const cssW = innerWidth, cssH = innerHeight;
  lastCssW = cssW;
  // Up to 2 device pixels per CSS pixel on desktop, within a fixed pixel budget.
  const dpr = Math.min(devicePixelRatio || 1, small ? 1.5 : 2);
  const budget = +(params.get('px') || (small ? 1.1e6 : 2.6e6));
  const k = Math.min(dpr, Math.sqrt(budget / (cssW * cssH)));
  W = Math.max(2, Math.round(cssW * k)); H = Math.max(2, Math.round(cssH * k));
  canvas.width = W; canvas.height = H;
  for (const key of Object.keys(targets)) if (targets[key]) { gl.deleteTexture(targets[key].tex); gl.deleteFramebuffer(targets[key].fb); }
  targets.scene = makeTarget(W, H, hdr);
  const bw = Math.max(2, W >> 2), bh = Math.max(2, H >> 2);
  targets.a = makeTarget(bw, bh, hdr);
  targets.b = makeTarget(bw, bh, hdr);
}
chooseResolution();

// Wordmark distance-field texture (filled in once the font has loaded).
const textTex = gl.createTexture();
gl.bindTexture(gl.TEXTURE_2D, textTex);
gl.texImage2D(gl.TEXTURE_2D, 0, gl.R16F, 1, 1, 0, gl.RED, gl.FLOAT, new Float32Array([10]));
for (const [k, v] of [[gl.TEXTURE_MIN_FILTER, gl.LINEAR], [gl.TEXTURE_MAG_FILTER, gl.LINEAR], [gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE], [gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE]]) gl.texParameteri(gl.TEXTURE_2D, k, v);

// ---------------------------------------------------------------------------
// The wordmark: drawn to a canvas, turned into an exact signed distance field in a worker.
// ---------------------------------------------------------------------------
const TEXT_W = 3.4, TEXT_Y = 1.08;
const text = { ready: false, hw: TEXT_W / 2, hh: 0.5 };
const edtWorker = `
function edt(g, W, H) {
  const INF = 1e20, n = Math.max(W, H), f = new Float64Array(n), d = new Float64Array(n), v = new Int32Array(n), z = new Float64Array(n + 1);
  const pass = (off, stride, len) => {
    for (let q = 0; q < len; q++) f[q] = g[off + q * stride];
    let k = 0; v[0] = 0; z[0] = -INF; z[1] = INF;
    for (let q = 1; q < len; q++) {
      let s = ((f[q] + q * q) - (f[v[k]] + v[k] * v[k])) / (2 * q - 2 * v[k]);
      while (s <= z[k]) { k--; s = ((f[q] + q * q) - (f[v[k]] + v[k] * v[k])) / (2 * q - 2 * v[k]); }
      k++; v[k] = q; z[k] = s; z[k + 1] = INF;
    }
    k = 0;
    for (let q = 0; q < len; q++) { while (z[k + 1] < q) k++; d[q] = (q - v[k]) * (q - v[k]) + f[v[k]]; }
    for (let q = 0; q < len; q++) g[off + q * stride] = d[q];
  };
  for (let x = 0; x < W; x++) pass(x, W, H);
  for (let y = 0; y < H; y++) pass(y * W, 1, W);
}
onmessage = (e) => {
  const { alpha, W, H, scale } = e.data;
  const a = new Float64Array(W * H), b = new Float64Array(W * H);
  for (let i = 0; i < W * H; i++) { const inside = alpha[i] > 127; a[i] = inside ? 0 : 1e20; b[i] = inside ? 1e20 : 0; }
  edt(a, W, H); edt(b, W, H);
  const sd = new Float32Array(W * H);
  for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
    const i = y * W + x, o = (H - 1 - y) * W + x;
    sd[o] = (alpha[i] > 127 ? -(Math.sqrt(b[i]) - 0.5) : Math.sqrt(a[i]) - 0.5) * scale;
  }
  postMessage(sd, [sd.buffer]);
};`;
function buildText() {
  return new Promise((resolve) => {
    const Wt = 1280, pad = 64;
    const c = document.createElement('canvas');
    const ctx = c.getContext('2d', { willReadFrequently: true });
    const font = (px) => `900 ${px}px "Inter Display", Inter, system-ui, sans-serif`;
    ctx.font = font(200);
    ctx.letterSpacing = '-8px';
    let m = ctx.measureText('anikit');
    const fs = (200 * (Wt - 2 * pad)) / (m.actualBoundingBoxRight + m.actualBoundingBoxLeft);
    ctx.font = font(fs);
    ctx.letterSpacing = `${-0.04 * fs}px`;
    m = ctx.measureText('anikit');
    const gh = m.actualBoundingBoxAscent + m.actualBoundingBoxDescent;
    const Ht = Math.ceil(gh + 2 * pad);
    c.width = Wt; c.height = Ht;
    ctx.font = font(fs);
    ctx.letterSpacing = `${-0.04 * fs}px`;
    ctx.fillStyle = '#fff';
    m = ctx.measureText('anikit');
    const left = (Wt - (m.actualBoundingBoxRight + m.actualBoundingBoxLeft)) / 2 + m.actualBoundingBoxLeft;
    ctx.fillText('anikit', left, pad + m.actualBoundingBoxAscent);
    const px = ctx.getImageData(0, 0, Wt, Ht).data;
    const alpha = new Uint8Array(Wt * Ht);
    for (let i = 0; i < alpha.length; i++) alpha[i] = px[i * 4 + 3];
    const scale = TEXT_W / (Wt - 2 * pad);
    const wk = new Worker(URL.createObjectURL(new Blob([edtWorker], { type: 'text/javascript' })));
    wk.onmessage = (e) => {
      gl.bindTexture(gl.TEXTURE_2D, textTex);
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.R16F, Wt, Ht, 0, gl.RED, gl.FLOAT, e.data);
      text.hw = (Wt * scale) / 2; text.hh = (Ht * scale) / 2;
      text.ready = true;
      wk.terminate();
      resolve();
    };
    wk.postMessage({ alpha, W: Wt, H: Ht, scale }, [alpha.buffer]);
  });
}

// ---------------------------------------------------------------------------
// The camera: two flight paths (eye and aim), timed by scroll progress 0–100.
// ---------------------------------------------------------------------------
const deg = Math.PI / 180;
const arc = (a, r, y) => [Math.sin(a * deg) * r, y, Math.cos(a * deg) * r];
const HC = [0, 1.05, 0];                      // helix centre
const A = [-Math.sin(120 * deg), 0, -Math.cos(120 * deg)]; // helix axis, along the camera's line after the arc
const along = (k, y = 0, side = 0) => [HC[0] + A[0] * k + A[2] * side, HC[1] + y, HC[2] + A[2] * k - A[0] * side];
const KEYS = [
  // P, eye, aim
  [0, [0, 0.36, 5.4], [0, 0.98, 0]],
  [7, [0.25, 0.42, 5.2], [0, 1.0, 0]],
  [15, arc(55, 4.7, 0.75), [0, 1.0, 0]],
  [23, arc(120, 4.3, 1.1), [0, 1.0, 0]],
  [30, along(-2.6, 0.05), along(1.5)],
  [37, along(1.4, 0.0, 0.1), along(4.5, -0.2)],
  [45, along(3.6, 2.4, 0.8), [0, 0.05, 0]],
  [54, [-1.4, 3.6, 3.2], [0, 0.0, 0]],
  [61, [-0.25, 4.1, 0.9], [0, 0, 0]],
  [66, [0, 0.9, 0.32], [0, -3, 0]],
  [70, [0, -1.1, 0.9], [0, -3.4, -0.5]],
  [76, [0.2, -2.3, 4.2], [0, -3.1, 0]],
  [84, [0.55, -2.75, 4.6], [0.1, -3.05, 0]],
  [88, [0.2, -1.6, 2.6], [0, 0.4, -0.6]],
  [91.5, [0, 0.6, 1.6], [0, 2.6, -2.5]],
  [100, [0, 7.2, 4.6], [0, 0.55, 0]],
];
function pathTable(index) {
  const pts = KEYS.map((k) => k[index]);
  const fp = flightPath(pts, { lookAhead: 1, bank: 0 });
  // Arc length of each key: the nearest sample on the path.
  const S = 4000, cum = [];
  let best = 0;
  for (let k = 0; k < pts.length; k++) {
    let bd = Infinity, bs = best;
    for (let i = 0; i <= S; i++) {
      const s = (fp.length * i) / S;
      if (s < best - 1e-6) continue;
      const p = fp.point(s), d = (p[0] - pts[k][0]) ** 2 + (p[1] - pts[k][1]) ** 2 + (p[2] - pts[k][2]) ** 2;
      if (d < bd) { bd = d; bs = s; }
      if (d > bd + 0.5 && bd < 0.01) break;
    }
    best = bs; cum.push(bs);
  }
  const timing = fp.timing(KEYS.map((k, i) => [k[0], cum[i] / fp.length]));
  // Dense table: no allocation while scrolling.
  const N = 2001, tab = new Float32Array(N * 3);
  for (let i = 0; i < N; i++) { const p = fp.point(timing((100 * i) / (N - 1))); tab.set(p, i * 3); }
  return tab;
}
const eyeTab = pathTable(1), aimTab = pathTable(2);
function sampleTab(tab, P, out) {
  const f = clamp01(P / 100) * 2000, i = Math.min(1999, f | 0), k = f - i;
  for (let c = 0; c < 3; c++) out[c] = tab[i * 3 + c] + (tab[i * 3 + 3 + c] - tab[i * 3 + c]) * k;
}

// ---------------------------------------------------------------------------
// The droplets and the scroll timeline.
// ---------------------------------------------------------------------------
const N = small ? 32 : ND;
let seed = 11;
const rnd = () => ((seed = (seed * 1664525 + 1013904223) >>> 0) / 4294967296);
const drops = Array.from({ length: N }, (_, i) => ({ h: 0, f: 0, g: 0, r: 0.075 + rnd() * 0.05, ox: rnd() * 2 - 1, oy: rnd() * 2 - 1, oz: rnd() * 2 - 1, i }));
const st = { m: 0, merge: 0, mix: 0, settle: 0, portal: 0, pad: 0 };
const tl = timeline({ autoplay: false })
  .add(st, { m: [0, 1] }, { duration: 15, ease: 'inOutSine' }, 7)
  .add(drops, { h: [0, 1] }, { duration: 6, ease: 'outBack', delay: stagger(0.13) }, 23)
  .add(drops, { f: [0, 1] }, { duration: 5, ease: 'outBounce', delay: stagger(0.09, { from: 'center' }) }, 40)
  .add(drops, { g: [0, 1] }, { duration: 2.2, ease: 'inOutQuad', delay: stagger(0.09, { from: 'center' }) }, 40)
  .add(st, { merge: [0, 1] }, { duration: 6, ease: 'inOutCubic' }, 56)
  .add(st, { portal: [0, 1] }, { duration: 4, ease: 'inQuad' }, 62)
  .add(st, { portal: [1, 0] }, { duration: 3, ease: 'outQuad' }, 66.5)
  .add(st, { mix: [0, 1] }, { duration: 8, ease: 'inOutCubic' }, 75)
  .add(st, { settle: [0, 1] }, { duration: 9, ease: 'inOutSine' }, 90)
  .add(st, { pad: [0, 1] }, { duration: 1, ease: 'linear' }, 99);

// Physics on the floor: a top-down 2D world (x, z), 100 units per metre.
const PS = 100;
let phys = null, physLive = false;
function makeWorld() {
  phys = world({ gravity: [0, 0], substeps: 4, iterations: 3, restitution: 0.42, friction: 0.04, damping: 0.25, bounds: [-380, -300, 380, 300] });
  for (let i = 0; i < N; i++) phys.add(0, 0, drops[i].r * PS * 0.98, 1);
  phys.attract(0, 0, 900, 600);
  phys.pause();
}

// Scroll position → a critically damped spring → the timeline and the camera.
const cam = { p: 0 };
let maxScroll = 1;
const SPRING = { stiffness: 42, damping: 2 * Math.sqrt(42) };
function onScroll() {
  const target = (clamp01(scrollY / maxScroll) * 100);
  if (reduced) cam.p = target; else spring(cam, { p: target }, SPRING);
}

// Pointer: parallax, light tilt and ripples, all on springs.
const ptr = { x: 0, y: 0 }, light = { x: 0, y: 0 }, rip = { x: 0, y: 1, s: 0 };
let px = 0, py = 0, lastMove = 0;
addEventListener('pointermove', (e) => {
  px = (e.clientX / innerWidth) * 2 - 1; py = 1 - (e.clientY / innerHeight) * 2;
  lastMove = performance.now();
  if (reduced) return;
  spring(ptr, { x: px, y: py }, { stiffness: 18, damping: 8.5 });
  spring(light, { x: px, y: py }, { stiffness: 30, damping: 11 });
  if (cam.p < 8 && hitTextPlane(px, py)) {
    spring(rip, { x: hitP[0], y: hitP[1] }, { stiffness: 120, damping: 22 });
    animate(rip, { s: [Math.max(rip.s, 0.006), 0] }, { duration: 2.4, ease: 'outCubic' });
  }
}, { passive: true });

// ---------------------------------------------------------------------------
// Camera state, no allocation per frame.
// ---------------------------------------------------------------------------
const eye = new Float32Array(3), aim = new Float32Array(3), camM = new Float32Array(9);
const fwd = [0, 0, 1], right = [1, 0, 0], up = [0, 1, 0];
let tanF = Math.tan(16 * deg);
function updateCamera(P) {
  sampleTab(eyeTab, P, eye); sampleTab(aimTab, P, aim);
  let fx = aim[0] - eye[0], fy = aim[1] - eye[1], fz = aim[2] - eye[2];
  let l = Math.hypot(fx, fy, fz); fx /= l; fy /= l; fz /= l;
  // right = fwd × worldUp
  let rx = -fz, rz = fx; l = Math.hypot(rx, rz) || 1; rx /= l; rz /= l;
  // Parallax: move the eye a little across the view.
  const k = reduced ? 0 : 1;
  eye[0] += rx * ptr.x * 0.12 * k; eye[2] += rz * ptr.x * 0.12 * k; eye[1] += ptr.y * 0.06 * k;
  fx = aim[0] - eye[0]; fy = aim[1] - eye[1]; fz = aim[2] - eye[2];
  l = Math.hypot(fx, fy, fz); fx /= l; fy /= l; fz /= l;
  rx = -fz; rz = fx; l = Math.hypot(rx, rz) || 1; rx /= l; rz /= l;
  const ux = -rz * fy, uy = rz * fx - rx * fz, uz = rx * fy;
  fwd[0] = fx; fwd[1] = fy; fwd[2] = fz; right[0] = rx; right[1] = 0; right[2] = rz; up[0] = ux; up[1] = uy; up[2] = uz;
  camM[0] = rx; camM[1] = 0; camM[2] = rz; camM[3] = ux; camM[4] = uy; camM[5] = uz; camM[6] = fx; camM[7] = fy; camM[8] = fz;
}
const hitP = [0, 0];
function hitTextPlane(nx, ny) {
  const asp = innerWidth / innerHeight;
  const dx = right[0] * nx * tanF * asp + up[0] * ny * tanF + fwd[0];
  const dy = right[1] * nx * tanF * asp + up[1] * ny * tanF + fwd[1];
  const dz = right[2] * nx * tanF * asp + up[2] * ny * tanF + fwd[2];
  if (Math.abs(dz) < 1e-4) return false;
  const t = -eye[2] / dz;
  if (t <= 0) return false;
  hitP[0] = eye[0] + dx * t; hitP[1] = eye[1] + dy * t;
  return true;
}
function rayFloor(nx, ny, y0, out) {
  const asp = innerWidth / innerHeight;
  const dx = right[0] * nx * tanF * asp + up[0] * ny * tanF + fwd[0];
  const dy = right[1] * nx * tanF * asp + up[1] * ny * tanF + fwd[1];
  const dz = right[2] * nx * tanF * asp + up[2] * ny * tanF + fwd[2];
  if (dy > -1e-4) return false;
  const t = (y0 - eye[1]) / dy;
  out[0] = eye[0] + dx * t; out[1] = eye[2] + dz * t;
  return true;
}

// Grab and fling droplets on the floor.
const grab = { i: -1, x: 0, z: 0, vx: 0, vz: 0, t: 0 };
const fp2 = [0, 0];
const onFloorBeat = () => cam.p > 44 && cam.p < 56.5;
canvas.addEventListener('pointerdown', (e) => {
  if (!phys || !physLive || !onFloorBeat()) return;
  const nx = (e.clientX / innerWidth) * 2 - 1, ny = 1 - (e.clientY / innerHeight) * 2;
  if (!rayFloor(nx, ny, 0.08, fp2)) return;
  const i = phys.pick(fp2[0] * PS, fp2[1] * PS);
  if (i < 0) return;
  grab.i = i; grab.x = fp2[0]; grab.z = fp2[1]; grab.vx = grab.vz = 0; grab.t = performance.now();
  canvas.setPointerCapture(e.pointerId);
  root.classList.add('grabbing');
});
canvas.addEventListener('pointermove', (e) => {
  if (grab.i < 0) return;
  const nx = (e.clientX / innerWidth) * 2 - 1, ny = 1 - (e.clientY / innerHeight) * 2;
  if (!rayFloor(nx, ny, 0.08, fp2)) return;
  const now = performance.now(), dt = Math.max(1, now - grab.t) / 1000;
  grab.vx = lerp(grab.vx, (fp2[0] - grab.x) / dt, 0.5); grab.vz = lerp(grab.vz, (fp2[1] - grab.z) / dt, 0.5);
  grab.x = fp2[0]; grab.z = fp2[1]; grab.t = now;
  phys.setPosition(grab.i, grab.x * PS, grab.z * PS);
  phys.setVelocity(grab.i, 0, 0);
});
const release = () => {
  if (grab.i < 0) return;
  phys.setVelocity(grab.i, grab.vx * PS, grab.vz * PS);
  grab.i = -1;
  root.classList.remove('grabbing');
};
canvas.addEventListener('pointerup', release);
canvas.addEventListener('pointercancel', release);

// ---------------------------------------------------------------------------
// The mist: an Anikit pointCloud, drawn as GPU points.
// ---------------------------------------------------------------------------
const NP = +(params.get('points') || (small ? 50000 : 100000));
let cloud = null, cloudPos = null, cloudBuf = null, cloudVao = null;
const CLOUD_C = [0, -3.1, 0];
const BAR_VALUES = [52, 21, 3.65];
function barLen(v) { return 60 + (Math.log10(v) / Math.log10(60)) * 240; }
function mistShape(i, n) {
  // A slow galaxy: a thin disc with arms, plus a halo.
  const a = rnd() * Math.PI * 2, u = rnd();
  if (i % 7 === 0) {
    const r = 80 + 200 * Math.sqrt(u), y = (rnd() - 0.5) * 180;
    return [Math.cos(a) * r, y, Math.sin(a) * r];
  }
  const arm = i % 3, rr = 30 + 250 * Math.pow(u, 0.7);
  const th = arm * 2.094 + rr * 0.012 + (rnd() - 0.5) * 0.7;
  return [Math.cos(th) * rr, (rnd() - 0.5) * (14 + rr * 0.08), Math.sin(th) * rr];
}
function barShape(i, n) {
  const b = i % 3;
  const len = barLen(BAR_VALUES[b]);
  const x = -150 + rnd() * len, y = 60 - b * 60 + (rnd() - 0.5) * 26, z = (rnd() - 0.5) * 26;
  return [x, y, z];
}
function makeCloud() {
  cloud = pointCloud(NP, { shapes: [mistShape, barShape], stiffness: 26, damping: 7, spread: 0.8, swirl: 55, noise: 3, autoStep: false });
  cloudBuf = gl.createBuffer();
  cloudVao = gl.createVertexArray();
  gl.bindVertexArray(cloudVao);
  gl.bindBuffer(gl.ARRAY_BUFFER, cloudBuf);
  gl.bufferData(gl.ARRAY_BUFFER, NP * 12, gl.DYNAMIC_DRAW);
  gl.enableVertexAttribArray(0);
  gl.vertexAttribPointer(0, 3, gl.FLOAT, false, 0, 0);
  gl.bindVertexArray(quadVao);
}

// ---------------------------------------------------------------------------
// DOM: beat labels, benchmark labels, install row.
// ---------------------------------------------------------------------------
const beats = $$('[data-beat]').map((el) => { const [a, b] = el.dataset.beat.split('-').map(Number); return { el, a, b, on: false }; });
const benchEls = $$('.bench-label');
const install = $('.install');
let installOn = false;
function setBeats(P) {
  for (const b of beats) {
    const on = P >= b.a && P < b.b;
    if (on !== b.on) {
      b.on = on;
      animate(b.el, { opacity: on ? 1 : 0, y: on ? 0 : 8 }, { duration: on ? 0.7 : 0.35, ease: on ? 'outCubic' : 'inQuad' });
    }
  }
  const io = P > 95;
  if (io !== installOn) { installOn = io; install.classList.toggle('on', io); animate(install, { opacity: io ? 1 : 0, y: io ? 0 : 14 }, { duration: io ? 0.9 : 0.35, ease: 'outCubic' }); }
}
const benchVis = { v: 0 };
let benchOn = false;
const bp = [0, 0];
function project(x, y, z, out) {
  const vx = x - eye[0], vy = y - eye[1], vz = z - eye[2];
  const qx = vx * right[0] + vy * right[1] + vz * right[2], qy = vx * up[0] + vy * up[1] + vz * up[2], qz = vx * fwd[0] + vy * fwd[1] + vz * fwd[2];
  const asp = innerWidth / innerHeight;
  out[0] = (qx / (qz * tanF * asp) * 0.5 + 0.5) * innerWidth;
  out[1] = (0.5 - qy / (qz * tanF) * 0.5) * innerHeight;
}
function placeBench(P) {
  const on = P > 79 && P < 87.5;
  if (on !== benchOn) { benchOn = on; animate(benchVis, { v: on ? 1 : 0 }, { duration: on ? 0.9 : 0.3, ease: 'outCubic' }); }
  for (let b = 0; b < 3; b++) {
    const el = benchEls[b];
    if (benchVis.v < 0.01) { if (el.style.visibility !== 'hidden') el.style.visibility = 'hidden'; continue; }
    el.style.visibility = '';
    const len = barLen(BAR_VALUES[b]);
    project(CLOUD_C[0] + (-150 + len + 22) / PS, CLOUD_C[1] + (60 - b * 60) / PS, CLOUD_C[2], bp);
    el.style.transform = `translate3d(${bp[0].toFixed(1)}px, ${bp[1].toFixed(1)}px, 0) translateY(-50%)`;
    el.style.opacity = benchVis.v;
  }
}

// ---------------------------------------------------------------------------
// The frame.
// ---------------------------------------------------------------------------
const D = new Float32Array(ND * 4), SQ = new Float32Array(ND).fill(1);
let time = 0, lastSeek = -1, fade = { v: 0 }, physWas = false;
const statsOut = { frames: 0, gpu: hdr ? 'hdr' : 'ldr', w: 0, h: 0 };
window.__mercury3d = statsOut;
statsOut.jump = (P) => { cam.p = P; };
const B1 = [A[2], 0, -A[0]];

function frame(dtMs) {
  const dt = Math.min(dtMs, 50) / 1000;
  if (!reduced) time += dt;
  const P = cam.p;
  if (Math.abs(P - lastSeek) > 1e-4) { tl.seek(P); lastSeek = P; }
  updateCamera(P);

  // Physics comes alive once the drops start landing.
  const live = P > 41 && P < 64;
  if (live && !physWas) {
    // Start every body under its drop.
    for (let i = 0; i < N; i++) { helixPos(drops[i], i, tmp3); phys.setPosition(i, tmp3[0] * PS, tmp3[2] * PS); phys.setVelocity(i, (rnd() - 0.5) * 60, (rnd() - 0.5) * 60); }
    phys.play();
  } else if (!live && physWas) phys.pause();
  physWas = physLive = live;

  // Droplets: helix → floor → pool.
  let gone = 0;
  const merge = st.merge;
  for (let i = 0; i < N; i++) {
    const d = drops[i];
    helixPos(d, i, tmp3);
    let x = tmp3[0], y = tmp3[1], z = tmp3[2];
    const sq = lerp(1, 0.8, sstep(0.3, 1, d.f));
    if (d.f > 0 || d.g > 0) {
      const wx = phys.x(i) / PS, wz = phys.y(i) / PS;
      x = lerp(x, wx, d.g); z = lerp(z, wz, d.g);
      y = lerp(y, d.r * sq, d.f);
    }
    let r = d.r * sstep(0, 0.12, d.h);
    // Merge: the drops run into one pool in the middle.
    if (merge > 0) {
      const k = sstep(i / N * 0.5, 0.5 + i / N * 0.5, merge);
      x *= 1 - k * 0.8; z *= 1 - k * 0.8; r *= 1 - k;
    }
    gone += clamp01(d.h * 3);
    D[i * 4] = x; D[i * 4 + 1] = y; D[i * 4 + 2] = z; D[i * 4 + 3] = r;
    SQ[i] = sq;
  }
  // The sphere gives up its volume as the drops leave it; at the end it rises again.
  const left = 1 - gone / N;
  const settle = st.settle;
  const rise = sstep(0.0, 0.45, settle);
  const sphR = 0.56 * Math.cbrt(Math.max(0, left)) * (st.m > 0.3 ? 1 : sstep(0.0, 0.3, st.m) ) ;
  const sphRFinal = lerp(sphR, 0.56 * rise, rise > 0 ? 1 : 0);
  const morph = settle > 0 ? 1 - sstep(0.4, 1, settle) : st.m;
  const showText = text.ready && morph < 0.999 && (st.m < 1 || settle > 0.4);

  const wipeRaw = sstep(0.32, -0.32, eye[1]);
  const wipe = wipeRaw <= 0 ? 0 : wipeRaw >= 1 ? 9 : wipeRaw * 1.9;

  setBeats(P);
  placeBench(P);
  if (!document.hidden) render(P, sphRFinal, morph, showText, wipe, rise);
  statsOut.frames++;
}
const tmp3 = [0, 0, 0];
function helixPos(d, i, out) {
  const u = N > 1 ? i / (N - 1) : 0;
  const k = (u - 0.5) * 4.6;
  const ang = u * Math.PI * 2 * 2.25 + time * 0.35;
  const R = 0.78 + 0.06 * Math.sin(u * 17 + time * 0.8);
  const hx = HC[0] + A[0] * k + (B1[0] * Math.cos(ang)) * R;
  const hy = HC[1] + Math.sin(ang) * R;
  const hz = HC[2] + A[2] * k + (B1[2] * Math.cos(ang)) * R;
  const sx = HC[0] + d.ox * 0.25, sy = HC[1] + d.oy * 0.25, sz = HC[2] + d.oz * 0.25;
  out[0] = lerp(sx, hx, d.h); out[1] = lerp(sy, hy, d.h); out[2] = lerp(sz, hz, d.h);
}

function render(P, sphR, morph, showText, wipe, rise) {
  const asp = W / H;
  tanF = Math.tan(16 * deg) * Math.max(1, 1.5 / (innerWidth / innerHeight));
  // Pass 1: the scene into the HDR buffer.
  gl.bindFramebuffer(gl.FRAMEBUFFER, targets.scene.fb);
  gl.viewport(0, 0, W, H);
  gl.disable(gl.BLEND);
  gl.useProgram(S.p);
  gl.bindVertexArray(quadVao);
  gl.uniform2f(S.uRes, W, H);
  gl.uniform1f(S.uTime, time);
  gl.uniformMatrix3fv(S.uCam, false, camM);
  gl.uniform3f(S.uEye, eye[0], eye[1], eye[2]);
  gl.uniform1f(S.uFov, tanF);
  gl.uniform4fv(S.uD, D);
  gl.uniform1fv(S.uSq, SQ);
  gl.uniform1i(S.uN, N);
  gl.activeTexture(gl.TEXTURE0);
  gl.bindTexture(gl.TEXTURE_2D, textTex);
  gl.uniform1i(S.uTex, 0);
  gl.uniform4f(S.uTR, 0, TEXT_Y, text.hw, text.hh);
  const breathe = reduced ? 0 : 0.004 * Math.sin(time * 1.1);
  gl.uniform4f(S.uTA, 0.088, morph, breathe, showText ? 1 : 0);
  const sy = st.settle > 0 ? lerp(0.05, TEXT_Y, rise) : TEXT_Y;
  gl.uniform4f(S.uSph, 0, sy, 0, sphR);
  const merge = st.merge;
  const poolR = 1.55 * Math.sqrt(merge) * (1 - 0.35 * rise);
  gl.uniform4f(S.uPool, poolR, 0.16 * Math.sqrt(merge), st.portal * 0.035, merge > 0.001 ? 1 : 0);
  gl.uniform4f(S.uRip, rip.x, rip.y, time, rip.s);
  gl.uniform2f(S.uLight, light.x, light.y);
  gl.uniform1f(S.uK, lerp(0.16, 0.12, sstep(40, 46, P)));
  gl.uniform1f(S.uWipe, wipe);
  gl.uniform2f(S.uEyeY, Math.max(eye[1], 0.03), Math.min(eye[1], -0.03));
  gl.uniform3f(S.uCloudC, CLOUD_C[0], CLOUD_C[1], CLOUD_C[2]);
  gl.uniform1f(S.uOut, hdr ? 1 : 0.25);
  gl.drawArrays(gl.TRIANGLES, 0, 3);

  // Pass 2: the mist, additive, only below the surface.
  const cloudOn = cloud && P > 63 && P < 92;
  if (cloudOn) {
    cloud.mix = st.mix;
    if (!reduced) cloud.step(dtLast);
    if (!cloudPos || cloudPos.length === 0) cloudPos = cloud.positions;
    gl.bindBuffer(gl.ARRAY_BUFFER, cloudBuf);
    gl.bufferSubData(gl.ARRAY_BUFFER, 0, cloudPos);
    gl.useProgram(PTS.p);
    gl.bindVertexArray(cloudVao);
    gl.uniform2f(PTS.uRes, W, H);
    gl.uniform1f(PTS.uTime, time);
    gl.uniformMatrix3fv(PTS.uCam, false, camM);
    gl.uniform3f(PTS.uEye, eye[0], Math.min(eye[1], -0.03), eye[2]);
    gl.uniform1f(PTS.uFov, tanF);
    gl.uniform3f(PTS.uOff, CLOUD_C[0], CLOUD_C[1], CLOUD_C[2]);
    gl.uniform1f(PTS.uScale, 1 / PS);
    gl.uniform1f(PTS.uSpin, (1 - sstep(0, 0.5, st.mix)) * time * 0.12);
    gl.uniform1f(PTS.uSize, 0.0042);
    gl.uniform1f(PTS.uWipe, wipe);
    gl.uniform1f(PTS.uGain, (hdr ? 1 : 0.25) * (0.55 + 0.6 * st.mix) * Math.min(1, 240000 / (NP * 2.4)) * sstep(63, 67, P) * (1 - sstep(89, 91.5, P)));
    gl.enable(gl.BLEND);
    gl.blendFunc(gl.ONE, gl.ONE);
    gl.drawArrays(gl.POINTS, 0, NP);
    gl.disable(gl.BLEND);
    gl.bindVertexArray(quadVao);
  }

  // Pass 3: bloom at quarter resolution.
  const a = targets.a, b = targets.b;
  gl.bindFramebuffer(gl.FRAMEBUFFER, a.fb);
  gl.viewport(0, 0, a.w, a.h);
  gl.useProgram(BR.p);
  gl.bindTexture(gl.TEXTURE_2D, targets.scene.tex);
  gl.uniform1i(BR.uSrc, 0);
  gl.uniform2f(BR.uTexel, 1 / a.w, 1 / a.h);
  gl.drawArrays(gl.TRIANGLES, 0, 3);
  gl.useProgram(BL.p);
  gl.uniform1i(BL.uSrc, 0);
  gl.uniform2f(BL.uTexel, 1 / a.w, 1 / a.h);
  for (let k = 0; k < 2; k++) {
    gl.bindFramebuffer(gl.FRAMEBUFFER, b.fb);
    gl.bindTexture(gl.TEXTURE_2D, a.tex);
    gl.uniform2f(BL.uDir, (1 + k) / a.w, 0);
    gl.drawArrays(gl.TRIANGLES, 0, 3);
    gl.bindFramebuffer(gl.FRAMEBUFFER, a.fb);
    gl.bindTexture(gl.TEXTURE_2D, b.tex);
    gl.uniform2f(BL.uDir, 0, (1 + k) / a.h);
    gl.drawArrays(gl.TRIANGLES, 0, 3);
  }

  // Pass 4: tone map, vignette, dither, to the screen.
  gl.bindFramebuffer(gl.FRAMEBUFFER, null);
  gl.viewport(0, 0, W, H);
  gl.useProgram(CO.p);
  gl.activeTexture(gl.TEXTURE0);
  gl.bindTexture(gl.TEXTURE_2D, targets.scene.tex);
  gl.uniform1i(CO.uSrc, 0);
  gl.activeTexture(gl.TEXTURE1);
  gl.bindTexture(gl.TEXTURE_2D, a.tex);
  gl.uniform1i(CO.uBloom, 1);
  gl.activeTexture(gl.TEXTURE0);
  gl.uniform2f(CO.uRes, W, H);
  gl.uniform1f(CO.uFade, 1);
  gl.uniform1f(CO.uScaleIn, hdr ? 1 : 4);
  gl.drawArrays(gl.TRIANGLES, 0, 3);
  statsOut.w = W; statsOut.h = H;
}
let dtLast = 1 / 60;

// ---------------------------------------------------------------------------
// Start.
// ---------------------------------------------------------------------------
function measure() {
  maxScroll = Math.max(1, document.documentElement.scrollHeight - innerHeight);
}
measure();
addEventListener('scroll', onScroll, { passive: true });
addEventListener('resize', () => {
  measure();
  if (innerWidth !== lastCssW) chooseResolution();
  onScroll();
});

if (reduced) engine.reducedMotion = 'always';
await ready;
await Promise.race([document.fonts.load('900 100px "Inter Display"'), new Promise((r) => setTimeout(r, 1200))]);
makeWorld();
makeCloud();
await buildText();
cam.p = clamp01(scrollY / maxScroll) * 100;
engine.onFrame((_t, dt) => { dtLast = Math.min(dt, 50) / 1000; frame(dt); });
// Reveal: the canvas fades in over the matching static frame once the first real frame is drawn.
requestAnimationFrame(() => requestAnimationFrame(() => {
  root.classList.add('live');
  animate('#gl', { opacity: [0, 1] }, { duration: 1.1, ease: 'inOutSine' });
  animate('.still', { opacity: [1, 0] }, { duration: 1.4, ease: 'inOutSine', delay: 0.15 });
}));

// Copy button.
$('.copy')?.addEventListener('click', async (e) => {
  const btn = e.currentTarget;
  try { await navigator.clipboard.writeText('npm i anikit'); btn.dataset.done = '1'; btn.textContent = 'copied'; setTimeout(() => { btn.textContent = 'copy'; delete btn.dataset.done; }, 1600); } catch { /* clipboard blocked */ }
});
