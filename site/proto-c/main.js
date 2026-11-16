import {
  ready, engine, timeline, stagger, staggerValues, spring, inertia, animate,
  motionValue, scrollProgress, flightPath,
} from '../../packages/anikit/dist/anikit.lean.js';
import { buildHeadphones } from './geometry.js';
import { Studio } from './studio.js';

const qs = new URLSearchParams(location.search);
const canvas = document.getElementById('stage');

// ---------------------------------------------------------------- materials
const lin = (hex) => [1, 3, 5].map((i) => (parseInt(hex.slice(i, i + 2), 16) / 255) ** 2.2);
const FINISHES = {
  chrome: { label: 'polished chrome', base: lin('#f4f4f6'), metal: 1, rough: 0.07, brushed: 0, glass: 0 },
  brushed: { label: 'brushed aluminium', base: lin('#e6e7ea'), metal: 1, rough: 0.3, brushed: 1, glass: 0 },
  brushedMist: { label: 'anodised, mist', base: lin('#a9bcd6'), metal: 1, rough: 0.32, brushed: 1, glass: 0 },
  coral: { label: 'anodised, coral', base: lin('#ff8a6e'), metal: 1, rough: 0.3, brushed: 1, glass: 0 },
  sage: { label: 'matte sage', base: lin('#a8b8a0'), metal: 0, rough: 0.55, brushed: 0, glass: 0 },
  glass: { label: 'frosted glass', base: lin('#f3f7fb'), metal: 0, rough: 0.3, brushed: 0, glass: 1 },
  rubberSand: { label: 'soft rubber, sand', base: lin('#e4dbcf'), metal: 0, rough: 0.78, brushed: 0, glass: 0 },
  graphite: { label: 'soft rubber, graphite', base: lin('#4a4d53'), metal: 0, rough: 0.7, brushed: 0, glass: 0 },
  butter: { label: 'soft rubber, butter', base: lin('#f6d98a'), metal: 0, rough: 0.72, brushed: 0, glass: 0 },
};
// What a click cycles through, per group.
const CYCLE = {
  band: ['chrome', 'brushed', 'coral', 'graphite'],
  bandpad: ['rubberSand', 'graphite', 'butter', 'sage'],
  slider: ['brushed', 'chrome', 'coral', 'graphite'],
  yoke: ['chrome', 'brushed', 'graphite', 'coral'],
  cup: ['brushedMist', 'coral', 'brushed', 'sage', 'graphite'],
  glass: ['glass', 'chrome', 'butter', 'brushedMist'],
  pad: ['rubberSand', 'graphite', 'butter', 'sage'],
};

// ---------------------------------------------------------------- frame state (reused every frame)
const F = {
  vp: new Float32Array(16), invVP: new Float32Array(16), shadowVP: new Float32Array(16),
  eye: new Float32Array(3), time: 0, appear: 0, wave: 0, floorY: -8.75, instances: 1,
  sky: new Float32Array(lin('#efedea')), floor: new Float32Array(lin('#e9e6e1')), envBoost: 1.0,
  shadowRect: new Float32Array(4), fade: 0, paper: new Float32Array([0xec / 255, 0xea / 255, 0xe6 / 255]),
};
const view = new Float32Array(16), proj = new Float32Array(16), tmp = new Float32Array(16);

