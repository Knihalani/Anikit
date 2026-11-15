// ANIKIT — a title-sequence film composed in code and rendered by the anikit
// engine itself, in Node. No browser, no screenshots: the WebAssembly core
// simulates and lights the particles, runs the physics, projects the 3D and
// drives every tween and camera move; a native Skia canvas draws type and
// vectors; raw frames stream into ffmpeg.
//
//   node film/render.mjs out.mp4                full render (1080p60)
//   PREVIEW=30 node film/render.mjs dir/        every 30th frame as JPEG
//   FROM=20 END=30 …                            part of the timeline
//   WORKERS=3 …                                 render slices in parallel
import { createCanvas, GlobalFonts, ImageData } from '@napi-rs/canvas';
import { spawn } from 'node:child_process';
import { mkdirSync, writeFileSync, rmSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const OUT = process.argv[2] || 'anikit-film.mp4';
const W = 1920, H = 1080, FPS = 60;
const PREVIEW = +(process.env.PREVIEW || 0);
const FROM = +(process.env.FROM || 0);
const END = +(process.env.END || 60);
const WORKERS = +(process.env.WORKERS || 1);
const SEG = process.env.SEG;

// ---------------------------------------------------------------------------
// Parallel slices: each worker simulates from t = 0 (no drawing) and renders
// its own slice; everything is seeded, so slices are identical and join
// seamlessly. No browser is involved, so nothing can go stale.
// ---------------------------------------------------------------------------
if (!SEG && !PREVIEW && WORKERS > 1) {
  const frames = Math.round(END * FPS), start = Math.round(FROM * FPS), per = Math.ceil((frames - start) / WORKERS);
  const tmp = OUT + '.parts';
  mkdirSync(tmp, { recursive: true });
  const parts = [];
  const t0 = Date.now();
  await Promise.all(Array.from({ length: WORKERS }, (_, k) => {
    const a = start + k * per, b = Math.min(frames, a + per), file = join(tmp, `p${k}.mp4`);
    parts.push(file);
    return new Promise((res, rej) => spawn(process.execPath, [fileURLToPath(import.meta.url), OUT], { env: { ...process.env, SEG: `${a}:${b}:${file}` }, stdio: 'inherit' })
      .on('close', (c) => (c ? rej(new Error('worker ' + k)) : res())));
  }));
  writeFileSync(join(tmp, 'list.txt'), parts.map((f) => `file '${f}'`).join('\n'));
  await new Promise((res, rej) => spawn('ffmpeg', ['-y', '-loglevel', 'error', '-f', 'concat', '-safe', '0', '-i', join(tmp, 'list.txt'), '-c', 'copy', '-movflags', '+faststart', OUT], { stdio: 'inherit' })
    .on('close', (c) => (c ? rej(new Error('concat')) : res())));
  rmSync(tmp, { recursive: true, force: true });
  console.log(`done → ${OUT} · ${frames - start} frames in ${((Date.now() - t0) / 1000).toFixed(0)}s (${WORKERS} workers)`);
  process.exit(0);
}

// ---------------------------------------------------------------------------
// The engine, in Node
// ---------------------------------------------------------------------------
const FONT = '/usr/share/fonts';
GlobalFonts.registerFromPath(`${FONT}/opentype/inter/InterDisplay-Black.otf`, 'Display');
GlobalFonts.registerFromPath(`${FONT}/opentype/inter/InterDisplay-BlackItalic.otf`, 'DisplayItalic');
GlobalFonts.registerFromPath(`${FONT}/truetype/jetbrains-mono/JetBrainsMono-Regular.ttf`, 'Mono');
globalThis.ImageData = ImageData;
globalThis.document = { createElement: () => createCanvas(1, 1) };
globalThis.__ANIKIT_MANUAL__ = true; // the film ticks the engine itself, exactly 1/60 s per frame
{
  let a = 0x2545f491;
  Math.random = () => {
    a |= 0; a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}
const A = await import(join(here, '../packages/anikit/dist/anikit.js'));
await A.ready;
const { engine, animate, pointCloud, glow, scene3d, textPoints, world, ease, noise2, delayedCall } = A;

const C = { paper: '#ece6da', ink: '#0c0c0c', red: '#ff3b14', blue: '#1f2bff', lime: '#d2ff3c', black: '#050505' };
const TAU = Math.PI * 2;
const clamp = (v, a = 0, b = 1) => Math.min(b, Math.max(a, v));
const prog = (t, a, b) => clamp((t - a) / (b - a));
const io = (t) => (t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2);
const outExpo = (t) => (t >= 1 ? 1 : 1 - Math.pow(2, -10 * t));

const main = createCanvas(W, H);
const ctx = main.getContext('2d');

// anikit's glow() renderer, drawing into a Skia canvas instead of a DOM one.
const buf = createCanvas(1, 1);
const shim = {
  style: {}, clientWidth: W, clientHeight: H,
  get width() { return buf.width; }, set width(v) { buf.width = v; },
  get height() { return buf.height; }, set height(v) { buf.height = v; },
  getContext: () => buf.getContext('2d'),
};
const g = glow(shim, { resolution: 0.85, background: C.black, mode: 'light', trail: 0.5, budget: 0 });
g.resize(W, H);
const cam = scene3d({ fov: 40, camera: { z: 1000 }, width: W, height: H, near: 4, far: 30000 });
const camReset = (z = 1000) => {
  Object.assign(cam.camera, { x: 0, y: 0, z });
  Object.assign(cam.target, { x: 0, y: 0, z: 0 });
  Object.assign(cam.rotation, { x: 0, y: 0, z: 0 });
};

// ---------------------------------------------------------------------------
// Shapes (160,000 points each)
// ---------------------------------------------------------------------------
const N = 160_000;
let seed = 11;
const rnd = () => ((seed = (seed * 1664525 + 1013904223) >>> 0) / 4294967296);
const gauss = () => (rnd() + rnd() + rnd() - 1.5) * 1.15;
const make = (fn) => { const a = new Float32Array(N * 3); for (let i = 0; i < N; i++) fn(i, a, i * 3); return a; };
const fib = (i, n) => { const y = 1 - ((i + 0.5) / n) * 2, r = Math.sqrt(1 - y * y), t = i * 2.399963; return [Math.cos(t) * r, y, Math.sin(t) * r]; };
const text = (s, width, dy = 0, depth = 12) => {
  const t = textPoints(s, N, { font: 'Display', weight: 900, width, depth });
  for (let i = 1; i < t.length; i += 3) t[i] += dy;
  return t;
};
const rotX = (a, o, deg) => { const c = Math.cos((deg * Math.PI) / 180), s = Math.sin((deg * Math.PI) / 180), y = a[o + 1], z = a[o + 2]; a[o + 1] = y * c - z * s; a[o + 2] = y * s + z * c; };
const shapes = {
  origin: make((i, a, o) => { const c = fib(i, N), r = 1.5 * Math.cbrt(rnd()); a[o] = c[0] * r; a[o + 1] = c[1] * r; a[o + 2] = c[2] * r; }),
  blast: make((i, a, o) => { const c = fib(i, N), r = 170 * Math.cbrt(0.2 + 0.8 * rnd()); a[o] = c[0] * r; a[o + 1] = c[1] * r; a[o + 2] = c[2] * r; }),
  // 500 dense clumps in a ball: blown apart they read as streaking debris.
  debris: (() => { const K = 500, cs = Array.from({ length: K }, (_, k) => { const c = fib(k, K), r = 150 * Math.cbrt(rnd()); return [c[0] * r, c[1] * r, c[2] * r]; });
    return make((i, a, o) => { const c = cs[i % K]; a[o] = c[0] + gauss() * 6; a[o + 1] = c[1] + gauss() * 6; a[o + 2] = c[2] + gauss() * 6; }); })(),
  // The wordmark extruded 240 units deep, so the orbiting camera reveals its depth.
  title: text('anikit', 1150, 40, 180),
  compiled: text('COMPILED.', 1180, 10),
  knot: make((i, a, o) => {
    const t = (i / N) * TAU, p = 2, q = 5, R = 300, rr = 130;
    const r = R + rr * Math.cos(q * t), tube = 34 * Math.sqrt(rnd()), ang = rnd() * TAU;
    a[o] = r * Math.cos(p * t) + tube * Math.cos(ang); a[o + 1] = r * Math.sin(p * t) + tube * Math.sin(ang); a[o + 2] = rr * 1.8 * Math.sin(q * t) + tube * Math.cos(ang * 1.7);
  }),
  wasm: text('WASM.', 1150, 0),
  galaxy: make((i, a, o) => {
    if (i % 9 === 0) { const c = fib(i, N), r = 90 * Math.pow(rnd(), 1.5); a[o] = c[0] * r; a[o + 1] = c[1] * r * 0.6; a[o + 2] = c[2] * r; return; }
    const arm = i % 4, d = Math.pow(rnd(), 0.7), r = 40 + d * 1250;
    const ang = (arm / 4) * TAU + d * 5.2 + gauss() * 0.22 * (1.1 - d);
    a[o] = Math.cos(ang) * r + gauss() * 22; a[o + 1] = gauss() * (26 * (1 - d) + 5); a[o + 2] = Math.sin(ang) * r + gauss() * 22;
  }),
  wordmark: text('anikit', 1050, 110, 30),
};
const names = Object.keys(shapes);
const S = Object.fromEntries(names.map((n, i) => [n, i]));
const cloud = pointCloud(N, { autoStep: false, shapes: names.map((n) => shapes[n]), stiffness: 34, damping: 7.5, spread: 0.7, noise: 1.6, swirl: 60, frequency: 0.012 });
cloud.snap(S.origin);
g.colors(N, () => { const r = rnd(); const k = r < 0.05 ? 1.7 : 0.5 + rnd() * 0.5; return [k, k, k]; });
/** Flow the cloud to a shape with a tween on its blend. */
const flow = (to, dur, e = 'inOutCubic') => {
  cloud.from = cloud.mix >= 0.5 ? cloud.to : cloud.from;
  cloud.to = S[to];
  cloud.mix = 0;
  animate(cloud, { mix: 1 }, { duration: dur, ease: e });
};
let clock = 1; // the particles' own time scale (cold open freezes it)
const TC = { c: 1 };

// ---------------------------------------------------------------------------
// Drawing helpers
// ---------------------------------------------------------------------------
const label = (s, x, y, color, align = 'left', size = 15) => {
  ctx.font = `400 ${size}px Mono`;
  ctx.fillStyle = color;
  ctx.textAlign = align;
  ctx.textBaseline = 'alphabetic';
  ctx.fillText(s.toUpperCase(), x, y);
};
const big = (s, x, y, px, color, align = 'left') => {
  ctx.font = `900 ${px}px Display`;
  ctx.textAlign = align;
  ctx.textBaseline = 'alphabetic';
  ctx.fillStyle = color;
  ctx.fillText(s, x, y);
};
/** Characters rise into place one by one behind a mask. */
const rise = (s, x, y, px, color, t0, t, o = {}) => {
  ctx.font = `900 ${px}px ${o.italic ? 'DisplayItalic' : 'Display'}`;
  ctx.textAlign = 'left';
  ctx.textBaseline = 'alphabetic';
  const st = o.stagger ?? 0.03;
  let cx = o.align === 'right' ? x - ctx.measureText(s).width : x;
  ctx.save();
  ctx.beginPath();
  ctx.rect(0, y - px * 1.02, W, px * 1.3);
  ctx.clip();
  ctx.fillStyle = color;
  for (let i = 0; i < s.length; i++) {
    const p = outExpo(prog(t, t0 + i * st, t0 + i * st + 0.75));
    ctx.fillText(s[i], cx, y + (1 - p) * px * 1.1);
    cx += ctx.measureText(s[i]).width;
  }
  ctx.restore();
};
const fill = (c) => { ctx.fillStyle = c; ctx.fillRect(0, 0, W, H); };
const particles = (o = {}) => {
  const ink = g.options.mode === 'ink';
  g.render(cam, cloud, { size: o.size ?? (ink ? 2.1 : 1.2), intensity: o.intensity ?? (ink ? 1.6 : 0.36), fog: o.fog ?? [500, 4200], exposure: o.exposure ?? 1, tint: o.tint, maxRadius: o.maxRadius ?? 5 });
  ctx.drawImage(buf, 0, 0, W, H);
};

// ---------------------------------------------------------------------------
// Movements
// ---------------------------------------------------------------------------
const M = [];
const movement = (name, t0, t1, bg, fg, fns) => M.push({ name, t0, t1, bg, fg, ...fns });

// 01 · COLD OPEN — 160,000 particles frozen mid-explosion; the camera walks through.
movement('Frozen', 0, 4.6, C.black, C.paper, {
  enter() {
    g.options.mode = 'light'; g.options.background = C.black; g.options.trail = 0.35;
    camReset(130);
    cam.rotation.x = 14;
    cloud.snap(S.debris);
    cloud.explode(0, 0, 0, 3400);
    for (let k = 0; k < 28; k++) cloud.step(1 / 60); // a few frames into the blast
    clock = 0;
    cloud.from = cloud.to = S.debris; cloud.mix = 0;
    // The camera starts inside the frozen debris and pulls back through it.
    animate(cam.rotation, { y: [-50, 110], x: [24, 8] }, { duration: 3.6, ease: 'inOutCubic' });
    animate(cam.camera, { z: [130, 1250] }, { duration: 3.6, ease: 'inOutCubic' });
    TC.c = 0;
    // Time resumes (a tween on the particles' clock), then everything implodes.
    delayedCall(3.0, () => animate(TC, { c: 1 }, { duration: 0.4, ease: 'inQuad' }));
    delayedCall(3.3, () => flow('origin', 0.9, 'inExpo'));
  },
  step() { clock = TC.c; },
  draw(t) {
    particles({ tint: [1, 0.68, 0.42], intensity: 0.62, size: 1.6, maxRadius: 9, fog: [200, 3800] });
    if (t > 3.9) { ctx.fillStyle = `rgba(255,248,236,${io(prog(t, 3.9, 4.6))})`; ctx.fillRect(0, 0, W, H); }
    big(`${clock.toFixed(2)}×`, W - 80, H - 100, 170, C.paper, 'right');
    label(clock < 0.01 ? '160,000 particles · time stopped · camera still moving' : 'time resumes', W - 86, H - 62, C.paper, 'right');
    label('frame-exact · rendered by anikit · no browser', 86, H - 62, C.paper);
  },
});

// 02 · TITLE — an extruded 3D wordmark in ink.
movement('Title', 4.6, 8.8, C.paper, C.ink, {
  enter() {
    clock = 1;
    g.options.mode = 'ink'; g.options.background = C.paper; g.options.trail = 0.5;
    camReset(1000);
    cloud.snap(S.origin);
    cloud.explode(0, 0, 0, 2200);
    cloud.from = S.origin; cloud.to = S.title; cloud.mix = 0;
    animate(cloud, { mix: 1 }, { duration: 1.5, ease: 'outCubic' });
    animate(cam.rotation, { y: [-34, 22], x: [18, 4] }, { duration: 4.2, ease: 'inOutCubic' });
    animate(cam.camera, { z: [1250, 980] }, { duration: 4.2, ease: 'outCubic' });
  },
  draw(t) {
    particles({ intensity: 1.9 });
    rise('Motion,', 80, H - 190, 118, C.ink, 5.9, t);
    rise('compiled.', 80, H - 76, 118, C.red, 6.05, t, { italic: true });
    label('a web animation engine · rust → webassembly', W - 86, H - 62, C.ink, 'right');
  },
});

// 03 · SAND — a word of 9,000 rigid bodies collapses, then time runs backwards.
let sand = null;
movement('Sand', 8.8, 18.0, C.lime, C.ink, {
  enter() {
    const off = createCanvas(W, H), o = off.getContext('2d');
    o.font = '900 400px Display'; o.textAlign = 'center'; o.textBaseline = 'middle'; o.fillStyle = '#fff';
    o.fillText('MOTION', W / 2, H / 2 - 40);
    const img = o.getImageData(0, 0, W, H).data, pts = [], sp = 5.2;
    for (let y = 0; y < H; y += sp) for (let x = 0; x < W; x += sp) if (img[(Math.round(y) * W + Math.round(x)) * 4 + 3] > 140) pts.push({ x: x + (rnd() - 0.5) * 0.6, y: y + (rnd() - 0.5) * 0.6, r: 2.45, mass: 1 });
    const w = world({ bounds: [0, -4000, W, H - 96], gravity: [0, 0], substeps: 3, iterations: 2, restitution: 0.08, friction: 0.35, damping: 0.03 });
    w.addMany(pts);
    sand = { w, n: pts.length, hist: [], mode: 'hold', k: 0 };
    delayedCall(1.6, () => {
      sand.mode = 'fall';
      w.set({ gravity: [0, 1700] });
      for (let i = 0; i < sand.n; i++) w.setVelocity(i, (rnd() - 0.5) * 120, -rnd() * 160);
    });
    delayedCall(5.6, () => {
      sand.mode = 'rewind';
      w.pause();
      sand.k = sand.hist.length - 1;
      // Rewind = a tween over the recorded history.
      animate(sand, { k: 0 }, { duration: 2.6, ease: 'inOutCubic', delay: 0.3, onComplete: () => (sand.mode = 'done') });
    });
  },
  step() {
    const s = sand;
    if (s.mode === 'fall' || (s.mode === 'hold' && !s.hist.length)) {
      const d = s.w.data, snap = new Float32Array(s.n * 2);
      for (let i = 0; i < s.n; i++) { snap[i * 2] = d[i * 3]; snap[i * 2 + 1] = d[i * 3 + 1]; }
      s.hist.push(snap);
    }
  },
  draw(t, lt) {
    const s = sand;
    const frame = s.mode === 'rewind' || s.mode === 'done' ? s.hist[Math.round(s.k)] : s.hist[s.hist.length - 1];
    fill(C.lime);
    ctx.fillStyle = C.ink;
    for (let i = 0; i < s.n; i++) ctx.fillRect(frame[i * 2] - 2.25, frame[i * 2 + 1] - 2.25, 4.5, 4.5);
    ctx.fillRect(0, H - 96 + 2.5, W, 2);
    label(`${s.n.toLocaleString('en-US')} rigid bodies · one webassembly physics world`, 86, H - 48, C.ink);
    const rw = s.mode === 'rewind';
    big(rw ? '◀◀ −1×' : s.mode === 'done' ? '■' : '1×', W - 80, 300, rw ? 150 : 110, rw ? C.red : C.ink, 'right');
    if (s.mode === 'done') rise('Deterministic. Rewindable.', 86, 210, 70, C.ink, s.doneAt ?? (s.doneAt = t), t);
  },
});

// 04 · FORM — type into a torus knot, and the camera flies straight through it.
movement('Through', 18.0, 27.6, C.red, C.ink, {
  enter() {
    g.options.mode = 'ink'; g.options.background = C.red; g.options.trail = 0.5;
    camReset(1000);
    cloud.snap(S.blast); cloud.explode(0, 0, 0, 2600);
    cloud.from = S.blast; cloud.to = S.compiled; cloud.mix = 0;
    animate(cloud, { mix: 1 }, { duration: 1.4, ease: 'outCubic' });
    delayedCall(2.5, () => flow('knot', 1.3));
    // Orbit the knot a full turn (ending square-on), then dive through its centre.
    animate(cam.rotation, { y: [0, 360], x: [0, 30, 0] }, { duration: 3.2, ease: 'inOutCubic', delay: 2.6 });
    delayedCall(5.7, () => {
      animate(cam.camera, { z: [1000, -900] }, { duration: 1.9, ease: 'inOutQuart' });
      animate(cam.target, { z: [0, -1900] }, { duration: 1.9, ease: 'inOutQuart' });
    });
    delayedCall(7.6, () => { camReset(1000); cloud.snap(S.knot); cloud.explode(0, 0, 0, 900); flow('wasm', 1.2, 'outCubic'); });
  },
  draw(t, lt) {
    particles({ intensity: 1.8, maxRadius: 8 });
    label('160,000 particles · ink on vermilion · one spring each', 86, H - 48, C.ink);
    label(lt < 2.5 ? 'type → points' : lt < 5.7 ? 'points → torus knot (2,5) · orbit' : lt < 7.6 ? 'camera dives through the knot' : 'knot → type', W - 86, H - 48, C.ink, 'right');
  },
});

// 05 · SPRINGS — a sheet of 3,000 springs in perspective, struck again and again.
let sheet = null;
movement('Springs', 27.6, 34.6, C.blue, C.paper, {
  enter() {
    const cols = 70, rows = 44, pts = [], xyz = new Float32Array(cols * rows * 3);
    for (let r = 0; r < rows; r++) for (let c = 0; c < cols; c++) pts.push({ x: (c / (cols - 1) - 0.5) * 2200, z: (r / (rows - 1) - 0.5) * 1500, h: 0 });
    sheet = { cols, rows, pts, xyz };
    camReset(950);
    cam.rotation.x = 36;
    animate(cam.rotation, { y: [-18, 24] }, { duration: 7, ease: 'inOutSine' });
    // Each strike sends a ring outward; every dot's height is anikit's spring
    // easing evaluated at its own delay (distance / wave speed).
    sheet.hits = [[0, 0, 0.2], [-600, 300, 1.0], [520, -260, 1.6], [0, 0, 2.5], [-300, -380, 3.1], [700, 360, 3.6], [0, 0, 4.4], [200, 120, 5.1]]
      .map(([x, z, at]) => ({ x, z, at, d: pts.map((p) => Math.hypot(p.x - x, p.z - z)) }));
  },
  draw(t, lt) {
    fill(C.blue);
    const { cols, rows, pts, xyz } = sheet;
    for (let i = 0; i < pts.length; i++) {
      let h = 0;
      for (const hit of sheet.hits) {
        const tau = lt - hit.at - hit.d[i] / 1500;
        if (tau <= 0 || tau > 1.8) continue;
        const rise = ease('outCubic', Math.min(1, tau / 0.16)); // soft wavefront
        h += -320 * Math.exp(-hit.d[i] / 650) * rise * (1 - ease('spring(bounce: 0.62, duration: 1.5)', tau / 1.8));
      }
      pts[i].h = h;
      xyz[i * 3] = pts[i].x; xyz[i * 3 + 1] = -h; xyz[i * 3 + 2] = pts[i].z;
    }
    const p = cam.project(xyz, { sort: false });
    const X = (i) => p.data[i * 4], Y = (i) => p.data[i * 4 + 1];
    ctx.lineWidth = 1.5;
    ctx.strokeStyle = 'rgba(236,230,218,0.55)';
    ctx.beginPath();
    for (let r = 0; r < rows; r++) for (let c = 0; c < cols; c++) { const i = r * cols + c; if (c === 0) ctx.moveTo(X(i), Y(i)); else ctx.lineTo(X(i), Y(i)); }
    for (let c = 0; c < cols; c++) for (let r = 0; r < rows; r++) { const i = r * cols + c; if (r === 0) ctx.moveTo(X(i), Y(i)); else ctx.lineTo(X(i), Y(i)); }
    ctx.stroke();
    for (let i = 0; i < pts.length; i++) {
      const sc = p.data[i * 4 + 3];
      if (sc <= 0) continue;
      const lift = clamp(-pts[i].h / 140);
      ctx.fillStyle = lift > 0.2 ? C.lime : C.paper;
      const r = (1.8 + lift * 3.2) * sc;
      ctx.fillRect(X(i) - r, Y(i) - r, r * 2, r * 2);
    }
    rise('Springs keep their velocity.', 86, 190, 72, C.paper, 28.0, t, { stagger: 0.022 });
    label(`${pts.length.toLocaleString('en-US')} springs · retargeted mid-flight · projected in webassembly`, 86, H - 48, C.paper);
  },
});

// 06 · EASE — curves plotted live as the pens move.
const sets = [
  ['outExpo', 'spring(bounce: 0.55)', 'outBounce', 'elastic.out(1, 0.3)', 'steps(8)', 'rough(16, 1.4, 7, outQuad)'],
  ['inOutBack', 'custom(M0,0 C0.2,1.4 0.5,-0.4 1,1)', 'anticipate', 'slowmo(0.6, 0.8)', 'blend(inQuad, outBounce, 0.5)', 'mirror(inOutSine)'],
];
movement('Ease', 34.6, 40.6, C.paper, C.ink, {
  enter() {},
  draw(t, lt) {
    fill(C.paper);
    big('Ease', 74, 330, 330, C.ink);
    label('css · penner · gsap · springs · svg curves · functions', 86, 384, C.ink);
    const set = sets[lt < 3 ? 0 : 1], lt2 = lt < 3 ? lt : lt - 3;
    const x0 = 860, x1 = W - 90, top = 110, lane = 155;
    for (let k = 0; k < set.length; k++) {
      const yb = top + (k + 1) * lane, amp = 95;
      const p = clamp((lt2 - 0.15 - k * 0.08) / 1.7);
      ctx.strokeStyle = 'rgba(12,12,12,0.16)'; ctx.lineWidth = 1;
      ctx.beginPath(); ctx.moveTo(x0, yb); ctx.lineTo(x1, yb); ctx.moveTo(x0, yb - amp); ctx.lineTo(x1, yb - amp); ctx.stroke();
      ctx.strokeStyle = C.red; ctx.lineWidth = 4.5; ctx.lineJoin = 'round';
      ctx.beginPath();
      const steps = Math.floor(p * 180);
      for (let j = 0; j <= steps; j++) { const u = j / 180, x = x0 + (x1 - x0) * u, y = yb - ease(set[k], u) * amp; if (j === 0) ctx.moveTo(x, y); else ctx.lineTo(x, y); }
      ctx.stroke();
      ctx.fillStyle = C.ink;
      ctx.beginPath(); ctx.arc(x0 + (x1 - x0) * p, yb - ease(set[k], p) * amp, 10, 0, TAU); ctx.fill();
      label(set[k], x0 - 26, yb - amp / 2 + 6, C.ink, 'right', 16);
    }
  },
});

// 07 · TUNNEL — 1,400 cards, projected and depth-sorted in WebAssembly, flown through.
let tunnel = null;
movement('Tunnel', 40.6, 47.0, C.lime, C.ink, {
  enter() {
    const n = 1400, xyz = new Float32Array(n * 3), meta = [];
    for (let i = 0; i < n; i++) {
      const a = i * 0.29, r = 380 + (i % 4) * 70;
      xyz[i * 3] = Math.cos(a) * r; xyz[i * 3 + 1] = Math.sin(a) * r; xyz[i * 3 + 2] = -i * 7.2;
      meta.push({ a, red: i % 9 === 0 });
    }
    tunnel = { n, xyz, meta, roll: { z: 0 } };
    camReset(1400);
    animate(cam.camera, { z: [1400, -8600] }, { duration: 6.2, ease: 'inOutCubic', delay: 0.1 });
    animate(cam.target, { z: [400, -9600] }, { duration: 6.2, ease: 'inOutCubic', delay: 0.1 });
    animate(cam.rotation, { z: [0, 150] }, { duration: 6.2, ease: 'inOutCubic', delay: 0.1 });
  },
  draw(t, lt) {
    fill(C.lime);
    const p = cam.project(tunnel.xyz);
    const roll = (cam.rotation.z * Math.PI) / 180;
    for (let k = 0; k < p.count; k++) {
      const i = p.order[k], sc = p.data[i * 4 + 3];
      if (sc <= 0) continue;
      const d = p.data[i * 4 + 2];
      if (d < 30) continue;
      const w = 64 * sc, h = 90 * sc;
      if (w < 0.6 || w > 2600) continue;
      ctx.save();
      ctx.globalAlpha = clamp(1.15 - d / 6500);
      ctx.translate(p.data[i * 4], p.data[i * 4 + 1]);
      ctx.rotate(tunnel.meta[i].a + roll);
      ctx.fillStyle = tunnel.meta[i].red ? C.red : C.ink;
      ctx.fillRect(-w / 2, -h / 2, w, h);
      ctx.restore();
    }
    big('1,400', 80, H - 118, 190, C.ink);
    label('cards · projected + radix depth-sorted in webassembly · every frame', 86, H - 60, C.ink);
  },
});

// 08 · GALAXY — 160,000 stars; the camera falls from above the disk into its core.
movement('Galaxy', 47.0, 54.0, C.black, C.paper, {
  enter() {
    g.options.mode = 'light'; g.options.background = C.black; g.options.trail = 0.55;
    camReset(2600);
    Object.assign(cam.camera, { y: 1900, z: 2600 });
    cloud.snap(S.origin); cloud.explode(0, 0, 0, 1500);
    cloud.from = S.origin; cloud.to = S.galaxy; cloud.mix = 0;
    animate(cloud, { mix: 1 }, { duration: 1.6, ease: 'outCubic' });
    animate(cam.camera, { y: [1900, 60], z: [2600, 260] }, { duration: 6.4, ease: 'inOutCubic', delay: 0.2 });
    animate(cam.rotation, { y: [0, 90] }, { duration: 7, ease: 'inOutSine' });
  },
  draw(t, lt) {
    particles({ tint: [0.82, 0.88, 1], intensity: 0.4, maxRadius: 7 });
    if (lt > 6.2) { ctx.fillStyle = `rgba(236,230,218,${io(prog(lt, 6.2, 7.0))})`; ctx.fillRect(0, 0, W, H); }
    rise('One engine.', 86, 190, 82, C.paper, 47.8, t);
    label('160,000 stars · a 4-arm spiral · camera dive', 86, H - 48, C.paper);
  },
});

// 09 · SIGNATURE
movement('anikit', 54.0, 60, C.paper, C.ink, {
  enter() {
    g.options.mode = 'ink'; g.options.background = C.paper; g.options.trail = 0.5;
    camReset(1000);
    cloud.snap(S.origin);
    cloud.explode(0, 0, 0, 1900);
    cloud.from = S.origin; cloud.to = S.wordmark; cloud.mix = 0;
    animate(cloud, { mix: 1 }, { duration: 1.7, ease: 'outCubic' });
    animate(cam.rotation, { y: [-14, 0] }, { duration: 3, ease: 'outCubic' });
  },
  draw(t, lt) {
    particles({ intensity: 1.9 });
    rise('Motion,', 80, H - 200, 128, C.ink, 55.0, t);
    rise('compiled.', 80, H - 80, 128, C.red, 55.2, t, { italic: true });
    if (lt > 2.0) {
      ctx.globalAlpha = clamp((lt - 2.0) / 0.6);
      label('npm install anikit', W - 86, H - 152, C.ink, 'right', 22);
      label('pip install anikit', W - 86, H - 118, C.ink, 'right', 22);
      label('rust · webassembly · typescript · python', W - 86, H - 80, C.ink, 'right');
      ctx.globalAlpha = 1;
    }
  },
});

// ---------------------------------------------------------------------------
// Frame loop
// ---------------------------------------------------------------------------
const frames = Math.round(END * FPS);
const [segA, segB, segFile] = SEG ? SEG.split(':') : [Math.round(FROM * FPS), frames, OUT];
const first = +segA, last = +segB, target = segFile;
let ff = null;
if (!PREVIEW) {
  const vf = [];
  if (first === 0) vf.push('fade=t=in:st=0:d=0.25');
  if (last === frames) vf.push(`fade=t=out:st=${((last - first) / FPS - 0.9).toFixed(3)}:d=0.9:color=0xece6da`);
  ff = spawn('ffmpeg', ['-y', '-loglevel', 'error', '-f', 'rawvideo', '-pix_fmt', 'rgba', '-s', `${W}x${H}`, '-r', String(FPS), '-i', '-',
    ...(vf.length ? ['-vf', vf.join(',')] : []), '-c:v', 'libx264', '-preset', 'medium', '-crf', '19', '-pix_fmt', 'yuv420p', '-movflags', '+faststart', target], { stdio: ['pipe', 'inherit', 'inherit'] });
} else mkdirSync(OUT, { recursive: true });
const write = (b) => new Promise((r) => (ff.stdin.write(b) ? r() : ff.stdin.once('drain', r)));

const WIPE = 0.34;
const t0 = Date.now();
let current = null;
for (let f = 0; f < last; f++) {
  const t = f / FPS;
  const m = M.find((x) => t >= x.t0 && t < x.t1) ?? M[M.length - 1];
  if (m !== current) {
    current = m;
    m.enter();
    // Start each movement on a clean light buffer (no trails from the last one).
    const keep = g.options.trail; g.options.trail = 0; g.render(cam, cloud, { exposure: 0 }); g.options.trail = keep;
  }
  await null; // let timelines created this frame autoplay (they start on a microtask)
  engine.tick(1000 / FPS);
  cloud.step((1 / FPS) * clock);
  m.step?.();
  // Slices pre-roll half a second unseen so trails/light buffers match the
  // previous slice exactly at the join.
  const preroll = SEG && f >= first - 30 && f < first;
  const render = (f >= first && (!PREVIEW || f % PREVIEW === 0)) || preroll;
  if (!render) continue;
  try { m.draw.call(m, t, t - m.t0); } catch (e) {
    console.error('crash at', t.toFixed(3), m.name, JSON.stringify({ cam: cam.camera, tgt: cam.target, rot: cam.rotation }), e.message);
    throw e;
  }
  // Hard-edged wipe into the next colour field.
  const i = M.indexOf(m), next = M[i + 1];
  if (next && t > next.t0 - WIPE) {
    const u = io(prog(t, next.t0 - WIPE, next.t0));
    ctx.fillStyle = next.bg; ctx.fillRect(0, 0, W * u, H);
    ctx.fillStyle = next.bg === C.red || m.bg === C.red ? C.ink : C.red; ctx.fillRect(W * u - 16, 0, 16, H);
  }
  if (i > 0 && t < m.t0 + 0.2) {
    const u = io(prog(t, m.t0, m.t0 + 0.2));
    ctx.fillStyle = m.bg; ctx.fillRect(W * u, 0, W * (1 - u), H);
  }
  // Slate
  big('anikit', 80, 84, 34, m.fg);
  label(`${String(i + 1).padStart(2, '0')} / ${String(M.length).padStart(2, '0')} — ${m.name}`, W - 86, 76, m.fg, 'right');
  const s = Math.floor(t), fr = f % FPS;
  label(`00:00:${String(s).padStart(2, '0')}:${String(fr).padStart(2, '0')}`, W - 86, 102, m.fg, 'right', 13);
  if (preroll) continue;
  if (PREVIEW) writeFileSync(join(OUT, `f${String(f).padStart(5, '0')}.jpg`), main.toBuffer('image/jpeg', 85));
  else await write(Buffer.from(ctx.getImageData(0, 0, W, H).data.buffer));
  if (f % 300 === 0) console.log(`${SEG ? `[${(first / FPS).toFixed(0)}s] ` : ''}${t.toFixed(1)}s · ${((Date.now() - t0) / 1000).toFixed(0)}s`);
}
if (ff) { ff.stdin.end(); await new Promise((r) => ff.on('close', r)); }
if (!SEG) console.log('done →', OUT, `${((Date.now() - t0) / 1000).toFixed(0)}s`);
