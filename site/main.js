// Anikit landing page. One chrome knot, rendered by anikit's WebAssembly rasteriser,
// breaks into seven pieces on scroll; each piece docks beside one demo and the
// pieces re-form the knot above the install command. Every pose below is a pure
// function of one smoothed scroll position, so scrubbing is continuous both ways.
import { ready, engine, renderer3d, scene3d, mesh, instances, spring, inView, set, animate, splitText, stagger } from '../packages/anikit/dist/anikit.lean.js';

await ready;
engine.reducedMotion = 'user';
const reduced = matchMedia('(prefers-reduced-motion: reduce)').matches;
const fine = matchMedia('(hover: hover) and (pointer: fine)').matches;
const $ = (s) => document.querySelector(s);

// ---------------------------------------------------------------------------
// Geometry: a (2, 3) torus knot cut into seven tube pieces
// ---------------------------------------------------------------------------
const PIECES = 7, PER = 64, OVER = 3, TUBE = 46;
const knotAt = (t) => {
  const r = 210 + 95 * Math.cos(3 * t);
  return [r * Math.cos(2 * t), r * Math.sin(2 * t), -95 * Math.sin(3 * t)];
};
const pieces = [];
for (let k = 0; k < PIECES; k++) {
  const pts = [];
  // Each piece runs OVER points into its neighbours, so its rounded caps hide inside
  // them while the knot is whole, and it reads as a solid part once it leaves.
  for (let j = -OVER; j <= PER + OVER; j++) pts.push(knotAt(((k + j / PER) / PIECES) * Math.PI * 2));
  const c = pts.reduce((a, p) => [a[0] + p[0] / pts.length, a[1] + p[1] / pts.length, a[2] + p[2] / pts.length], [0, 0, 0]);
  const caps = [0, 0.6, 0.85, 0.96];
  const radii = pts.map((_, j) => caps[Math.min(j, pts.length - 1 - j)] ?? 1);
  const m = mesh.tube(pts.map((p) => [p[0] - c[0], p[1] - c[1], p[2] - c[2]]), { radius: TUBE, sides: 20, radii });
  pieces.push({ mesh: m, c, inst: instances(1), dir: (() => { const l = Math.hypot(...c) || 1; return [c[0] / l, c[1] / l, c[2] / l]; })() });
}
const sections = [...document.querySelectorAll('.demo')];
const accents = sections.map((s) => s.dataset.accent.match(/[0-9a-f]{2}/gi).map((h) => (parseInt(h, 16) / 255) * 1.6));
sections.forEach((s) => s.style.setProperty('--accent', s.dataset.accent));

// ---------------------------------------------------------------------------
// Renderer: a fixed, reduced internal resolution (about 0.45 Mpx), upscaled by CSS.
// It is chosen once (and lowered at most once, while the hero is idle), so the
// buffers are never reallocated mid-scroll.
// ---------------------------------------------------------------------------
const canvas = $('#stage');
const small = innerWidth < 760;
const dpr = Math.min(devicePixelRatio || 1, 3);
const budgetPx = small ? 300_000 : 1_000_000;
const pickScale = () => Math.min(Math.max(small ? 0.75 : 0.8, dpr * (small ? 0.5 : 0.8)), Math.sqrt(budgetPx / (innerWidth * innerHeight)), 1);
let calFactor = 1;
let scale = pickScale();
let vw = innerWidth, vh = innerHeight, W = 0, H = 0;
const cam = scene3d({ fov: 30, camera: { z: 1600 }, near: 20, far: 6000, width: 16, height: 9 });
let r3 = null;
const fit = () => {
  const w = Math.max(120, Math.round(vw * scale)), h = Math.max(120, Math.round(vh * scale));
  if (w === W && h === H && r3) return;
  W = w; H = h;
  canvas.width = W; canvas.height = H;
  if (r3) r3.resize(W, H); else r3 = renderer3d(canvas);
  cam.width = W; cam.height = H;
};
fit();
const wpp = () => (2 * Math.tan((cam.fov * Math.PI) / 360) * cam.camera.z) / vh; // world units per CSS px at z = 0
const toWorld = (px, py) => [(px - vw / 2) * wpp(), -(py - vh / 2) * wpp()];