function perspective(o, fovy, aspect, near, far) {
  const f = 1 / Math.tan(fovy / 2), nf = 1 / (near - far);
  o.fill(0);
  o[0] = f / aspect; o[5] = f; o[10] = (far + near) * nf; o[11] = -1; o[14] = 2 * far * near * nf;
}
function lookAt(o, ex, ey, ez, tx, ty, tz) {
  let zx = ex - tx, zy = ey - ty, zz = ez - tz;
  let l = Math.hypot(zx, zy, zz); zx /= l; zy /= l; zz /= l;
  let xx = zz, xy = 0, xz = -zx; // up × z with up = +Y
  l = Math.hypot(xx, xz) || 1; xx /= l; xz /= l;
  const yx = zy * xz - zz * xy, yy = zz * xx - zx * xz, yz = zx * xy - zy * xx;
  o[0] = xx; o[1] = yx; o[2] = zx; o[3] = 0;
  o[4] = xy; o[5] = yy; o[6] = zy; o[7] = 0;
  o[8] = xz; o[9] = yz; o[10] = zz; o[11] = 0;
  o[12] = -(xx * ex + xy * ey + xz * ez); o[13] = -(yx * ex + yy * ey + yz * ez); o[14] = -(zx * ex + zy * ey + zz * ez); o[15] = 1;
}
function mul(o, a, b) {
  for (let c = 0; c < 4; c++) for (let r = 0; r < 4; r++) {
    o[c * 4 + r] = a[r] * b[c * 4] + a[4 + r] * b[c * 4 + 1] + a[8 + r] * b[c * 4 + 2] + a[12 + r] * b[c * 4 + 3];
  }
}
function invert(o, m) {
  const a00 = m[0], a01 = m[1], a02 = m[2], a03 = m[3], a10 = m[4], a11 = m[5], a12 = m[6], a13 = m[7];
  const a20 = m[8], a21 = m[9], a22 = m[10], a23 = m[11], a30 = m[12], a31 = m[13], a32 = m[14], a33 = m[15];
  const b00 = a00 * a11 - a01 * a10, b01 = a00 * a12 - a02 * a10, b02 = a00 * a13 - a03 * a10, b03 = a01 * a12 - a02 * a11;
  const b04 = a01 * a13 - a03 * a11, b05 = a02 * a13 - a03 * a12, b06 = a20 * a31 - a21 * a30, b07 = a20 * a32 - a22 * a30;
  const b08 = a20 * a33 - a23 * a30, b09 = a21 * a32 - a22 * a31, b10 = a21 * a33 - a23 * a31, b11 = a22 * a33 - a23 * a32;
  const d = 1 / (b00 * b11 - b01 * b10 + b02 * b09 + b03 * b08 - b04 * b07 + b05 * b06);
  o[0] = (a11 * b11 - a12 * b10 + a13 * b09) * d; o[1] = (a02 * b10 - a01 * b11 - a03 * b09) * d;
  o[2] = (a31 * b05 - a32 * b04 + a33 * b03) * d; o[3] = (a22 * b04 - a21 * b05 - a23 * b03) * d;
  o[4] = (a12 * b08 - a10 * b11 - a13 * b07) * d; o[5] = (a00 * b11 - a02 * b08 + a03 * b07) * d;
  o[6] = (a32 * b02 - a30 * b05 - a33 * b01) * d; o[7] = (a20 * b05 - a22 * b02 + a23 * b01) * d;
  o[8] = (a10 * b10 - a11 * b08 + a13 * b06) * d; o[9] = (a01 * b08 - a00 * b10 - a03 * b06) * d;
  o[10] = (a30 * b04 - a31 * b02 + a33 * b00) * d; o[11] = (a21 * b02 - a20 * b04 - a23 * b00) * d;
  o[12] = (a11 * b07 - a10 * b09 - a12 * b06) * d; o[13] = (a00 * b09 - a01 * b07 + a02 * b06) * d;
  o[14] = (a31 * b01 - a30 * b03 - a32 * b00) * d; o[15] = (a20 * b03 - a21 * b01 + a22 * b00) * d;
}
/** Axis-angle rotation into a 3×3 (column-major). */
function rot3(o, x, y, z, a) {
  const c = Math.cos(a), s = Math.sin(a), t = 1 - c;
  o[0] = t * x * x + c; o[1] = t * x * y + s * z; o[2] = t * x * z - s * y;
  o[3] = t * x * y - s * z; o[4] = t * y * y + c; o[5] = t * y * z + s * x;
  o[6] = t * x * z + s * y; o[7] = t * y * z - s * x; o[8] = t * z * z + c;
}

// ---------------------------------------------------------------- scene
let studio;
try {
  studio = new Studio(canvas);
} catch (e) {
  document.body.classList.add('no-gl');
  throw e;
}
await ready;

