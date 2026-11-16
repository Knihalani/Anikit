// Liquid mercury: one ray-marched metal scene behind the whole page.
// Anikit springs move the drops, a physics world() takes them over as beads,
// and the scroll position decides which stage the metal flows into.
import { engine, ready, spring, world, animate, smoothScroll, inView } from '../../../packages/anikit/dist/anikit.lean.js';
import { vert, frag, MAX_DROPS } from './shader.js';
import { startDemos } from './demos.js';

const $ = (s, r = document) => r.querySelector(s);
const $$ = (s, r = document) => Array.from(r.querySelectorAll(s));
const reduced = matchMedia('(prefers-reduced-motion: reduce)').matches;
const fine = matchMedia('(hover: hover) and (pointer: fine)').matches;
const small = innerWidth < 700;
const clamp01 = (v) => (v < 0 ? 0 : v > 1 ? 1 : v);
const sstep = (a, b, x) => { const t = clamp01((x - a) / (b - a)); return t * t * (3 - 2 * t); };
const lerp = (a, b, t) => a + (b - a) * t;

let vw = innerWidth, vh = innerHeight;
const N = small ? 14 : MAX_DROPS;

// ---------------------------------------------------------------------------
// WebGL
// ---------------------------------------------------------------------------
const canvas = $('#metal');
const gl = canvas.getContext('webgl2', { antialias: false, alpha: false, depth: false, stencil: false, powerPreference: 'high-performance' });
const root = document.documentElement;
let prog = null, U = {}, tex = null;
function setupGL() {
  const sh = (type, src) => { const s = gl.createShader(type); gl.shaderSource(s, src); gl.compileShader(s); if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s)); return s; };
  prog = gl.createProgram();
  gl.attachShader(prog, sh(gl.VERTEX_SHADER, vert));
  gl.attachShader(prog, sh(gl.FRAGMENT_SHADER, frag));
  gl.bindAttribLocation(prog, 0, 'aPos');
  gl.linkProgram(prog);
  if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(prog));
  gl.useProgram(prog);
  const buf = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, buf);
  gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 3, -1, -1, 3]), gl.STATIC_DRAW);
  gl.enableVertexAttribArray(0);
  gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0);
  for (const n of ['uRes', 'uTime', 'uD', 'uTex', 'uTR', 'uTA', 'uMelt', 'uRimA', 'uRimAP', 'uRimB', 'uRimBP', 'uNeck', 'uNeckR', 'uPool', 'uLight', 'uK']) U[n] = gl.getUniformLocation(prog, n);
  tex = gl.createTexture();
  gl.bindTexture(gl.TEXTURE_2D, tex);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
  gl.texImage2D(gl.TEXTURE_2D, 0, gl.R16F, 1, 1, 0, gl.RED, gl.FLOAT, new Float32Array([1000]));
  gl.uniform1i(U.uTex, 0);
}
let glOk = !!gl;
if (glOk) { try { setupGL(); } catch (e) { console.warn(e); glOk = false; } }
root.classList.add(glOk ? 'gl' : 'no-gl');

// Resolution adapts to the frame rate: start modest, step down when frames run long,
// step back up when there is headroom.
const qMax = Math.min(devicePixelRatio || 1, small ? 1 : 1.25);
let q = Math.min(qMax, +(new URLSearchParams(location.search).get('q') || (small ? 0.6 : 0.8)));
const qMin = 0.35;
function sizeCanvas() {
  const w = Math.max(2, Math.round(vw * q)), h = Math.max(2, Math.round(vh * q));
  if (canvas.width !== w || canvas.height !== h) { canvas.width = w; canvas.height = h; }
}