// Section offsets, cached (no layout reads in the frame loop).
let last = -1; // reduced motion: the scroll position last drawn
let tops = [], installTop = 0, installBottom = 0, breakTop = 0, breakH = 1;
const measure = () => {
  vw = innerWidth; vh = innerHeight;
  const y = scrollY;
  tops = sections.map((s) => { const b = s.getBoundingClientRect(); return { top: b.top + y, h: b.height, side: s.dataset.side === 'right' ? -1 : 1 }; });
  last = -1; // reduced motion: a resize clears the canvas, so draw again
  const ib = $("#install").getBoundingClientRect(); installTop = ib.top + y; installBottom = ib.bottom + y;
  const bb = $('#break').getBoundingClientRect(); breakTop = bb.top + y; breakH = bb.height;
  fit();
};
measure();
addEventListener('resize', () => { scale = pickScale() * calFactor; measure(); });
new ResizeObserver(() => measure()).observe(document.body);

// ---------------------------------------------------------------------------
// Poses
// ---------------------------------------------------------------------------
const clamp = (v) => (v < 0 ? 0 : v > 1 ? 1 : v);
const ease = (t) => (t < 0.5 ? 4 * t * t * t : 1 - (-2 * t + 2) ** 3 / 2);
const mix = (a, b, t) => a + (b - a) * t;
const tilt = { x: 0, y: 0 };
if (fine && !reduced) addEventListener('pointermove', (e) => spring(tilt, { x: (e.clientY / vh - 0.5) * 22, y: (e.clientX / vw - 0.5) * 30 }, { stiffness: 90, damping: 14 }), { passive: true });

const rotM = instances(1);
/** Rotate a point by the renderer's own Euler convention (read back from an instance matrix). */
const rotate = (rx, ry, rz, p) => {
  rotM.set(0, { rx, ry, rz });
  const m = rotM.data;
  return [m[0] * p[0] + m[3] * p[1] + m[6] * p[2], m[1] * p[0] + m[4] * p[1] + m[7] * p[2], m[2] * p[0] + m[5] * p[1] + m[8] * p[2]];
};

let sy = scrollY, syRaw = scrollY, t = 0;
const intro = { v: reduced || scrollY > 10 ? 0 : 1 };
const pose = (k, out) => {
  const knotScale = small ? 0.62 : 0.86;
  // A: assembled in the hero (screen-fixed), slowly turning, tilted by the pointer.
  const hx = small ? vw / 2 : vw * 0.66, hy = small ? vh * 0.3 : vh * 0.46;
  const [ax, ay] = toWorld(hx, hy);
  const grx = 58 + tilt.x, gry = t * 14 + tilt.y, grz = t * 6;
  // The burst: scrubbed by scroll, plus the intro, where the pieces fly in and lock together.
  const b = Math.max(clamp((sy - breakTop * 0.15) / (breakTop + breakH * 0.35 - breakTop * 0.15)), intro.v * (0.6 + (k % 3) * 0.2));
  const d = clamp((sy - (breakTop + breakH * 0.25)) / (breakH * 0.75)); // flight to the docks
  const rc = rotate(grx, gry, grz, pieces[k].c);
  const burst = ease(b) * (small ? 260 : 420);
  const P = pieces[k];
  let x = ax + rc[0] * knotScale + rotate(grx, gry, grz, P.dir)[0] * burst;
  let y = ay + rc[1] * knotScale + rotate(grx, gry, grz, P.dir)[1] * burst;
  let z = rc[2] * knotScale + burst * 0.4;
  let rx = grx + ease(b) * (k % 2 ? 40 : -40), ry = gry + ease(b) * 30 * (k - 3), rz = grz;
  let s = knotScale;
  // D: docked beside section k, moving with it.
  const T = tops[k];
  const dock = (out2) => {
    const px = small ? vw / 2 : vw / 2 + T.side * vw * 0.235;
    const py = small ? T.top + vh * 0.21 - syRaw : T.top + T.h / 2 - syRaw; // raw scroll: stays attached to its section
    const [wx, wy] = toWorld(px, py);
    out2.x = wx; out2.y = wy; out2.z = 0;
    out2.rx = 25 + tilt.x * 0.6 + Math.sin(t * 0.7 + k) * 12; out2.ry = t * 24 + k * 50 + tilt.y * 0.6; out2.rz = k * 30;
    out2.s = small ? 0.7 : 1.35;
  };
  const D = {};
  dock(D);
  const de = ease(d);
  x = mix(x, D.x, de); y = mix(y, D.y, de); z = mix(z, D.z, de);
  rx = mix(rx, D.rx, de); ry = mix(ry, D.ry, de); rz = mix(rz, D.rz, de); s = mix(s, D.s, de);
  // F: re-formed above the install command.
  const f = ease(clamp((sy - (installTop - vh * 1.05)) / (vh * 0.95)));
  if (f > 0) {
    const [fx, fy] = toWorld(vw / 2, Math.max(vh * (small ? 0.25 : 0.27), installBottom - syRaw - vh * (small ? 0.75 : 0.73)));
    const fs = small ? 0.46 : 0.52, frx = 58 + tilt.x, fry = t * 14 + tilt.y, frz = t * 6;
    const fc = rotate(frx, fry, frz, P.c);
    x = mix(x, fx + fc[0] * fs, f); y = mix(y, fy + fc[1] * fs, f); z = mix(z, fc[2] * fs, f);
    rx = mix(rx, frx, f); ry = mix(ry, fry, f); rz = mix(rz, frz, f); s = mix(s, fs, f);
  }
  out.set(0, { x, y, z, rx, ry, rz, scale: s });
  return { b, d, f };
};