// 300 instances: a 20×15 grid around the hero, which is instance 0.
const COLS = 20, ROWS = 15, N = COLS * ROWS, HERO = 7 * COLS + 10;
const delays = staggerValues(N, 1, { grid: [COLS, ROWS], from: HERO, ease: 'inQuad' });
const maxDelay = Math.max(...delays);
const inst = new Float32Array(N * 4);
{
  let k = 1;
  for (let j = 0; j < ROWS; j++) for (let i = 0; i < COLS; i++) {
    const idx = j * COLS + i, slot = idx === HERO ? 0 : k++;
    const o = slot * 4;
    inst[o] = (i - 10) * 23 + (j % 2) * 6;
    inst[o + 1] = (j - 7) * 21;
    inst[o + 2] = idx === HERO ? 0 : 0.04 + (delays[idx] / maxDelay) * 0.95;
    inst[o + 3] = ((idx * 2654435761) % 1000) / 1000 * 0.8;
  }
}

const hero = buildHeadphones(1);
const lod = buildHeadphones(0.32);
const parts = hero.map((p, i) => {
  const fin = FINISHES[p.mat];
  const m = { r: fin.base[0], g: fin.base[1], b: fin.base[2], metal: fin.metal, rough: fin.rough, brushed: fin.brushed, glass: fin.glass };
  return {
    ...p, e: 0, hi: 0, finish: p.mat, m,
    gpu: studio.addMesh(p.mesh, inst), lod: studio.addMesh(lod[i].mesh, inst),
    rot: new Float32Array([1, 0, 0, 0, 1, 0, 0, 0, 1]), offset: new Float32Array(3),
    mat: { base: new Float32Array(fin.base), params: new Float32Array([fin.metal, fin.rough, fin.brushed, fin.glass]) },
    spinAxis: (() => { const [x, y, z] = p.spin; const l = Math.hypot(x, y, z); return [x / l, y / l, z / l]; })(),
  };
});
const groups = {};
for (const p of parts) (groups[p.group] ||= []).push(p);

// ---------------------------------------------------------------- choreography (one anikit timeline, scrubbed by scroll)
const rig = { dist: 50, ty: -1.2, lift: 0, yawOff: 0, pitchOff: 0, labels: 0, fin: 0, appear: 0, wave: 0, fov: 30 };
const order = ['band', 'bandpad', 'slider', 'yoke', 'cup', 'glass', 'pad'];
const explodeOrder = [...parts].sort((a, b) => order.indexOf(a.group) - order.indexOf(b.group));
const tl = timeline({ autoplay: false })
  .add(rig, { dist: 66, lift: 3.2, yawOff: -0.55, pitchOff: 0.12, ty: 0 }, { duration: 3, ease: 'inOutCubic' }, 1)
  .add(explodeOrder, { e: 1 }, { duration: 1.8, ease: 'inOutCubic', delay: stagger(0.11) }, 1.2)
  .add(rig, { labels: 1 }, { duration: 0.6, ease: 'outCubic' }, 3.9)
  .label('hold', 4.5)
  .add(rig, { labels: 0 }, { duration: 0.4, ease: 'inCubic' }, 6)
  .add([...explodeOrder].reverse(), { e: 0 }, { duration: 1.3, ease: 'inOutCubic', delay: stagger(0.06) }, 6.1)
  .add(rig, { lift: 0, yawOff: 0, pitchOff: 0, dist: 50, ty: -1.2 }, { duration: 1.4, ease: 'inOutCubic' }, 6.2)
  .add(rig, { fin: 1 }, { duration: 2.6, ease: 'inOutSine' }, 7.6)
  .add(rig, { appear: 1.05 }, { duration: 1.9, ease: 'linear' }, 7.9)
  .add(rig, { wave: 1 }, { duration: 1.4, ease: 'inOutSine' }, 8.6)
  .add(rig, { fov: 30 }, { duration: 0.4 }, 10);
const TL_LEN = tl.duration;

// Finale camera: a crane move along a smooth anikit flight path, sampled once.
const crane = flightPath([[0, 6, 50], [26, 26, 92], [52, 62, 150], [64, 96, 200]], { lookAhead: 40, bank: 0 });
const CRANE_N = 240, craneXYZ = new Float32Array(CRANE_N * 3);
crane.points(CRANE_N).forEach((p, i) => craneXYZ.set(p, i * 3));

