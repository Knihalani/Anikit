import { init, engine, animate, timeline, stagger, splitText, scrollTrigger, world, flip, mesh, htmlCanvas, scene } from '../../packages/anikit/dist/anikit.lean.js';
import { createGL } from './gl.js';
import { paint } from './paint.js';

const $ = (s) => document.querySelector(s);
const PERF = new URLSearchParams(location.search).has('perf');
const reduce = matchMedia('(prefers-reduced-motion: reduce)').matches;
const clamp = (v, a, b) => (v < a ? a : v > b ? b : v);
const smooth = (a, b, v) => { const t = clamp((v - a) / (b - a), 0, 1); return t * t * (3 - 2 * t); };

// Copy buttons work before anything else loads.
for (const b of document.querySelectorAll('[data-copy]')) b.addEventListener('click', () => {
  navigator.clipboard?.writeText(b.dataset.copy).catch(() => {});
  const c = b.querySelector('code'); const was = c.textContent;
  c.textContent = 'copied'; setTimeout(() => (c.textContent = was), 1200);
});

const idle = () => new Promise((r) => (window.requestIdleCallback ? requestIdleCallback(() => r(), { timeout: 120 }) : setTimeout(r, 16)));

const T = (window.__t = []), mark = (n) => T.push([n, Math.round(performance.now())]);
mark('module');
await init();
mark('wasm');
await document.fonts.ready;
mark('fonts');
// Let the HTML paint before any WebGL work starts.
await new Promise((r) => requestAnimationFrame(() => setTimeout(r, 0)));

// ─── WebGL overlay ─────────────────────────────────────────────────────────
const canvas = $('#gl');
// anikit scene() owns the one WebGL2 context and the frame; the letters, the
// ripple and the shatter draw into it as scene layers.
mark('pre-scene');
let S = null;
try {
  S = scene(canvas, { background: 'transparent', shadows: false, bloom: false, fxaa: false, adaptive: false, autoRender: false,
    environment: 'studio', camera: { fov: 44, near: 20, far: 10000, position: [0, 0, 1000], target: [0, 0, 0] } });
} catch { S = null; }
mark('scene');
const G = createGL(canvas, S ? canvas.getContext('webgl2') : null);
mark('gl');
let vw = innerWidth, vh = innerHeight;
let quality = 0; // 0 best … 4: governor steps down effects before resolution
const dprFor = () => Math.min(devicePixelRatio || 1, 2) * (quality >= 4 ? 0.6 : quality >= 3 ? 0.8 : 1);
G?.resize(vw, vh, dprFor());
function setDpr() { G?.resize(vw, vh, dprFor()); if (S) S.pixelRatio = dprFor(); }

// ─── Perf: frame times and the quality governor ────────────────────────────
const N = 120, times = new Float64Array(N), sorted = new Float64Array(N);
let ti = 0, filled = 0, lastT = 0, govClock = 0;
const perfEl = $('#perf');
if (PERF) perfEl.hidden = false;
function stat() {
  const n = filled;
  sorted.set(times);
  const v = sorted.subarray(0, n).sort();
  return [v[(n * 0.5) | 0], v[Math.min(n - 1, (n * 0.95) | 0)], v[n - 1]];
}
let perfText = 0;
function perf(now) {
  if (lastT) { times[ti] = now - lastT; ti = (ti + 1) % N; if (filled < N) filled++; }
  lastT = now;
  if (filled < 60 || now - govClock < 1000) return;
  govClock = now;
  const [p50, p95, max] = stat();
  if (p95 > 22 && quality < 4 && document.visibilityState === 'visible') { quality++; setDpr(); filled = 0; }
  else if (p95 < 17.5 && quality > 0 && now - perfText > 6000) { quality--; setDpr(); perfText = now; filled = 0; }
  if (PERF) perfEl.textContent = `p50 ${p50.toFixed(1)} ms\np95 ${p95.toFixed(1)} ms\nmax ${max.toFixed(1)} ms\nquality ${quality}  dpr ${G ? G.dpr.toFixed(2) : '-'}`;
}