// ---------------------------------------------------------------------------
// The wordmark as a distance field: draw it, then a two-pass exact EDT.
// ---------------------------------------------------------------------------
const text = { ready: false, cx: 0, cy: 0, hw: 0, hh: 0, scale: 0, glyphH: 0.3, homes: [] };
function edt(grid, W, H) {
  const INF = 1e20, n = Math.max(W, H);
  const f = new Float64Array(n), d = new Float64Array(n), v = new Int32Array(n), z = new Float64Array(n + 1);
  const pass = (get, set, len) => {
    for (let q = 0; q < len; q++) f[q] = get(q);
    let k = 0; v[0] = 0; z[0] = -INF; z[1] = INF;
    for (let q = 1; q < len; q++) {
      let s = ((f[q] + q * q) - (f[v[k]] + v[k] * v[k])) / (2 * q - 2 * v[k]);
      while (s <= z[k]) { k--; s = ((f[q] + q * q) - (f[v[k]] + v[k] * v[k])) / (2 * q - 2 * v[k]); }
      k++; v[k] = q; z[k] = s; z[k + 1] = INF;
    }
    k = 0;
    for (let q = 0; q < len; q++) { while (z[k + 1] < q) k++; d[q] = (q - v[k]) * (q - v[k]) + f[v[k]]; }
    for (let q = 0; q < len; q++) set(q, d[q]);
  };
  for (let x = 0; x < W; x++) pass((y) => grid[y * W + x], (y, val) => (grid[y * W + x] = val), H);
  for (let y = 0; y < H; y++) pass((x) => grid[y * W + x], (x, val) => (grid[y * W + x] = val), W);
}
function buildText() {
  const W = 1024, pad = 56;
  const c = document.createElement('canvas');
  const ctx = c.getContext('2d', { willReadFrequently: true });
  ctx.font = '900 200px "Inter Display", Inter, system-ui, sans-serif';
  let m = ctx.measureText('anikit');
  const fs = (200 * (W - 2 * pad)) / (m.actualBoundingBoxRight + m.actualBoundingBoxLeft);
  ctx.font = `900 ${fs}px "Inter Display", Inter, system-ui, sans-serif`;
  m = ctx.measureText('anikit');
  const gh = m.actualBoundingBoxAscent + m.actualBoundingBoxDescent;
  const H = Math.ceil(gh + 2 * pad);
  c.width = W; c.height = H;
  ctx.font = `900 ${fs}px "Inter Display", Inter, system-ui, sans-serif`;
  ctx.letterSpacing = `${-0.045 * fs}px`;
  ctx.fillStyle = '#fff';
  ctx.textBaseline = 'alphabetic';
  m = ctx.measureText('anikit');
  const left = (W - (m.actualBoundingBoxRight + m.actualBoundingBoxLeft)) / 2 + m.actualBoundingBoxLeft;
  ctx.fillText('anikit', left, pad + m.actualBoundingBoxAscent);
  const px = ctx.getImageData(0, 0, W, H).data;
  const inside = new Uint8Array(W * H);
  for (let i = 0; i < W * H; i++) inside[i] = px[i * 4 + 3] > 127 ? 1 : 0;
  const a = new Float64Array(W * H), b = new Float64Array(W * H);
  for (let i = 0; i < W * H; i++) { a[i] = inside[i] ? 0 : 1e20; b[i] = inside[i] ? 1e20 : 0; }
  edt(a, W, H); edt(b, W, H);
  const sd = new Float32Array(W * H);
  // Rows flipped so texture v grows upwards like world y.
  for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
    const i = y * W + x, o = (H - 1 - y) * W + x;
    sd[o] = inside[i] ? -(Math.sqrt(b[i]) - 0.5) : Math.sqrt(a[i]) - 0.5;
  }
  gl.bindTexture(gl.TEXTURE_2D, tex);
  gl.texImage2D(gl.TEXTURE_2D, 0, gl.R16F, W, H, 0, gl.RED, gl.FLOAT, sd);
  text.W = W; text.H = H; text.pad = pad; text.sd = sd; text.gh = gh;
  // Homes for the drops: the deepest point of evenly spaced columns.
  text.cols = [];
  for (let i = 0; i < N; i++) {
    let best = null;
    for (let off = 0; off < 60 && !best; off += 4) for (const s of [1, -1]) {
      const x = Math.round(pad + ((i + 0.5) / N) * (W - 2 * pad) + s * off);
      let by = -1, bd = 0;
      for (let y = 0; y < H; y++) { const v = sd[y * W + x]; if (v < bd) { bd = v; by = y; } }
      if (by >= 0 && (!best || bd < best.d)) best = { x, y: by, d: bd };
      if (best) break;
    }
    text.cols.push(best || { x: W / 2, y: H / 2, d: -10 });
  }
  text.ready = true;
  layoutText();
}
function layoutText() {
  if (!text.W) return;
  const aspect = vw / vh;
  const glyphW = Math.min(aspect * (small ? 0.84 : 0.74), 1.12);
  const worldW = (glyphW * text.W) / (text.W - 2 * text.pad);
  text.scale = worldW / text.W;
  text.hw = worldW / 2;
  text.hh = (text.H * text.scale) / 2;
  text.cx = aspect / 2;
  text.cy = small ? 0.55 : 0.545;
  text.glyphH = text.gh * text.scale;
  // The HTML wordmark (shown at once, before WebGL) sits exactly where the metal one forms.
  const h1 = $('.wordmark');
  h1.style.fontSize = '100px';
  const k = (glyphW * vh) / h1.firstElementChild.getBoundingClientRect().width;
  h1.style.fontSize = `${100 * Math.min(k, 4)}px`;
  h1.style.top = `${(1 - text.cy) * 100}%`;
  text.homes = text.cols.map((c) => ({
    x: (text.cx - text.hw + (c.x + 0.5) * text.scale) * vh,
    y: vh - (text.cy - text.hh + (c.y + 0.5) * text.scale) * vh,
    depth: -c.d * text.scale,
  }));
}