// Scroll → spring-smoothed progress (anikit motion values).
const scrolled = scrollProgress();
const progress = motionValue(0);
scrolled.on((v) => progress.spring(v, { stiffness: 70, damping: 20 }));

// ---------------------------------------------------------------- orbit: drag with inertia, settle with a spring
const orbit = { yaw: -0.5, pitch: 0.16, drift: 0 };
let drag = null, orbitAnims = [];
const stopOrbit = () => { for (const a of orbitAnims) a.kill(); orbitAnims.length = 0; };
canvas.addEventListener('pointerdown', (e) => {
  stopOrbit();
  drag = { x: e.clientX, y: e.clientY, t: performance.now(), vx: 0, vy: 0, moved: 0, id: e.pointerId };
  canvas.setPointerCapture(e.pointerId);
  document.body.classList.add('grabbing');
});
canvas.addEventListener('pointermove', (e) => {
  if (!drag) return;
  const now = performance.now(), dt = Math.max(1, now - drag.t) / 1000;
  const dx = e.clientX - drag.x, dy = e.clientY - drag.y;
  orbit.yaw -= dx * 0.0065;
  orbit.pitch = Math.max(-0.12, Math.min(0.85, orbit.pitch + dy * 0.004));
  drag.vx = drag.vx * 0.6 + (-dx * 0.0065 / dt) * 0.4;
  drag.vy = drag.vy * 0.6 + (dy * 0.004 / dt) * 0.4;
  drag.moved += Math.abs(dx) + Math.abs(dy);
  drag.x = e.clientX; drag.y = e.clientY; drag.t = now;
});
const endDrag = (e) => {
  if (!drag) return;
  document.body.classList.remove('grabbing');
  const d = drag; drag = null;
  if (d.moved < 6 && e.type === 'pointerup') { click(e); return; }
  const v = performance.now() - d.t > 80 ? 0 : d.vx;
  orbitAnims.push(inertia(orbit, { yaw: { velocity: v } }, { timeConstant: 0.5 }));
  orbitAnims.push(spring(orbit, { pitch: 0.16 }, { stiffness: 60, damping: 11, velocity: d.vy }));
};
canvas.addEventListener('pointerup', endDrag);
canvas.addEventListener('pointercancel', endDrag);

// ---------------------------------------------------------------- click a part: change its finish, spring-smoothed
function click(e) {
  if (rig.appear > 0.05) return;
  const r = canvas.getBoundingClientRect();
  const id = studio.pick(F, parts, (e.clientX - r.left) / r.width, (e.clientY - r.top) / r.height);
  if (!id) return;
  const g = parts[id - 1].group;
  const list = CYCLE[g];
  const next = list[(list.indexOf(groups[g][0].finish) + 1) % list.length];
  const fin = FINISHES[next];
  for (const p of groups[g]) {
    p.finish = next;
    spring(p.m, { r: fin.base[0], g: fin.base[1], b: fin.base[2], metal: fin.metal, rough: fin.rough, brushed: fin.brushed, glass: fin.glass }, { stiffness: 120, damping: 16 });
    animate(p, { hi: [1, 0] }, { duration: 0.9, ease: 'outCubic' });
  }
  const label = labels.find((l) => l.group === g);
  if (label) animate(label.sub, { text: { value: fin.label, type: 'scramble' } }, { duration: 0.5 });
}

// ---------------------------------------------------------------- labels pinned in 3D (HTML, positioned per frame)
const LABELS = [['band', 'Headband'], ['bandpad', 'Band cushion'], ['slider', 'Slider'], ['yoke', 'Yoke'], ['cup', 'Housing'], ['glass', 'Glass cap'], ['pad', 'Ear cushion']];
const labelLayer = document.getElementById('labels');
const labels = LABELS.map(([group, name]) => {
  const part = groups[group].find((p) => p.centre[0] < 0.5) || groups[group][0];
  const el = document.createElement('div');
  el.className = 'label';
  el.innerHTML = `<b></b><span></span>`;
  el.querySelector('b').textContent = name;
  const sub = el.querySelector('span');
  sub.textContent = FINISHES[part.finish].label;
  labelLayer.appendChild(el);
  return { group, part, el, sub, x: 0, y: 0, o: -1 };
});