// ─── 1. Hero: the headline extrudes into letters you can spin ──────────────
const word = $('#word');
const split = splitText(word, { type: 'chars' });
const chars = split.chars;
const letters = chars.map((el) => ({ el, mesh: null, cx: 0, cy: 0, sx: 0, sy: 0, fs: 0, e: 0, rx: 0, ry: 0, w: 0, live: false }));

function measureLetters() {
  const fs = parseFloat(getComputedStyle(word).fontSize);
  const font = `900 ${fs}px InterDisplay`;
  const probe = document.createElement('canvas').getContext('2d');
  probe.font = font;
  const marker = document.createElement('span');
  marker.style.cssText = 'display:inline-block;width:0;height:0;vertical-align:baseline';
  word.append(marker);
  const base = marker.getBoundingClientRect().bottom + scrollY;
  marker.remove();
  const pad = 8;
  for (const L of letters) {
    const r = L.el.getBoundingClientRect();
    const m = probe.measureText(L.el.textContent);
    const tw = Math.ceil(m.width) + pad * 2;
    const asc = m.actualBoundingBoxAscent, desc = m.actualBoundingBoxDescent;
    const th = Math.ceil(asc + desc) + pad * 2;
    L.cx = r.left + scrollX - pad + tw / 2;
    L.cy = base - asc - pad + th / 2;
    L.sx = tw; L.sy = tw * (L.mesh ? L.mesh.aspect : th / tw); L.fs = fs;
  }
  return font;
}

async function buildLetters() {
  const font = measureLetters();
  for (const L of letters) {
    await idle();
    const t0 = performance.now();
    const m3 = mesh.text(L.el.textContent, { font, resolution: 150, depth: 0.1, bevel: 0.35 });
    L.aspect = m3.aspect;
    L.sy = L.sx * L.aspect;
    const t1 = performance.now();
    const arr = m3.arrays();
    m3.free();
    L.mesh = G.mesh(arr);
    mark('letter ' + Math.round(t1 - t0) + '/' + Math.round(performance.now() - t1));
  }
}

// Letter motion: spin velocity from the pointer, a spring home to a whole turn.
let clock = 0, lastHeroInput = -1e9, scrollE = 0;
const intro = { on: 0, rest: 0 };
const look = { x: 0, y: 0 };
addEventListener('pointermove', (e) => { look.x = e.clientX / vw - 0.5; look.y = e.clientY / vh - 0.5; }, { passive: true });
const ptr = { x: -1, y: -1, down: false, px: 0, vx: 0 };
word.addEventListener('pointerdown', (e) => { ptr.down = true; ptr.px = e.clientX; word.setPointerCapture(e.pointerId); lastHeroInput = clock; });
word.addEventListener('pointermove', (e) => {
  const dx = e.clientX - (ptr.px || e.clientX); ptr.px = e.clientX;
  ptr.x = e.clientX; ptr.y = e.clientY + scrollY;
  lastHeroInput = clock;
  for (const L of letters) {
    if (Math.abs(ptr.x + scrollX - L.cx) < L.sx * 0.5) L.w += dx * (ptr.down ? 0.0016 : 0.0006) * 60;
  }
});
addEventListener('pointerup', () => (ptr.down = false));