// ---------------------------------------------------------------------------
// Stops: the hero, each demo stage, and the install row.
// ---------------------------------------------------------------------------
const sections = $$('.stop');
const stops = [{ kind: 'hero', el: $('#top') }, ...sections.map((el) => ({ kind: el.id, el, stage: $('.stage', el), pin: $('.pin', el) })), { kind: 'install', el: $('#install'), stage: $('#install-row') }];
const stopIndex = Object.fromEntries(stops.map((s, i) => [s.kind, i]));
function measure() {
  vw = innerWidth; vh = innerHeight;
  const y = scrollY;
  for (const s of stops) {
    s.top = s.el.getBoundingClientRect().top + y;
    s.h = s.el.offsetHeight;
    if (s.stage) {
      const r = s.stage.getBoundingClientRect();
      s.sw = r.width; s.sh = r.height; s.sl = r.left;
      s.docTop = r.top + y;
      s.offTop = s.pin ? r.top - s.pin.getBoundingClientRect().top : 0;
      s.radius = s.kind === 'install' ? r.height / 2 : parseFloat(getComputedStyle(s.stage).borderTopLeftRadius) || 28;
    }
  }
  layoutText();
  sizeCanvas();
  if (phys) phys.set({ bounds: physBounds() });
}
// Where the stage is on screen for a scroll position, and how far (in viewports) it is from resting centred.
function place(s, y) {
  if (s.kind === 'hero') { s.d = y / vh; return; }
  if (s.pin) {
    const holdEnd = s.top + s.h - vh;
    const pinTop = y < s.top ? s.top - y : y > holdEnd ? holdEnd - y : 0;
    s.st = pinTop + s.offTop;
    s.d = (y < s.top ? s.top - y : y > holdEnd ? y - holdEnd : 0) / vh;
  } else {
    s.st = s.docTop - y;
    s.d = Math.abs(s.st + s.sh / 2 - vh / 2) / vh;
  }
}