// ---------------------------------------------------------------- captions
const captions = [...document.querySelectorAll('[data-at]')].map((el) => ({ el, a: +el.dataset.at, b: +el.dataset.to, o: -1 }));

// ---------------------------------------------------------------- frame
let cssW = 0, cssH = 0;
function resize() {
  cssW = canvas.clientWidth; cssH = canvas.clientHeight;
  studio.resize(cssW, cssH);
}
addEventListener('resize', resize);
resize();

const P = new Float32Array(4);
function project(x, y, z) {
  const m = F.vp;
  const cx = m[0] * x + m[4] * y + m[8] * z + m[12], cy = m[1] * x + m[5] * y + m[9] * z + m[13], cw = m[3] * x + m[7] * y + m[11] * z + m[15];
  P[0] = (cx / cw * 0.5 + 0.5) * cssW; P[1] = (0.5 - cy / cw * 0.5) * cssH; P[2] = cw;
}

function update(dtMs) {
  const dt = Math.min(0.05, dtMs / 1000);
  F.time += dt;
  const p = Math.min(1, Math.max(0, progress.get()));
  tl.seek(p * TL_LEN);

  // Idle drift on the hero only while nobody is dragging; eased in and out.
  const idle = !drag && orbitAnims.every((a) => a.finished === undefined || true) ? 1 : 0;
  orbit.drift += ((idle && p < 0.02 ? 1 : 0) - orbit.drift) * Math.min(1, dt * 1.5);
  if (!drag) orbit.yaw += orbit.drift * dt * 0.12;

  // Parts.
  const lift = rig.lift;
  for (let i = 0; i < parts.length; i++) {
    const q = parts[i], e = q.e;
    q.offset[0] = q.explode[0] * e; q.offset[1] = q.explode[1] * e + lift; q.offset[2] = q.explode[2] * e;
    rot3(q.rot, q.spinAxis[0], q.spinAxis[1], q.spinAxis[2], (q.spin[3] * Math.PI / 180) * e);
    const m = q.m, b = q.mat.base, pr = q.mat.params;
    b[0] = Math.max(0, m.r); b[1] = Math.max(0, m.g); b[2] = Math.max(0, m.b);
    pr[0] = Math.min(1, Math.max(0, m.metal)); pr[1] = Math.min(1, Math.max(0.03, m.rough)); pr[2] = Math.max(0, m.brushed); pr[3] = Math.min(1, Math.max(0, m.glass));
  }

  // Camera: orbit around the hero, blended into the crane path for the finale.
  const yaw = orbit.yaw + rig.yawOff, pitch = orbit.pitch + rig.pitchOff, d = rig.dist;
  let ex = Math.sin(yaw) * Math.cos(pitch) * d, ey = rig.ty + Math.sin(pitch) * d, ez = Math.cos(yaw) * Math.cos(pitch) * d;
  let tx = 0, ty = rig.ty, tz = 0;
  const fin = rig.fin;
  if (fin > 0) {
    const s = fin * (CRANE_N - 1), k = Math.min(CRANE_N - 2, Math.floor(s)), f = s - k, o = k * 3;
    const cx = craneXYZ[o] + (craneXYZ[o + 3] - craneXYZ[o]) * f, cy = craneXYZ[o + 1] + (craneXYZ[o + 4] - craneXYZ[o + 1]) * f, cz = craneXYZ[o + 2] + (craneXYZ[o + 5] - craneXYZ[o + 2]) * f;
    // Rotate the crane by the user's orbit so dragging still works at the end.
    const cs = Math.cos(orbit.yaw + 0.5), sn = Math.sin(orbit.yaw + 0.5);
    const w = fin * fin * (3 - 2 * fin);
    ex += (cx * cs + cz * sn - ex) * w; ey += (cy - 2 - ey) * w; ez += (-cx * sn + cz * cs - ez) * w;
    ty += (-14 - ty) * w; tz += (-30 * cs - tz) * w * 0; tx += 0;
  }
  F.eye[0] = ex; F.eye[1] = ey; F.eye[2] = ez;
  perspective(proj, (rig.fov * Math.PI) / 180, cssW / cssH, 1, 2000);
  lookAt(view, ex, ey, ez, tx, ty, tz);
  mul(F.vp, proj, view);
  invert(F.invVP, F.vp);

  F.appear = rig.appear; F.wave = rig.wave;
  F.instances = rig.appear > 0.001 ? N : 1;

  // Contact-shadow window: tight around the hero, the whole field for the finale.
  const half = 22 + 240 * Math.min(1, rig.appear * 1.6);
  F.shadowRect[0] = -half; F.shadowRect[1] = -half; F.shadowRect[2] = 1 / (2 * half); F.shadowRect[3] = 1 / (2 * half);
  F.shadowVP.fill(0);
  F.shadowVP[0] = 1 / half; F.shadowVP[9] = 1 / half; F.shadowVP[15] = 1;

  // Labels: anchored to each exploded part, written only while visible.
  const lo = rig.labels;
  for (const l of labels) {
    const q = l.part;
    project(q.centre[0] + q.offset[0], q.centre[1] + q.offset[1], q.centre[2] + q.offset[2]);
    const o = lo < 0.01 ? 0 : lo;
    if (o > 0 || l.o !== 0) {
      l.el.style.transform = `translate3d(${P[0].toFixed(1)}px, ${P[1].toFixed(1)}px, 0) translate(-50%, -50%) scale(${(0.92 + 0.08 * o).toFixed(3)})`;
      l.el.style.opacity = o.toFixed(3);
      l.o = o;
    }
  }
  // Captions: cross-fade by timeline time.
  const t = p * TL_LEN;
  for (const c of captions) {
    const o = Math.max(0, Math.min(1, (t - c.a) / 0.35, (c.b - t) / 0.35));
    if (Math.abs(o - c.o) > 0.002) { c.el.style.opacity = o.toFixed(3); c.el.style.transform = `translateY(${((1 - o) * 10).toFixed(1)}px)`; c.o = o; }
  }
}