function stepLetters(dt) {
  const target = Math.max(intro.on, intro.rest, clock - lastHeroInput < 2.6 ? 1 : 0, scrollE);
  let any = false;
  for (let i = 0; i < letters.length; i++) {
    const L = letters[i];
    // Extrusion: critically damped toward the target.
    L.e += (target - L.e) * (1 - Math.exp(-dt * 6));
    if (Math.abs(target - L.e) < 1e-4) L.e = target;
    // Spin: free while fast, then a spring to the nearest whole turn.
    const lookY = reduce ? 0 : look.x * 0.5 * intro.rest;
    const home = Math.round((L.ry - lookY) / (Math.PI * 2)) * Math.PI * 2 + lookY;
    const k = Math.abs(L.w) > 2.2 ? 0 : 38;
    L.w += (-(L.ry - home) * k - L.w * (k ? 9 : 0.9)) * dt;
    L.ry += L.w * dt;
    if (!intro.rest && Math.abs(L.w) < 1e-3 && Math.abs(L.ry - home) < 1e-4) { L.ry = home % (Math.PI * 2); L.w = 0; }
    const tilt = scrollE * -(0.6 + 0.08 * i) + (reduce ? 0 : look.y * 0.35 * intro.rest); // tip back as the page leaves, a little more per letter
    L.rx += (tilt - L.rx) * (1 - Math.exp(-dt * 10));
    const live = L.e > 1e-3 || Math.abs(L.ry % (Math.PI * 2)) > 1e-3 || Math.abs(L.rx) > 1e-3;
    if (live !== L.live) { L.live = live; L.el.style.color = live && L.mesh ? 'transparent' : ''; }
    if (live) any = true;
  }
  return any;
}

// ─── 2. The sheet folds along three creases ────────────────────────────────
const sheet = $('#sheet'), fold = $('#fold');
const STRIPS = 4;
let strips = [], stripsWrap = null, stripH = 0;
function buildStrips() {
  stripsWrap?.remove();
  const r = sheet.getBoundingClientRect(), f = fold.getBoundingClientRect();
  stripH = r.height / STRIPS;
  stripsWrap = document.createElement('div');
  stripsWrap.className = 'strips';
  stripsWrap.setAttribute('aria-hidden', 'true');
  stripsWrap.style.cssText = `left:${r.left - f.left}px;top:${r.top - f.top}px;width:${r.width}px;height:${r.height}px;transform-origin:50% 50%`;
  strips = [];
  for (let k = 0; k < STRIPS; k++) {
    const s = document.createElement('div');
    s.className = 'strip';
    s.style.cssText = `top:${k * stripH}px;width:${r.width}px;height:${stripH + 0.5}px;transform-origin:50% 0`;
    const c = sheet.cloneNode(true);
    c.removeAttribute('id');
    c.style.cssText = `top:${-k * stripH}px;width:${r.width}px;height:${r.height}px;border-radius:${k === 0 ? '6px 6px 0 0' : k === STRIPS - 1 ? '0 0 6px 6px' : '0'}`;
    const shade = document.createElement('div');
    shade.className = 'shade';
    // Light on the upward faces, a crease shadow in each valley.
    shade.style.background = k % 2 ? 'linear-gradient(rgba(70,45,10,.32), rgba(255,255,255,.35) 30%, rgba(255,255,255,.55))' : 'linear-gradient(rgba(255,255,255,.2), rgba(70,45,10,.12) 55%, rgba(70,45,10,.42))';
    s.append(c, shade);
    stripsWrap.append(s);
    strips.push({ s, shade });
  }
  fold.append(stripsWrap);
  stripsWrap.style.visibility = 'hidden';
}
const foldState = { p: 0 };
let foldDrawn = -1;
function drawFold() {
  const p = reduce ? 0 : foldState.p;
  if (Math.abs(p - foldDrawn) < 1e-5) return;
  foldDrawn = p;
  const a = smooth(0.03, 0.55, p) * 1.42; // radians, ~81°
  const on = a > 1e-4;
  fold.classList.toggle('folding', on);
  stripsWrap.style.visibility = on ? 'visible' : 'hidden';
  if (!on) return;
  const lift = smooth(0.5, 1, p);
  const total = STRIPS * stripH;
  const c = Math.cos(a), s = Math.sin(a);
  for (let k = 0; k < STRIPS; k++) {
    const { s: el, shade } = strips[k];
    const z = k % 2 ? stripH * s : 0;
    el.style.transform = `translate3d(0,${(k * stripH * (c - 1)).toFixed(2)}px,${z.toFixed(2)}px) rotateX(${((k % 2 ? -a : a) * 57.2958).toFixed(3)}deg)`;
    shade.style.opacity = s.toFixed(3);
  }
  const packed = total * c;
  stripsWrap.style.transform = `translate3d(0,${((total - packed) / 2 - lift * vh * 0.5).toFixed(2)}px,${(-lift * 300).toFixed(1)}px) rotateX(${(s * 30 + lift * 20).toFixed(2)}deg) rotateY(${(-s * 16).toFixed(2)}deg) rotateZ(${(-lift * 7).toFixed(2)}deg)`;
}