// ---------------------------------------------------------------------------
// Drops
// ---------------------------------------------------------------------------
let seed = 7;
const rnd = () => ((seed = (seed * 1664525 + 1013904223) >>> 0) / 4294967296);
const drops = Array.from({ length: N }, (_, i) => {
  const a = rnd() * Math.PI * 2, rr = 0.25 + rnd() * 0.25;
  return {
    pos: { x: vw / 2 + Math.cos(a) * rr * vh, y: vh * 0.45 + Math.sin(a) * rr * vh * 0.6 },
    out: { x: 0, y: 0 }, prev: { x: 0, y: 0 }, vx: 0, vy: 0,
    r: 0.019 + rnd() * 0.017, rad: 0.0, phase: rnd() * Math.PI * 2, dir: rnd() < 0.5 ? -1 : 1, lean: rnd() * 2 - 1,
    cfg: { stiffness: 55 + (i % 5) * 14, damping: 8 + (i % 3) * 1.6 },
  };
});
for (const d of drops) { d.out.x = d.prev.x = d.pos.x; d.out.y = d.prev.y = d.pos.y; }

const state = { form: 0, melt: 0, t: 0, wm: 0, chain: 0 };
const pointer = { x: vw / 2, y: vh / 2, at: -1e9, inHero: false };
const light = { x: 0, y: 0 };
const chainCfgs = [{ stiffness: 260, damping: 16 }, { stiffness: 180, damping: 7 }, { stiffness: 40, damping: 10 }];
let chainCfg = chainCfgs[0];
export function setChainPreset(k) { chainCfg = chainCfgs[k]; }

addEventListener('pointermove', (e) => {
  pointer.x = e.clientX; pointer.y = e.clientY; pointer.at = performance.now();
  if (fine && !reduced) spring(light, { x: (e.clientX / vw) * 2 - 1, y: 1 - (e.clientY / vh) * 2 }, { stiffness: 30, damping: 11 });
}, { passive: true });

// Hero: drops live inside the letters and bud off now and then; the pointer pulls one out.
let pulled = -1, neckR = 0, neckBroken = false;
function heroTarget(d, i, t, out) {
  const h = text.homes[i];
  if (!h) { out.x = d.pos.x; out.y = d.pos.y; out.r = d.r; return; }
  const bud = reduced ? 0 : Math.pow(Math.max(0, Math.sin(t * 0.42 + d.phase)), 6);
  const reach = text.glyphH * 0.42;
  let x = h.x + d.lean * bud * reach * 0.45 * vh, y = h.y + d.dir * bud * reach * vh;
  // Melting: everything slides down into a puddle under the letters.
  const m = sstep(0.05, 0.85, state.melt);
  const px = text.cx * vh + ((i + 0.5) / N - 0.5) * text.hw * 1.5 * vh;
  const py = vh - (text.cy - text.hh * 0.62) * vh + Math.sin(i * 2.3) * 0.02 * vh;
  x = lerp(x, px, m); y = lerp(y, py, m);
  if (i === pulled && !neckBroken) { x = pointer.x; y = pointer.y; }
  else if (i === pulled) { x = pointer.x; y = pointer.y; }
  out.x = x; out.y = y;
  out.r = Math.min(d.r, Math.max(0.012, h.depth * 1.15)) * (1 + 0.35 * bud) * (1 - 0.25 * m);
  if (i === pulled) out.r = Math.max(out.r, 0.026);
}

