// Anikit landing page. One chrome knot, rendered by anikit's WebAssembly rasteriser,
// breaks into seven pieces on scroll; each piece docks beside one demo and the
// pieces re-form the knot above the install command. Every pose below is a pure
// function of one smoothed scroll position, so scrubbing is continuous both ways.
import { ready, engine, renderer3d, scene3d, mesh, instances, spring, inView } from '../packages/anikit/dist/anikit.lean.js';

await ready;
engine.reducedMotion = 'user';
const reduced = matchMedia('(prefers-reduced-motion: reduce)').matches;
const fine = matchMedia('(hover: hover) and (pointer: fine)').matches;
const $ = (s) => document.querySelector(s);

// ---------------------------------------------------------------------------
// Geometry: a (2, 3) torus knot cut into seven tube pieces
// ---------------------------------------------------------------------------
const PIECES = 7, PER = 64, TUBE = 46;
const knotAt = (t) => {
  const r = 210 + 95 * Math.cos(3 * t);
  return [r * Math.cos(2 * t), r * Math.sin(2 * t), -95 * Math.sin(3 * t)];
};
const pieces = [];
for (let k = 0; k < PIECES; k++) {
  const pts = [];
  for (let j = 0; j <= PER; j++) pts.push(knotAt(((k + j / PER) / PIECES) * Math.PI * 2));
  const c = pts.reduce((a, p) => [a[0] + p[0] / pts.length, a[1] + p[1] / pts.length, a[2] + p[2] / pts.length], [0, 0, 0]);
  // Rounded-off ends so each piece reads as a solid part once it leaves the knot.
  const radii = pts.map((_, j) => { const e = Math.min(j, PER - j); return e === 0 ? 0.3 : e === 1 ? 0.78 : e === 2 ? 0.95 : 1; });
  const m = mesh.tube(pts.map((p) => [p[0] - c[0], p[1] - c[1], p[2] - c[2]]), { radius: TUBE, sides: 20, radii });
  pieces.push({ mesh: m, c, inst: instances(1), dir: (() => { const l = Math.hypot(...c) || 1; return [c[0] / l, c[1] / l, c[2] / l]; })() });
}
const sections = [...document.querySelectorAll('.demo')];
const accents = sections.map((s) => s.dataset.accent.match(/[0-9a-f]{2}/gi).map((h) => (parseInt(h, 16) / 255) * 1.6));
sections.forEach((s) => s.style.setProperty('--accent', s.dataset.accent));

// ---------------------------------------------------------------------------
// Renderer: reduced internal resolution, adapted to hold ~7 ms per frame
// ---------------------------------------------------------------------------
const canvas = $('#stage');
const small = innerWidth < 760;
let scale = small ? Math.max(0.8, Math.min(devicePixelRatio || 1, 3) * 0.5) : Math.min(devicePixelRatio || 1, 2) * 0.6;
let vw = innerWidth, vh = innerHeight, W = 0, H = 0;
const cam = scene3d({ fov: 30, camera: { z: 1600 }, near: 20, far: 6000, width: 16, height: 9 });
let r3 = null;
const fit = () => {
  W = Math.max(120, Math.round(vw * scale)); H = Math.max(120, Math.round(vh * scale));
  canvas.width = W; canvas.height = H;
  if (r3) r3.resize(W, H); else r3 = renderer3d(canvas);
  cam.width = W; cam.height = H;
};
fit();
const wpp = () => (2 * Math.tan((cam.fov * Math.PI) / 360) * cam.camera.z) / vh; // world units per CSS px at z = 0
const toWorld = (px, py) => [(px - vw / 2) * wpp(), -(py - vh / 2) * wpp()];

// Section offsets, cached (no layout reads in the frame loop).
let tops = [], installTop = 0, installBottom = 0, breakTop = 0, breakH = 1;
const measure = () => {
  vw = innerWidth; vh = innerHeight;
  const y = scrollY;
  tops = sections.map((s) => { const b = s.getBoundingClientRect(); return { top: b.top + y, h: b.height, side: s.dataset.side === 'right' ? -1 : 1 }; });
  const ib = $("#install").getBoundingClientRect(); installTop = ib.top + y; installBottom = ib.bottom + y;
  const bb = $('#break').getBoundingClientRect(); breakTop = bb.top + y; breakH = bb.height;
  fit();
};
measure();
addEventListener('resize', measure);
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