// ─── 3. Cards are rigid bodies (anikit world) ─────────────────────────────
const stage = $('#cards'), grid = $('#grid');
const cardEls = [...grid.querySelectorAll('.card')];
const cards = cardEls.map((el) => ({ el, x: 0, y: 0, r: 0, base: 0, ox: 0, oy: 0, written: '' }));
let W = null, physics = false;
const dropBtn = $('#drop'), tidyBtn = $('#tidy');

function drop() {
  if (physics) return;
  for (const c of cards) animate(c, { x: c.x, y: c.y, r: c.r }, { duration: 0.01 }); // stop any tidy-up in flight
  W?.destroy();
  W = world({ container: stage, gravity: [0, 2000], restitution: 0.25, friction: 0.35, substeps: 6, iterations: 3, damping: 0.02 });
  const S = stage.getBoundingClientRect();
  for (const c of cards) {
    const r = c.el.getBoundingClientRect();
    const w = c.el.offsetWidth, h = c.el.offsetHeight;
    // Untransformed centre, in stage coordinates.
    const cx = r.left + r.width / 2 - S.left, cy = r.top + r.height / 2 - S.top;
    c.ox = cx - c.x; c.oy = cy - c.y;
    const rad = Math.min(w / 6, h / 4);
    const ca = Math.cos(c.r), sa = Math.sin(c.r);
    const pts = [];
    for (let j = 0; j < 2; j++) for (let i = 0; i < 3; i++) {
      const lx = (i - 1) * (w / 3), ly = (j - 0.5) * (h / 2);
      pts.push({ x: cx + lx * ca - ly * sa, y: cy + lx * sa + ly * ca, r: rad, mass: 1 });
    }
    c.base = W.addMany(pts);
    for (let a = 0; a < 6; a++) for (let b = a + 1; b < 6; b++) W.link(c.base + a, c.base + b, { stiffness: 1 });
    // Throw each card towards its own spot across the full width.
    const tx = S.width * (0.08 + 0.84 * (cards.indexOf(c) + Math.random() * 0.6) / cards.length);
    const kick = (tx - cx) * 1.5 + (Math.random() - 0.5) * 200;
    const spin = (Math.random() - 0.5) * 500;
    for (let a = 0; a < 6; a++) W.setVelocity(c.base + a, kick, -700 - Math.random() * 300 + ((a % 3) - 1) * spin);
  }
  W.dragging({ hitSlop: 24 });
  physics = true;
  stage.classList.add('flying');
  dropBtn.disabled = true; tidyBtn.disabled = false;
}
function tidy() {
  if (!physics) return;
  physics = false;
  W.pause();
  stage.classList.remove('flying');
  for (const c of cards) {
    // unwrap so the card turns the short way home
    c.r = Math.atan2(Math.sin(c.r), Math.cos(c.r));
  }
  animate(cards, { x: 0, y: 0, r: 0 }, { ease: 'spring(bounce: 0.22, duration: 0.9)', delay: stagger(0.05) });
  dropBtn.disabled = false; tidyBtn.disabled = true;
}
dropBtn.addEventListener('click', drop);
tidyBtn.addEventListener('click', tidy);
cardEls.forEach((el) => el.addEventListener('pointerdown', (e) => { if (!physics && !e.target.closest('button')) drop(); }));