// A point on a rounded rectangle's outline, u in [0, 1).
function rimPoint(x0, y0, w, h, r, u, out) {
  r = Math.min(r, w / 2, h / 2);
  const lw = w - 2 * r, lh = h - 2 * r, arc = (Math.PI * r) / 2, total = 2 * lw + 2 * lh + 4 * arc;
  let s = (((u % 1) + 1) % 1) * total;
  const segs = [
    [lw, (k) => [x0 + r + k * lw, y0]],
    [arc, (k) => { const a = -Math.PI / 2 + k * (Math.PI / 2); return [x0 + w - r + Math.cos(a) * r, y0 + r + Math.sin(a) * r]; }],
    [lh, (k) => [x0 + w, y0 + r + k * lh]],
    [arc, (k) => { const a = k * (Math.PI / 2); return [x0 + w - r + Math.cos(a) * r, y0 + h - r + Math.sin(a) * r]; }],
    [lw, (k) => [x0 + w - r - k * lw, y0 + h]],
    [arc, (k) => { const a = Math.PI / 2 + k * (Math.PI / 2); return [x0 + r + Math.cos(a) * r, y0 + h - r + Math.sin(a) * r]; }],
    [lh, (k) => [x0, y0 + h - r - k * lh]],
    [arc, (k) => { const a = Math.PI + k * (Math.PI / 2); return [x0 + r + Math.cos(a) * r, y0 + r + Math.sin(a) * r]; }],
  ];
  for (const [len, fn] of segs) { if (s <= len) { const [x, y] = fn(len > 0 ? s / len : 0); out.x = x; out.y = y; return out; } s -= len; }
  out.x = x0; out.y = y0; return out;
}
const RIM_GAP = 7; // px between the stage edge and the metal ring's centre line
function stageTarget(s, d, i, t, out) {
  const g = RIM_GAP;
  if (s.kind === 'physics' && state.wm > 0 && phys) {
    const bx = s.sl + phys.x(i), by = s.st + phys.y(i);
    rimPoint(s.sl - g, s.st - g, s.sw + 2 * g, s.sh + 2 * g, s.radius + g, i / N + t * 0.01, out);
    out.x = lerp(out.x, bx, state.wm); out.y = lerp(out.y, by, state.wm);
    out.r = lerp(d.r * 0.8, d.r * 1.45, state.wm);
    return;
  }
  if (s.kind === 'springs' && state.chain > 0 && i < CHAIN) {
    rimPoint(s.sl - g, s.st - g, s.sw + 2 * g, s.sh + 2 * g, s.radius + g, i / N + t * 0.01, out);
    const c = chain[i];
    out.x = lerp(out.x, s.sl + c.x, state.chain); out.y = lerp(out.y, s.st + c.y, state.chain);
    out.r = lerp(d.r * 0.8, 0.036 * (1 - (i / CHAIN) * 0.6), state.chain);
    return;
  }
  const dir = stopIndex[s.kind] % 2 ? 1 : -1;
  rimPoint(s.sl - g, s.st - g, s.sw + 2 * g, s.sh + 2 * g, s.radius + g, i / N + dir * t * 0.012 + stopIndex[s.kind] * 0.07, out);
  out.r = d.r * (s.kind === 'install' ? 0.62 : 0.8);
}

// ---------------------------------------------------------------------------
// Physics stage: the drops become beads in a world() you can throw.
// ---------------------------------------------------------------------------
let phys = null, physOn = false, lastToss = 0, lastGrab = -1e9;
const physStop = stops[stopIndex.physics];
const physBounds = () => [6, 6, physStop.sw - 6, physStop.sh - 6];
function makeWorld() {
  phys = world({ container: physStop.stage, bounds: physBounds(), gravity: [0, 2400], substeps: 4, iterations: 3, restitution: 0.38, friction: 0.06, damping: 0.01 });
  for (let i = 0; i < N; i++) phys.add(physStop.sw / 2 + (i - N / 2) * 20, -200 - i * 30, drops[i].r * 1.45 * vh, 1 + drops[i].r * 40);
  phys.dragging({ hitSlop: 18 });
  phys.pause();
  physStop.stage.addEventListener('pointerdown', () => (lastGrab = performance.now()));
}
function physicsToggle(on, s) {
  if (on === physOn || !phys) return;
  physOn = on;
  if (on) {
    for (let i = 0; i < N; i++) {
      const d = drops[i];
      const x = Math.min(Math.max(d.out.x - s.sl, 20), s.sw - 20), y = Math.min(Math.max(d.out.y - s.st, 20), s.sh - 20);
      phys.setRadius?.(i, d.r * 1.45 * vh);
      phys.setPosition(i, x, y);
      phys.setVelocity(i, d.vx * 0.5, d.vy * 0.5);
    }
    phys.play();
    animate(state, { wm: 1 }, { duration: 0.9, ease: 'inOutCubic' });
  } else {
    animate(state, { wm: 0 }, { duration: 0.9, ease: 'inOutCubic', onComplete: () => { if (!physOn) phys.pause(); } });
  }
}