let sy = scrollY, t = 0;
const pose = (k, out) => {
  const knotScale = small ? 0.62 : 0.95;
  // A: assembled in the hero (screen-fixed), slowly turning, tilted by the pointer.
  const hx = small ? vw / 2 : vw * 0.66, hy = small ? vh * 0.3 : vh * 0.46;
  const [ax, ay] = toWorld(hx, hy);
  const grx = 58 + tilt.x, gry = t * 14 + tilt.y, grz = t * 6;
  const b = clamp((sy - breakTop * 0.15) / (breakTop + breakH * 0.35 - breakTop * 0.15)); // the burst
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
    const px = small ? vw / 2 : vw / 2 + T.side * vw * 0.25;
    const py = small ? T.top + vh * 0.22 - sy : T.top + T.h / 2 - sy;
    const [wx, wy] = toWorld(px, py);
    out2.x = wx; out2.y = wy; out2.z = 0;
    out2.rx = 25 + tilt.x * 0.6 + Math.sin(t * 0.7 + k) * 12; out2.ry = t * 24 + k * 50 + tilt.y * 0.6; out2.rz = k * 30;
    out2.s = small ? 1.0 : 1.55;
  };
  const D = {};
  dock(D);
  const de = ease(d);
  x = mix(x, D.x, de); y = mix(y, D.y, de); z = mix(z, D.z, de);
  rx = mix(rx, D.rx, de); ry = mix(ry, D.ry, de); rz = mix(rz, D.rz, de); s = mix(s, D.s, de);
  // F: re-formed above the install command.
  const f = ease(clamp((sy - (installTop - vh * 1.05)) / (vh * 0.95)));
  if (f > 0) {
    const [fx, fy] = toWorld(vw / 2, Math.max(vh * 0.3, installBottom - sy - vh * 0.7));
    const fs = small ? 0.55 : 0.75, frx = 58 + tilt.x, fry = t * 14 + tilt.y, frz = t * 6;
    const fc = rotate(frx, fry, frz, P.c);
    x = mix(x, fx + fc[0] * fs, f); y = mix(y, fy + fc[1] * fs, f); z = mix(z, fc[2] * fs, f);
    rx = mix(rx, frx, f); ry = mix(ry, fry, f); rz = mix(rz, frz, f); s = mix(s, fs, f);
  }
  out.set(0, { x, y, z, rx, ry, rz, scale: s });
  return { b, d, f };
};

// ---------------------------------------------------------------------------
// Frame loop
// ---------------------------------------------------------------------------
const items = pieces.map((p, k) => ({
  mesh: p.mesh, instances: p.inst, cull: false,
  material: { metal: 0.92, iridescence: 0.45, hue: k / PIECES, specular: 1.6, shininess: 140, rim: 0.5, lines: 0, lineColor: accents[k] || [1.6, 1.6, 1.6], grid: [0, 5], lineWidth: 0.04, pulse: 0, pulseCount: 2, pulsePhase: 0 },
}));
const env = { background: '#06070b', sky: [0.05, 0.06, 0.12], horizon: [0.5, 0.42, 0.7], ground: [0.02, 0.02, 0.03], strips: [2.6, 2.6, 2.9], key: [-0.4, 0.6, 0.7], keyColor: [1, 0.95, 0.9], fill: [0.6, -0.2, 0.5], fillColor: [0.3, 0.4, 0.9], rimColor: [0.9, 0.5, 1] };
let avg = 6, last = -1, stats = { ms: 0, w: 0, h: 0 };
window.__stage = stats;
const frame = (dt) => {
  if (!reduced) t += Math.min(dt, 50) / 1000;
  sy += (scrollY - sy) * (reduced ? 1 : 1 - Math.exp(-dt / 70));
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
    m.lines = 1.4 * glowing; m.pulse = 2.2 * glowing; m.pulsePhase = t * 0.8 + k * 0.3;
  }
  const t0 = performance.now();
  r3.render(cam, items, env);
  const ms = performance.now() - t0;
  avg = avg * 0.9 + ms * 0.1;
  stats.ms = avg; stats.w = W; stats.h = H;
  const cap = small ? Math.max(0.9, Math.min(devicePixelRatio || 1, 3) * 0.55) : Math.min(devicePixelRatio || 1, 2) * 0.75;
  const want = Math.min(cap, Math.max(0.32, scale * Math.sqrt(7 / Math.max(2, avg))));
  if (Math.abs(want - scale) / scale > 0.12) { scale = want; fit(); }
  if (!canvas.classList.contains('on')) canvas.classList.add('on');
};
engine.onFrame((_t, dt) => frame(dt));

// ---------------------------------------------------------------------------
// Demos load on approach (dynamic import keeps them off the critical path)
// ---------------------------------------------------------------------------
let demos = null;
const loadDemos = () => (demos ??= import('./demos.js').then((m) => m.start({ reduced, fine, small })));
inView('#break', () => { loadDemos(); }, { amount: 0 });
if (scrollY > innerHeight) loadDemos();