function readCards() {
  if (!physics) return;
  const d = W.data;
  for (const c of cards) {
    const b = c.base * 3;
    let sx = 0, sy = 0;
    for (let a = 0; a < 6; a++) { sx += d[b + a * 3]; sy += d[b + a * 3 + 1]; }
    c.x = sx / 6 - c.ox; c.y = sy / 6 - c.oy;
    c.r = Math.atan2(d[b + 7] - d[b + 1], d[b + 6] - d[b]);
  }
}
function writeCards() {
  for (const c of cards) {
    const t = `translate3d(${c.x.toFixed(1)}px,${c.y.toFixed(1)}px,0) rotate(${(c.r * 57.2958).toFixed(2)}deg)`;
    if (t !== c.written) { c.written = t; c.el.style.transform = c.x || c.y || c.r ? t : ''; }
  }
}

// The small "Try" buttons each show off one function.
const tries = cardEls.map((el) => el.querySelector('.mini'));
tries[0].addEventListener('click', () => animate(cardEls[0].querySelector('code'), { x: [0, 18, -10, 0], color: ['#1c1a17', '#ff5b3a', '#1c1a17'] }, { duration: 0.7 }));
tries[1].addEventListener('click', () => animate(cardEls[1].querySelector('code'), { scale: [1, 1.5, 1] }, { ease: 'spring(bounce: 0.5, duration: 0.8)' }));
tries[2].addEventListener('click', () => (physics ? W.impulse(stage.clientWidth / 2, stage.clientHeight, 900, 1600) : drop()));
tries[3].addEventListener('click', () => timeline({ defaults: { duration: 0.3, ease: 'outCubic' } })
  .add(cardEls[3].querySelector('code'), { y: [0, -8] }).add(cardEls[3].querySelector('p'), { opacity: [1, 0.3] }, '-=0.15')
  .add(cardEls[3].querySelector('code'), { y: 0 }).add(cardEls[3].querySelector('p'), { opacity: 1 }, '-=0.15'));
tries[4].addEventListener('click', () => { if (!physics) flip(cardEls, () => grid.append(...cardEls.slice(1).concat(cardEls[0])), { duration: 0.7, ease: 'spring(bounce: 0.2, duration: 0.7)' }); });
const sixP = cardEls[5].querySelector('p');
tries[5].addEventListener('click', () => {
  const s = splitText(sixP, { type: 'chars' });
  animate(s.chars, { y: [0, -10, 0], color: ['#1c1a17', '#3157ff', '#1c1a17'] }, { duration: 0.6, delay: stagger(0.015), onComplete: () => s.revert() });
});

// ─── 4. The source ripples like water ──────────────────────────────────────
const src = $('#src');
const MAXR = 12, rips = new Float32Array(MAXR * 4);
let ripHead = 0, ripLive = false, codePainted = false, lastRip = 0, lastRx = -1e9, lastRy = -1e9;
function addRipple(x, y, amp) {
  const k = ripHead++ % MAXR;
  rips[k * 4] = x; rips[k * 4 + 1] = y; rips[k * 4 + 2] = 0; rips[k * 4 + 3] = amp;
}
src.addEventListener('pointermove', (e) => {
  if (!codePainted) return;
  const r = src.getBoundingClientRect();
  const x = e.clientX - r.left, y = e.clientY - r.top;
  const now = clock;
  if (now - lastRip > 0.07 && Math.hypot(x - lastRx, y - lastRy) > 18) { addRipple(x, y, 1.1); lastRip = now; lastRx = x; lastRy = y; }
});
src.addEventListener('pointerdown', (e) => { if (!codePainted) return; const r = src.getBoundingClientRect(); addRipple(e.clientX - r.left, e.clientY - r.top, 2.2); });
function stepRipples(dt) {
  let live = false;
  for (let k = 0; k < MAXR; k++) {
    if (rips[k * 4 + 3] <= 0) continue;
    rips[k * 4 + 2] += dt;
    if (rips[k * 4 + 2] > 3.2) rips[k * 4 + 3] = 0; else live = true;
  }
  if (live !== ripLive) { ripLive = live; src.classList.toggle('live', live); }
  return live;
}
// htmlCanvas() rasterises the real element into a texture in our WebGL context;
// the small painter in paint.js is the stand-in if that ever fails.
const html = {};
async function capture(which, el) {
  const scale = Math.min(devicePixelRatio || 1, 2);
  try {
    if (!html[which]) html[which] = htmlCanvas(el, { target: G.gl, live: false, scale });
    else await html[which].refresh();
    await html[which].ready;
    G.use(which, html[which].texture);
  } catch {
    G.upload(which, paint(el, scale));
  }
}
async function paintCode() {
  await capture('code', src);
  codePainted = true;
}