// ---------------------------------------------------------------- perf overlay + quality governor
const times = new Float32Array(240), sorted = new Float32Array(240);
let tn = 0, lastT = 0, lastReport = 0, govT = 0, warm = 0;
const perfEl = qs.has('perf') ? Object.assign(document.body.appendChild(document.createElement('pre')), { id: 'perf' }) : null;
const pinned = qs.get('q');
if (pinned !== null) { studio.quality = +pinned; if (studio.quality <= 0) studio.resScale = 0.75; resize(); }
function stats() {
  const n = Math.min(tn, 240);
  sorted.set(times); const s = sorted.subarray(0, n).sort();
  return [s[Math.floor(n * 0.5)] || 0, s[Math.floor(n * 0.95)] || 0, s[n - 1] || 0];
}
function govern(now) {
  if (pinned !== null || tn < 90 || now - govT < 1500) return;
  const [, p95] = stats();
  if (p95 > 22 && studio.quality > 0) {
    studio.quality--;
    if (studio.quality === 0) studio.resScale = 0.75;
    resize();
    govT = now; tn = 0;
  }
}

function frame(now, dtMs) {
  if (lastT) { times[tn++ % 240] = now - lastT; }
  lastT = now;
  update(dtMs);
  studio.render(F, parts);
  if (++warm > 3) govern(now);
  if (perfEl && now - lastReport > 500) {
    const [a, b, c] = stats();
    perfEl.textContent = `frame p50 ${a.toFixed(1)}  p95 ${b.toFixed(1)}  max ${c.toFixed(1)} ms\nquality ${studio.quality}  ${studio.w}×${studio.h}  ${F.instances} copies`;
    lastReport = now;
  }
}

// Pre-warm every path (finale instancing, pick) before the first visible frame.
F.instances = N; F.appear = 1.4; update(16);
F.appear = 1.4; F.instances = N; studio.render(F, parts);
studio.pick(F, parts, 0.5, 0.5);
update(16); studio.render(F, parts);

engine.onFrame((t, dt) => frame(performance.now(), dt));
animate(F, { fade: 1 }, { duration: 1.1, ease: 'outCubic', delay: 0.05 });
document.body.classList.add('ready');