// Springs stage: a chain of drops, each springing after the one ahead.
const CHAIN = Math.min(12, N);
const chain = Array.from({ length: CHAIN }, () => ({ x: 0, y: 0 }));
let chainOn = false;
const springStop = stops[stopIndex.springs];
function chainToggle(on) {
  if (on === chainOn) return;
  chainOn = on;
  if (on) for (let i = 0; i < CHAIN; i++) { chain[i].x = drops[i].out.x - springStop.sl; chain[i].y = drops[i].out.y - springStop.st; }
  animate(state, { chain: on ? 1 : 0 }, { duration: 1.1, ease: 'inOutCubic' });
}
function stepChain(t) {
  const s = springStop;
  const recent = performance.now() - pointer.at < 2500;
  const px = pointer.x - s.sl, py = pointer.y - s.st;
  const inside = recent && px > 0 && py > 0 && px < s.sw && py < s.sh;
  const aim = inside ? { x: px, y: py } : reduced ? { x: s.sw / 2, y: s.sh * 0.45 } :
    { x: s.sw / 2 + Math.sin(t * 1.05) * s.sw * 0.32, y: s.sh * 0.44 + Math.sin(t * 2.1) * s.sh * 0.24 };
  spring(chain[0], aim, chainCfg);
  for (let i = 1; i < CHAIN; i++) spring(chain[i], { x: chain[i - 1].x, y: chain[i - 1].y }, chainCfg);
}

// ---------------------------------------------------------------------------
// Frame
// ---------------------------------------------------------------------------
const D = new Float32Array(MAX_DROPS * 4);
const tmpA = { x: 0, y: 0, r: 0 }, tmpB = { x: 0, y: 0, r: 0 };
const fade = sections.map((el) => ({ el, items: [$('h2', el), ...$$('.stage > *', el), $('.snippet', el)].filter(Boolean), last: -1 }));
const heroFade = { items: [$('.lede'), $('.cue')], last: -1 };
let frameAvg = 16.7, frames = 0, lastNow = performance.now();
const stats = { q: 0, w: 0, h: 0, ms: 0 };
window.__mercury = stats;

function rimUniform(locR, locP, s) {
  if (!s || s.kind === 'hero') { gl.uniform4f(locP, 0, 0, 0, 0); return; }
  const f = 1 - sstep(0.04, 0.46, s.d);
  const g = RIM_GAP;
  const x0 = s.sl - g, y0 = s.st - g, w = s.sw + 2 * g, h = s.sh + 2 * g;
  gl.uniform4f(locR, (x0 + w / 2) / vh, (vh - (y0 + h / 2)) / vh, w / 2 / vh, h / 2 / vh);
  gl.uniform4f(locP, Math.min(s.radius + g, h / 2) / vh, f * 0.0095 * Math.min(1.2, Math.max(0.75, 720 / vh)), sstep(0.3, 1, f) * 0.9, 0);
}