// ─── 5. The page shatters and comes back together ─────────────────────────
const panel = $('#panel');
const fin = { p: 0 }, burst = { e: 0 };
let shards = null, shardsLow = null, panelPainted = false, shatterOn = false, origin = [0.5, 0.6];
async function paintPanel() {
  await capture('panel', panel);
  panelPainted = true;
}
$('#breakit').addEventListener('click', (e) => {
  const r = panel.getBoundingClientRect();
  origin = [e.clientX - r.left, e.clientY - r.top];
  timeline().add(burst, { e: [0, 0.62] }, { duration: 0.9, ease: 'outCubic' })
    .add(burst, { e: 0 }, { duration: 1.5, ease: 'inOutCubic' }, '+=0.15');
});

// ─── Scroll: anikit scroll triggers drive the fold and the finale ─────────
scrollTrigger({ trigger: '#fold-wrap', start: 'top top', end: 'bottom bottom', scrub: 0.5,
  animation: animate(foldState, { p: [0, 1] }, { ease: 'linear', duration: 1, autoplay: false }) });
if (!reduce) scrollTrigger({ trigger: '#cards', start: 'top 35%', once: true, onEnter: () => drop() });
scrollTrigger({ trigger: '#finale', start: 'top top', end: 'bottom bottom', scrub: 0.6,
  animation: animate(fin, { p: [0, 1] }, { ease: 'linear', duration: 1, autoplay: false }) });

// ─── Frame loop ────────────────────────────────────────────────────────────
let sy = scrollY;
addEventListener('scroll', () => (sy = scrollY), { passive: true });
let heroRect = { top: 0, bottom: 0 }, codeTop = 0, codeLeft = 0, codeW = 0, codeH = 0;
function layout() {
  vw = innerWidth; vh = innerHeight;
  setDpr();
  if (S) {
    const D = (vh / 2) / Math.tan((22 * Math.PI) / 180);
    S.camera.position.set(vw / 2, -vh / 2, D);
    S.camera.target.set(vw / 2, -vh / 2, 0);
  }
  const h = $('#hero').getBoundingClientRect();
  heroRect = { top: h.top + scrollY, bottom: h.bottom + scrollY };
  const c = src.getBoundingClientRect();
  codeTop = c.top + scrollY; codeLeft = c.left + scrollX; codeW = c.width; codeH = c.height;
  if (letters[0].mesh) measureLetters();
  buildStrips(); foldDrawn = -1;
  if (codePainted) paintCode();
  if (panelPainted) paintPanel();
}
let resizeT = 0;
addEventListener('resize', () => { clearTimeout(resizeT); resizeT = setTimeout(layout, 120); });