// ---------------------------------------------------------------------------
// Page choreography, driven by the same smoothed scroll value as the knot
// ---------------------------------------------------------------------------
const heroEls = [...document.querySelectorAll('.hero-text, .hero-meta, .scroll-cue')];
const titleChars = splitText('#break-title', { type: 'words,chars', charClass: 'ch', wordClass: 'gw' }).chars;
const note = $('#break-note');
const panels = sections.map((s) => s.querySelector('.panel'));
const lastP = panels.map(() => -1);
let lastHero = -1, lastTitle = -1;
set(titleChars, { opacity: 0 });
set(note, { opacity: 0 });
if (!reduced) animate('.scroll-cue i', { scaleX: [0, 1, 1], x: [0, 0, 46], opacity: [1, 1, 0] }, { duration: 1.8, ease: 'inOutCubic', repeat: Infinity });
const rail = $('#rail');
const railLinks = [...rail.querySelectorAll('a')];
railLinks.forEach((a, k) => a.style.setProperty('--c', sections[k].dataset.accent));
let railActive = -2;
const choreograph = () => {
  // The rail shows while the demos are on screen and marks the one in view.
  let act = -1;
  for (let k = 0; k < tops.length; k++) if (sy + vh * 0.5 >= tops[k].top) act = k;
  if (sy + vh * 0.5 >= installTop) act = -1;
  if (act !== railActive) {
    rail.classList.toggle('on', act >= 0);
    railLinks.forEach((a, k) => a.classList.toggle('on', k === act));
    railActive = act;
  }
  // Hero copy lifts away as the knot starts to break.
  const h = Math.round(clamp(sy / (vh * 0.6)) * 1000) / 1000;
  if (h !== lastHero) { lastHero = h; set(heroEls, { opacity: 1 - h, y: -h * 70 }); }
  // The pinned title assembles character by character, then dissolves as the pieces leave.
  const a = (sy - (breakTop - vh * 0.55)) / (vh * 0.75), z = (sy - (breakTop + breakH - vh * 1.25)) / (vh * 0.4);
  const tk = Math.round(clamp(a) * 1000) / 1000 + Math.round(clamp(z) * 1000) / 1000 * 2;
  if (tk !== lastTitle) {
    lastTitle = tk;
    const n = titleChars.length;
    for (let i = 0; i < n; i++) {
      const pin = ease(clamp(a * 1.7 - (i / n) * 0.7)), pout = ease(clamp(z * 1.7 - ((n - 1 - i) / n) * 0.7));
      set(titleChars[i], { opacity: pin * (1 - pout), y: (1 - pin) * 60 - pout * 60, rotateX: (1 - pin) * -70 });
    }
    set(note, { opacity: clamp(a * 2 - 1) * (1 - clamp(z * 2)), y: (1 - clamp(a * 2 - 1)) * 16 });
  }
  // Each demo panel rises into place as its section enters (a continuous scrub, no triggers).
  for (let k = 0; k < panels.length; k++) {
    const T = tops[k];
    const r = Math.round(ease(clamp((sy + vh - T.top - (small ? vh * 0.3 : 0)) / (vh * 0.6))) * 1000) / 1000;
    if (r === lastP[k]) continue;
    lastP[k] = r;
    set(panels[k], { opacity: 0.001 + r * 0.999, y: (1 - r) * 110 });
  }
};