function frame(dtMs) {
  const dt = Math.min(dtMs, 50) / 1000;
  if (!reduced) state.t += dt;
  const t = state.t;
  const y = scrollY;
  for (const s of stops) place(s, y);
  state.melt = clamp01(y / (vh * 0.85));

  // The two nearest stops, and how far the metal has travelled from one to the other.
  let a = stops[0], b = stops[1];
  for (const s of stops) { if (s.d < a.d) { b = a; a = s; } else if (s !== a && s.d < b.d) b = s; }
  if (b.d < a.d) [a, b] = [b, a];
  const wRaw = a.d + b.d > 0 ? a.d / (a.d + b.d) : 0;
  const w = sstep(0.12, 0.88, wRaw);

  physicsToggle(physStop.d < 0.12, physStop);
  chainToggle(springStop.d < 0.2);
  if (state.chain > 0) stepChain(t);
  if (physOn && !reduced && performance.now() - lastGrab > 5000 && t - lastToss > 3.6) {
    lastToss = t;
    phys.impulse(physStop.sw * (0.25 + rnd() * 0.5), physStop.sh + 10, physStop.sw * 0.45, 1500 + rnd() * 700);
  }

  // Hero pointer pull.
  const recent = performance.now() - pointer.at < 2200;
  const overHero = a.kind === 'hero' && state.melt < 0.25 && text.ready && state.form > 0.95;
  if (overHero && recent && !reduced) {
    if (pulled < 0) {
      let best = 1e9;
      text.homes.forEach((h, i) => { const dd = Math.hypot(h.x - pointer.x, h.y - pointer.y); if (dd < best) { best = dd; pulled = i; } });
      if (best > vh * 0.35) pulled = -1;
      neckBroken = false;
    }
  } else if (pulled >= 0) {
    const h = text.homes[pulled], dp = drops[pulled];
    if (!h || Math.hypot(dp.out.x - h.x, dp.out.y - h.y) < 0.02 * vh || !overHero) { pulled = -1; neckBroken = false; }
  }

  for (let i = 0; i < N; i++) {
    const d = drops[i];
    const ta = target(a, d, i, t, tmpA), tb = target(b, d, i, t, tmpB);
    const tx = lerp(ta.x, tb.x, w), ty = lerp(ta.y, tb.y, w), tr = lerp(ta.r, tb.r, w) * (1 + 0.6 * Math.sin(Math.PI * w));
    // In transit the ring narrows into a falling stream, then opens out again.
    const mid = Math.sin(Math.PI * w);
    const sx = lerp(tx, vw / 2 + (tx - vw / 2) * 0.22, mid * mid);
    const formed = a.kind !== 'hero' || state.form > 0.02;
    if (formed) spring(d.pos, { x: sx, y: ty }, i === pulled ? { stiffness: 90, damping: 13 } : d.cfg);
    d.rad += (tr * (a.kind === 'hero' && b.d > 0.6 ? lerp(0.35, 1, state.form) : 1) - d.rad) * Math.min(1, dt * 5);
    d.prev.x = d.out.x; d.prev.y = d.out.y;
    d.out.x = d.pos.x; d.out.y = d.pos.y;
    if (dt > 0) { d.vx = (d.out.x - d.prev.x) / dt; d.vy = (d.out.y - d.prev.y) / dt; }
  }

  // Neck between a pulled drop and the letter it came from.
  let neck = 0;
  if (pulled >= 0 && text.homes[pulled]) {
    const h = text.homes[pulled], dp = drops[pulled];
    const dist = Math.hypot(dp.out.x - h.x, dp.out.y - h.y) / vh;
    if (dist > 0.2) neckBroken = true;
    if (neckBroken && dist < 0.03) neckBroken = false;
    neck = neckBroken ? 0 : dp.rad * 0.62 * clamp01(1 - dist / 0.2) ** 0.6;
  }
  neckR += (neck - neckR) * Math.min(1, dt * (neck < neckR ? 14 : 6));

  // UI fades: content appears once the ring has formed, leaves before it breaks.
  for (let k = 0; k < fade.length; k++) {
    const s = stops[k + 1], o = sstep(0.62, 0.96, 1 - sstep(0.04, 0.46, s.d));
    const r = Math.round(o * 100) / 100;
    if (r !== fade[k].last) { fade[k].last = r; for (const el of fade[k].items) { el.style.opacity = r; el.style.visibility = r === 0 ? 'hidden' : ''; } }
  }
  const ho = Math.round((1 - sstep(0.02, 0.3, state.melt)) * 100) / 100;
  if (ho !== heroFade.last) { heroFade.last = ho; for (const el of heroFade.items) el.style.opacity = ho; }

  if (glOk && !document.hidden) render(a, b, t);
}
function target(s, d, i, t, out) {
  if (s.kind === 'hero') { heroTarget(d, i, t, out); return out; }
  stageTarget(s, d, i, t, out);
  return out;
}