let ready = false, drewLast = false;
const frameState = { letters: false, rip: false, sh: false, e: 0 };
let panelRect = { left: 0, top: 0, width: 1, height: 1 };
function drawLayers() {
  if (frameState.letters) {
    for (const L of letters) {
      if (!L.live || !L.mesh || L.e < 0.01) continue;
      G.letter(L.mesh, L.cx - scrollX, L.cy - sy, L.sx, L.sy, L.fs * 0.1 * L.e, L.rx, L.ry, L.e, 1, 1);
    }
    for (const L of letters) {
      if (!L.live || !L.mesh) continue;
      const depth = L.fs * 0.1 * L.e;
      const cy = L.cy - sy;
      G.letter(L.mesh, L.cx - scrollX, cy, L.sx, L.sy, depth, L.rx, L.ry, L.e, 1);
      const turned = Math.abs(Math.sin(L.ry)) > 0.02 || Math.cos(L.ry) < 0 || Math.abs(L.rx) > 0.02;
      if (L.e > 1e-3 && turned) G.letter(L.mesh, L.cx - scrollX, cy, L.sx, L.sy, depth, L.rx, L.ry, L.e, -1);
      G.clearDepth();
    }
  }
  if (frameState.rip) G.ripple(codeLeft - scrollX, codeTop - sy, codeW, codeH, rips, quality >= 1 ? 6 : MAXR);
  if (frameState.sh) G.shatter(quality >= 2 ? shardsLow : shards, panelRect.left, panelRect.top, panelRect.width, panelRect.height, frameState.e, origin[0], origin[1]);
}
if (S) S.add({ render() { drawLayers(); } });
engine.onFrame((now, dtMs) => {
  perf(now);
  const dt = Math.min(dtMs, 50) / 1000;
  clock += dt;
  sy = scrollY;
  readCards();
  if (!ready) { writeCards(); drawFold(); return; }
  scrollE = reduce ? 0 : smooth(0, vh * 0.7, sy);
  const heroOn = sy < heroRect.bottom;
  const lettersLive = heroOn ? stepLetters(dt) : false;
  const ripOn = stepRipples(dt) && codeTop - sy < vh && codeTop + codeH - sy > 0;
  const fe = smooth(0.12, 0.42, fin.p) * (1 - smooth(0.62, 0.92, fin.p));
  const e = Math.max(reduce ? 0 : fe * 0.55, burst.e);
  const sh = e > 1e-4 && panelPainted;
  if (sh !== shatterOn) { shatterOn = sh; panel.classList.toggle('live', sh); if (sh && burst.e < 1e-4) origin = [vw * 0.5, vh * 0.55]; }
  if (shatterOn) panelRect = panel.getBoundingClientRect();
  writeCards();
  drawFold();
  if (!G) return;
  frameState.rip = ripOn; frameState.sh = shatterOn; frameState.e = e;
  frameState.letters = lettersLive;
  if (S) {
    const any = lettersLive || ripOn || shatterOn;
    if (any || drewLast) S.render();
    drewLast = any;
    return;
  }
  G.begin();
  drawLayers();
});

// ─── Start: paint first, pre-warm everything, then come alive ──────────────
mark('setup');
layout();
mark('layout');
if (G) {
  shards = G.shards(8, 5);
  shardsLow = G.shards(6, 4);
  await buildLetters();
  mark('letters');
  await idle(); await paintCode();
  mark('code');
  await idle(); await paintPanel();
  mark('panel');
  // Pre-warm every program once, off-screen, so the first real frame doesn't compile.
  if (S) {
    // Pre-warm every program once, off-screen, then clear.
    const keep = [letters[0].cx, codeLeft];
    letters[0].cx = -99999; codeLeft = -99999; letters[0].live = true;
    frameState.letters = frameState.rip = frameState.sh = true; panelRect = { left: -99999, top: -99999, width: 1, height: 1 };
    S.render();
    [letters[0].cx, codeLeft] = keep; letters[0].live = false;
    frameState.letters = frameState.rip = frameState.sh = false;
    S.render();
  } else {
    G.begin();
    G.letter(letters[0].mesh, -9999, -9999, 1, 1, 1, 0, 0, 1, 1);
    G.ripple(-9999, -9999, 1, 1, rips, 1);
    G.shatter(shards, -9999, -9999, 1, 1, 0.5, 0, 0);
    G.begin();
  }
  ready = true;
  if (!reduce) {
    // The introduction: letters rise out of the paper, turn once, and settle back.
    letters.forEach((L, i) => animate(L, { w: 9.5 }, { duration: 0.01, delay: 0.4 + i * 0.09 }));
    animate(intro, { on: [0, 1, 1, 0] }, { duration: 3.4, ease: 'inOutSine', delay: 0.2 });
    // After the introduction the letters stay raised off the page and follow the pointer.
    animate(intro, { rest: [0, 0.85] }, { duration: 1.2, ease: 'inOutSine', delay: 2.6 });
  }
}
window.__proto = { S, addRipple, drop, tidy, letters, cards, foldState, fin, burst, get quality() { return quality; } };