// ---------------------------------------------------------------------------
// Frame loop
// ---------------------------------------------------------------------------
const items = pieces.map((p, k) => ({
  mesh: p.mesh, instances: p.inst, cull: false,
  material: { metal: 0.92, iridescence: 0.45, hue: k / PIECES, specular: 1.6, shininess: 140, rim: 0.5, lines: 0, lineColor: accents[k] || [1.6, 1.6, 1.6], grid: [0, 4], lineWidth: 0.018 },
}));
const env = { transparent: true, background: '#06070b', sky: [0.05, 0.06, 0.12], horizon: [0.5, 0.42, 0.7], ground: [0.02, 0.02, 0.03], strips: [2.6, 2.6, 2.9], key: [-0.4, 0.6, 0.7], keyColor: [1, 0.95, 0.9], fill: [0.6, -0.2, 0.5], fillColor: [0.3, 0.4, 0.9], rimColor: [0.9, 0.5, 1] };
let avg = 6, frames = 0, minMs = Infinity, stats = { ms: 0, w: 0, h: 0 };
window.__stage = stats;
const frame = (dt) => {
  if (!reduced) t += Math.min(dt, 50) / 1000;
  syRaw = scrollY;
  sy += (syRaw - sy) * (reduced ? 1 : 1 - Math.exp(-dt / 70));
  if (Math.abs(scrollY - sy) < 0.5) sy = scrollY;
  if (reduced) {
    if (sy === last) return; // reduced motion: draw only when the scroll changes
    last = sy;
  }
  let glowing = 0;
  for (let k = 0; k < PIECES; k++) {
    const { d, f } = pose(k, pieces[k].inst);
    const m = items[k].material;
    // Docked pieces light up in their section's accent, with pulses running along them.
    glowing = Math.max(0, d - f);
    m.lines = 1.1 * glowing;
  }
  choreograph();
  const t0 = performance.now();
  r3.render(cam, items, env);
  const ms = performance.now() - t0;
  avg = avg * 0.9 + ms * 0.1;
  stats.ms = avg; stats.w = W; stats.h = H; stats.last = ms;
  // Calibration, twice at most (after 60 and 150 frames, while the hero is on screen):
  // if even the fastest frame of the window took over 7 ms (the minimum ignores frames
  // slowed by other work), lower the resolution. Never during the demos.
  minMs = Math.min(minMs, ms);
  if ((++frames === 60 || frames === 150) && minMs > 7 && sy < vh * 0.5 && scale > 0.25) {
    const f = Math.max(0.5, Math.sqrt(6 / minMs));
    calFactor *= f; scale *= f; fit();
  }
  if (frames === 60) minMs = Infinity;
  if (!canvas.classList.contains('on')) canvas.classList.add('on');
};
engine.onFrame((_t, dt) => frame(dt));
if (intro.v) animate(intro, { v: 0 }, { duration: 1.6, ease: 'spring(bounce: 0.18, duration: 1.4)', delay: 0.1 });
if (location.search.includes('debug')) {
  // Profiling hook: render the pose at scroll position y, n times, and return the times.
  window.__at = (y, n = 5) => { const out = []; for (let i = 0; i < n; i++) { sy = syRaw = y; for (let k = 0; k < PIECES; k++) pose(k, pieces[k].inst); const a = performance.now(); r3.render(cam, items, env); out.push(+(performance.now() - a).toFixed(1)); } return out; };
}

// ---------------------------------------------------------------------------
// Demos load on approach (dynamic import keeps them off the critical path)
// ---------------------------------------------------------------------------
let demos = null;
const loadDemos = () => (demos ??= import('./demos.js').then((m) => m.start({ reduced, fine, small })));
inView('#break', () => { loadDemos(); }, { amount: 0 });
if (scrollY > innerHeight) loadDemos();