function render(a, b, t) {
  const now = performance.now();
  const ft = now - lastNow; lastNow = now;
  if (ft < 1000) frameAvg = frameAvg * 0.94 + ft * 0.06;
  if (++frames % 30 === 0) {
    if (frameAvg > 19 && q > qMin) { q = Math.max(qMin, q * 0.86); sizeCanvas(); }
    else if (frameAvg < 15.5 && q < qMax && frames > 120) { q = Math.min(qMax, q * 1.06); sizeCanvas(); }
  }
  gl.viewport(0, 0, canvas.width, canvas.height);
  gl.uniform2f(U.uRes, canvas.width, canvas.height);
  gl.uniform1f(U.uTime, t);
  D.fill(0);
  for (let i = 0; i < N; i++) {
    const d = drops[i];
    D[i * 4] = d.out.x / vh; D[i * 4 + 1] = (vh - d.out.y) / vh; D[i * 4 + 2] = d.rad;
  }
  gl.uniform4fv(U.uD, D);
  // Wordmark: grows out of the metal, then melts with the scroll.
  const m = state.melt;
  const T = text.glyphH * 0.085 * (1 - 0.55 * m);
  const show = text.ready && m < 0.995;
  gl.uniform4f(U.uTR, text.cx, text.cy - m * 0.04, text.hw, text.hh);
  gl.uniform3f(U.uTA, show ? T * Math.max(0.02, state.form) : 0, (1 - state.form) * text.glyphH * 0.12 + sstep(0.15, 1, m) * T * 1.6, text.scale);
  gl.uniform1f(U.uMelt, m);
  // Puddle under the melting letters, carried up with the page once it has formed.
  const lift = Math.max(0, scrollY - vh * 0.85) / vh;
  const pr = text.ready ? sstep(0.1, 0.8, m) * text.hw * 0.7 * (1 - sstep(0.15, 0.75, lift)) : 0;
  gl.uniform4f(U.uPool, text.cx, text.cy - text.hh * 0.62 + lift, pr, 0.006 + 0.006 * sstep(0.1, 0.8, m));
  rimUniform(U.uRimA, U.uRimAP, a);
  rimUniform(U.uRimB, U.uRimBP, b);
  if (pulled >= 0 && text.homes[pulled]) {
    const h = text.homes[pulled], dp = drops[pulled];
    gl.uniform4f(U.uNeck, h.x / vh, (vh - h.y) / vh, dp.out.x / vh, (vh - dp.out.y) / vh);
  }
  gl.uniform1f(U.uNeckR, neckR);
  gl.uniform2f(U.uLight, light.x, light.y);
  gl.uniform1f(U.uK, lerp(lerp(0.045, 0.016, state.wm), 0.024, state.chain));
  gl.drawArrays(gl.TRIANGLES, 0, 3);
  stats.q = q; stats.w = canvas.width; stats.h = canvas.height; stats.ms = frameAvg;
}

// ---------------------------------------------------------------------------
// Start: a static frame right away, then the core, the type and the motion.
// ---------------------------------------------------------------------------
measure();
for (const s of stops) place(s, scrollY);
for (const d of drops) d.rad = d.r * 0.8;
if (glOk) render(stops[0], stops[1], 0);

if (reduced) engine.reducedMotion = 'always';
await ready;
const fontReady = Promise.race([document.fonts.load('900 100px "Inter Display"'), new Promise((r) => setTimeout(r, 900))]);
await fontReady;
if (glOk) buildText();
measure();
makeWorld();
if (fine && !reduced) smoothScroll({ smooth: 0.55 });
animate(state, { form: [0, 1] }, { duration: reduced ? 0 : 2.6, ease: 'inOutCubic', delay: 0.1 });
if (glOk) animate('.wordmark', { opacity: 0 }, { duration: 1.4, ease: 'inOutSine', delay: 0.5 });
engine.onFrame((_t, dt) => frame(dt));
let rz = 0;
addEventListener('resize', () => { cancelAnimationFrame(rz); rz = requestAnimationFrame(measure); });
addEventListener('load', measure);
document.fonts.ready.then(measure);
startDemos({ reduced, fine, small, setChainPreset, inView });
