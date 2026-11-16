import { createFractal, de } from './fractal.js';
// anikit is imported dynamically so the fractal's first frame never waits for it.
const ak = import('../../../packages/anikit/dist/anikit.lean.js');

const $ = (s, r = document) => r.querySelector(s);
const $$ = (s, r = document) => [...r.querySelectorAll(s)];
const params = new URLSearchParams(location.search);
const reduced = matchMedia('(prefers-reduced-motion: reduce)').matches;
const small = matchMedia('(pointer: coarse)').matches || innerWidth < 640;
const TAU = Math.PI * 2;

const q = params.get('q');   // ?q=0.4 pins the fractal's resolution (captures)

// 1 · The dive: a spiral of camera stations closing in on one point of the surface,
// each about 2.4× closer than the last. Found by marching the same estimate in JS.
const sub = (a, b) => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
const add = (a, b) => [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
const mul = (a, s) => [a[0] * s, a[1] * s, a[2] * s];
const len = (a) => Math.hypot(a[0], a[1], a[2]);
const norm = (a) => mul(a, 1 / (len(a) || 1));
const cross = (a, b) => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
const march = (ro, rd) => { let t = 0; for (let i = 0; i < 400 && t < 8; i++) { const d = de(...add(ro, mul(rd, t))); if (d < 1e-6 + t * 1e-5) return t; t += d * 0.9; } return Infinity; };

const START = [0.75, 0.95, 3.1];
const AIM = norm(sub([0.22, 0.42, 0.45], START));
const P = add(START, mul(AIM, march(START, AIM)));     // the point we dive towards
const N = mul(AIM, -1);                                // back out towards the start
const A = norm(cross(N, [0, 1, 0])), B = cross(A, N);
const D0 = len(sub(START, P));
const STATIONS = 9;                                    // hero, 7 chambers, the void
const ratio = 0.42;
const pts = [START];
for (let k = 1; k < STATIONS; k++) {
  const d = D0 * Math.pow(ratio, k);
  let off = 0.55;
  let c;
  // Spiral around the dive line; pull in towards it if the view of P would be blocked.
  for (let tries = 0; tries < 6; tries++, off *= 0.6) {
    const a = k * 1.25;
    c = add(add(P, mul(N, d)), mul(add(mul(A, Math.cos(a)), mul(B, Math.sin(a))), d * off));
    const dir = norm(sub(P, c));
    if (march(c, dir) > len(sub(P, c)) * 0.97 && de(...c) > d * 0.25) break;
  }
  pts.push(c);
}
pts.push(add(P, mul(sub(pts[STATIONS - 1], P), 0.35)));   // a point beyond the last station for look-ahead
// 2 · The fractal draws its first frame now, before anikit has loaded.
const fr = createFractal($('#fractal'), { quality: q ? +q : undefined, initial: { eye: START, forward: norm(add(mul(norm(sub(pts[1], START)), 0.35), mul(norm(sub(P, START)), 0.65))) } });
const { init, engine, animate, timeline, stagger, spring, magnetic, splitText, scroll, smoothScroll, inView, flip, flightPath, world, pointCloud, glow, scene3d, circle, star, rect } = await ak;
const path = flightPath(pts, { lookAhead: 0.4, bank: 0.55, maxBank: 16, bankWindow: 0.9 });

// Arc length of every station (nearest sample on the path).
const stationS = (() => {
  const n = 6000, out = [];
  let j = 0;
  for (const p of pts.slice(0, STATIONS)) {
    let best = Infinity, bs = 0;
    for (let i = j; i <= n; i++) { const s = (i / n) * path.length; const d = len(sub(path.point(s), p)); if (d < best) { best = d; bs = s; j = i; } }
    out.push(bs);
  }
  return out;
})();

// 3 · Scroll → distance along the path. Holds keep the camera nearly still while a demo is up.
const chambers = $$('.chamber');
const voidSec = $('.void');
let keys = [], timing = null, maxScroll = 1;
function layout() {
  const vh = innerHeight;
  maxScroll = Math.max(1, document.documentElement.scrollHeight - vh);
  const top = (el) => el.getBoundingClientRect().top + scrollY;
  keys = [[0, stationS[0] / path.length]];
  chambers.forEach((sec, i) => {
    const t = top(sec), range = sec.offsetHeight - vh, k = i + 1;
    const a = stationS[k], b = stationS[k] + (stationS[k + 1] - stationS[k]) * 0.1;
    keys.push([t + range * 0.36, a / path.length], [t + range * 0.68, b / path.length]);
  });
  const tv = top(voidSec);
  keys.push([tv + voidSec.offsetHeight * 0.5, stationS[STATIONS - 1] / path.length], [maxScroll + 1, (stationS[STATIONS - 1] + 0.0004) / path.length]);
  timing = path.timing(keys);
}

// Pointer parallax on a spring; breathing and drift on the clock.
const par = { x: 0, y: 0 };
const cam = { s: 0 };
let fade = 0;
let zoomText = '';
const zoomEl = $('#zoom');

function steer(timeMs) {
  if (!fr) return;
  const t = timeMs / 1000;
  const y = reduced ? snapScroll() : scrollY;
  const s = Math.min(path.length * 0.999, timing(y));
  cam.s = s;
  const pose = path.at(s, { lookAhead: Math.max(1e-4, len(sub(path.point(s), P)) * 0.5) });
  const eye0 = pose.eye;
  const dist = len(sub(eye0, P));
  // Look mostly at P so the point we dive towards stays centred behind the panels.
  let fwd = norm(add(mul(norm(sub(pose.target, eye0)), 0.35), mul(norm(sub(P, eye0)), 0.65)));
  const roll = ((reduced ? 0 : pose.roll * 0.8) + (reduced ? 0 : Math.sin(t * 0.07) * 2.5)) * (Math.PI / 180);
  let r = norm(cross(fwd, [0, 1, 0])), u = cross(r, fwd);
  const up = add(mul(u, Math.cos(roll)), mul(r, Math.sin(roll)));
  r = norm(cross(fwd, up));
  const S = Math.max(1e-5, dist * 0.6);
  let eye = eye0;
  if (!reduced) {
    eye = add(eye, add(mul(r, (par.x * 0.10 + Math.sin(t * 0.13) * 0.05) * S), mul(up, (-par.y * 0.07 + Math.cos(t * 0.11) * 0.04) * S)));
    fwd = norm(add(fwd, add(mul(r, par.x * 0.012), mul(up, -par.y * 0.010))));
  }
  const breathe = reduced ? 0 : Math.sin((t * TAU) / 26) * 0.42 * Math.min(1, dist / D0);
  const z = D0 / Math.max(dist, 1e-6);
  const zt = z < 10 ? z.toFixed(1) : Math.round(z).toLocaleString('en-US');
  if (zt !== zoomText) { zoomText = zt; zoomEl.textContent = '×' + zt; }
  fr.set({ eye, forward: fwd, up, power: 8 + breathe, fade, focus: len(sub(P, eye)) * 0.9, time: t, depth: Math.log2(D0 / Math.max(dist, 1e-6)) / 10 });
}

// Reduced motion: no flight; the camera sits at the station of the section in view.
function snapScroll() {
  let best = 0;
  for (let i = 1; i < keys.length; i++) if (scrollY >= keys[i][0] - innerHeight * 0.5) best = i;
  return keys[best][0];
}

if (fr) {
  layout();
  steer(0);
  fr.draw();
  fr.start();
}

// ---------------------------------------------------------------- anikit
await init();
if (reduced) { engine.reducedMotion = 'user'; engine.reducedMotionMode = 'fade'; }
addEventListener('resize', () => { layout(); });
addEventListener('load', layout);
engine.onFrame((time) => steer(time));
if (!reduced) smoothScroll({ smooth: small ? 0.5 : 0.75 });

if (!reduced && !small) {
  addEventListener('pointermove', (e) => spring(par, { x: (e.clientX / innerWidth) * 2 - 1, y: (e.clientY / innerHeight) * 2 - 1 }, { stiffness: 18, damping: 9 }));
}

// Hero: letters rise, then the copy leaves as you scroll in.
{
  const { chars } = splitText('#title', { type: 'chars' });
  $('#title').classList.add('split');
  timeline({ defaults: { ease: 'outCubic' } })
    .add(chars, { yPercent: [60, 0], opacity: [0, 1], rotateX: [-50, 0] }, { duration: 1.4, ease: 'outExpo', delay: stagger(0.06) })
    .add('#lede', { opacity: [0, 1], y: [14, 0] }, { duration: 1.2 }, '-=0.9')
    .add('#cue', { opacity: [0, 1] }, { duration: 1.2 }, '-=0.6');
  scroll(animate('.hero-in', { opacity: [1, 1, 0], scale: [1, 1, 0.94], y: [0, 0, -40] }, { autoplay: false, ease: 'linear' }), { target: $('.hero'), offset: ['start start', 'end start'] });
}

// Every chamber panel eases in over ~50vh, holds, and eases out over ~40vh.
chambers.forEach((sec) => {
  const panel = $('.panel', sec);
  const tl = timeline({ autoplay: false, defaults: { ease: 'inOutSine' } })
    .add(panel, { opacity: [0, 1], y: [70, 0], scale: [0.965, 1] }, { duration: 0.36 }, 0)
    .add(panel, { opacity: [1, 0], y: [0, -60], scale: [1, 1.02] }, { duration: 0.3 }, 0.7);
  scroll(tl, { target: sec, offset: ['start start', 'end end'] });
});
{
  const tl = timeline({ autoplay: false, defaults: { ease: 'inOutSine' } })
    .add('#install-core', { opacity: [0, 1], y: [50, 0] }, { duration: 0.5 }, 0.3)
    .add({}, { duration: 0.2 }, 0.8);
  scroll(tl, { target: voidSec, offset: ['start end', 'end end'] });
  // The fractal dissolves into the void.
  scroll((p) => { fade = Math.min(1, Math.max(0, (p - 0.05) / 0.55)); zoomEl.style.opacity = String(1 - fade); }, { target: voidSec, offset: ['start end', 'end end'] });
}
// Progress rail.
scroll((p) => {
  $('#rail-bar').style.transform = `scaleY(${p})`;
  const y = scrollY + innerHeight * 0.5;
  let k = 0;
  chambers.forEach((s, i) => { if (y > s.offsetTop) k = i + 1; });
  $('#rail-n').textContent = String(k).padStart(2, '0');
});

// Run a demo only while its chamber is near the viewport.
function whileNear(sec, fn) {
  let off = 0, last = 0, on = false;
  const tick = (now) => { off = 0; const dt = last ? Math.min(50, now - last) : 16.7; last = now; fn(dt / 1000); };
  inView(sec, () => {
    on = true; last = 0;
    const stop = engine.onFrame((t) => tick(t));
    return () => { on = false; stop?.(); };
  }, { margin: '20% 0px 20% 0px' });
  return () => on;
}

// 01 · Physics
{
  const stage = $('#world-stage'), canvas = $('#world-canvas'), ctx = canvas.getContext('2d');
  const w = world({ container: stage, gravity: [0, 1100], restitution: 0.35, friction: 0.04, substeps: 4 });
  const pal = ['#ffcf8a', '#ff8a6b', '#f2efe9', '#8fb4ff', '#c9a6ff'];
  let cols = [];
  const fill = () => {
    const W = stage.clientWidth, H = stage.clientHeight;
    const n = small ? 140 : 230;
    const bodies = Array.from({ length: n }, (_, i) => ({ x: 20 + Math.random() * (W - 40), y: -Math.random() * H * 1.4, r: 5 + Math.pow(Math.random(), 2) * 13 }));
    w.addMany(bodies);
    cols = bodies.map((_, i) => pal[i % pal.length]);
  };
  const fit = () => { const d = Math.min(devicePixelRatio, 2); canvas.width = stage.clientWidth * d; canvas.height = stage.clientHeight * d; ctx.setTransform(d, 0, 0, d, 0, 0); w.refreshBounds(); };
  fit(); fill();
  addEventListener('resize', fit);
  w.pointer({ mode: 'repel', radius: 90 });
  w.dragging();
  w.pause();
  let clock = 0;
  inView($('#physics'), () => { w.play(); return () => w.pause(); }, { margin: '20% 0px 20% 0px' });
  whileNear($('#physics'), (dt) => {
    clock += dt;
    // Every few seconds a pulse from below throws a handful of bodies back up.
    if (clock > 3.2 && !reduced) {
      clock = 0;
      const W = stage.clientWidth, H = stage.clientHeight;
      w.impulse(W * (0.2 + Math.random() * 0.6), H + 10, W * 0.28, 1500);
    }
    ctx.clearRect(0, 0, stage.clientWidth, stage.clientHeight);
    w.draw(ctx, { color: (i) => cols[i] || '#fff', clear: false });
  });
}

// 02 · 100k particles
{
  const NP = small ? 60_000 : 100_000;
  if (NP !== 100_000) { $('#cloud h2').textContent = `${NP.toLocaleString('en-US')} particles`; $('#cloud .snippet').textContent = `pointCloud(${NP.toLocaleString('en-US').replaceAll(',', '_')}).morph(1, { duration: 2.4 })`; }
  let rs = 7; const rnd = () => ((rs = (rs * 16807) % 2147483647) / 2147483647);
  const shapes = [
    (i, a, o) => { const y = 1 - (2 * (i + 0.5)) / NP, r = Math.sqrt(1 - y * y), t = i * 2.39996; const R = 230 + (rnd() - 0.5) * 8; a[o] = Math.cos(t) * r * R; a[o + 1] = y * R; a[o + 2] = Math.sin(t) * r * R; },
    (i, a, o) => { const t = (i / NP) * TAU, r = 150 + 68 * Math.cos(3 * t), ph = rnd() * TAU, rr = 24 * Math.sqrt(rnd()); a[o] = r * Math.cos(2 * t) + Math.cos(ph) * rr; a[o + 1] = r * Math.sin(2 * t) + Math.sin(ph) * rr; a[o + 2] = -68 * Math.sin(3 * t) + Math.cos(ph * 1.7) * rr; },
    (i, a, o) => { const arm = i % 3, f = rnd(), r = 20 + Math.pow(f, 0.7) * 300, th = r * 0.018 + (arm * TAU) / 3 + (rnd() - 0.5) * 0.5 * (1 - f * 0.5); a[o] = Math.cos(th) * r; a[o + 2] = Math.sin(th) * r; a[o + 1] = (rnd() - 0.5) * 30 * (1 - f); },
  ];
  let cloud = null, g = null;
  const jobs = [() => { cloud = pointCloud(NP, { autoStep: false, stiffness: 30, damping: 7, spread: 0.8, noise: 1.4, swirl: 60, frequency: 0.012 }); return true; }];
  shapes.forEach((fn, k) => {
    let buf = null, i = 0;
    jobs.push((dl) => { buf ??= new Float32Array(NP * 3); const end = performance.now() + Math.min(6, Math.max(2, dl.timeRemaining() - 1)); while (i < NP && performance.now() < end) { const stop = Math.min(NP, i + 2000); for (; i < stop; i++) fn(i, buf, i * 3); } if (i < NP) return false; cloud.shape(k, buf); if (k === 0) cloud.snap(0); return true; });
  });
  jobs.push(() => { g = glow('#cloud-canvas', { resolution: small ? 0.7 : 0.8, background: '#05060a', trail: 0.35, budget: 4 }); return true; });
  jobs.push(() => { g.colors(NP, (j) => { const u = j / NP, k = rnd(), sp = k < 0.04 ? 1.8 : 0.7 + k * 0.4; return [(0.85 + 0.15 * Math.cos(TAU * u)) * sp, (0.55 + 0.25 * Math.cos(TAU * (u - 0.2))) * sp, (0.45 + 0.4 * Math.cos(TAU * (u - 0.55))) * sp]; }); return true; });
  const ready = new Promise((res) => {
    const idle = window.requestIdleCallback || ((f) => setTimeout(() => f({ timeRemaining: () => 8 }), 30));
    const run = (dl) => { if (!jobs.length) return res(); if (jobs[0](dl)) jobs.shift(); idle(run, { timeout: 400 }); };
    idle(run, { timeout: 400 });
  });
  ready.then(() => {
    const canvas = $('#cloud-canvas');
    const scene = scene3d({ fov: 40, camera: { z: 1000 }, near: 10, far: 5000 });
    const fit = () => { const w = canvas.clientWidth, h = canvas.clientHeight; g.resize(w, h); scene.resize(w, h, 1); };
    fit(); addEventListener('resize', fit);
    let next = 1, timer = 0, rot = 0;
    const ptr = { x: 0, y: 0, on: false };
    canvas.addEventListener('pointermove', (e) => { const r = canvas.getBoundingClientRect(); ptr.x = e.clientX - r.left; ptr.y = e.clientY - r.top; ptr.on = true; });
    canvas.addEventListener('pointerleave', () => (ptr.on = false));
    whileNear($('#cloud'), (dt) => {
      timer += dt;
      cloud.step(dt);
      if (!reduced) rot += dt;
      if (timer > 5.2 && !reduced) { timer = 0; cloud.morph(next, { duration: 2.4, ease: 'inOutCubic' }); next = (next + 1) % 3; }
      scene.camera.x = Math.sin(rot * 0.22) * 1000; scene.camera.z = Math.cos(rot * 0.22) * 1000; scene.camera.y = 260;
      g.render(scene, cloud, { size: 1.1, intensity: small ? 0.5 : 0.32, fog: [600, 2200], ...(ptr.on ? { lens: { x: ptr.x, y: ptr.y } } : {}) });
    });
  });
}

// 03 · Split text: three lines in turn, each a timeline.
{
  const lines = ['Every letter starts a little later than the last.', 'Steps can follow, overlap or wait for each other.', 'Split by line, by word or by character.'];
  const el = $('#reveal');
  let k = 0, playing = false, current = null;
  const play = () => {
    el.textContent = lines[k % lines.length]; k++;
    const { chars } = splitText(el, { type: 'words,chars' });
    current = timeline({ onComplete: () => (playing ? play() : null) })
      .add(chars, { yPercent: [110, 0], rotate: [8, 0], opacity: [0, 1] }, { duration: 0.9, ease: 'spring(bounce: 0.2, duration: 0.9)', delay: stagger(0.018) })
      .add(chars, { yPercent: [0, -110], opacity: [1, 0] }, { duration: 0.6, ease: 'inCubic', delay: stagger(0.01) }, '+=2.2');
  };
  inView($('#text .stage'), () => { playing = true; if (!current) play(); else current.play(); return () => { playing = false; current?.pause(); }; });
}

// 04 · Springs: three dots chase one target with different settings; the pointer takes over.
{
  const stage = $('#spring-stage'), tgt = $('#spring-target');
  const dots = $$('.dot', stage);
  const cfg = [{ stiffness: 260, damping: 16 }, { stiffness: 180, damping: 7 }, { stiffness: 40, damping: 10 }];
  let pointer = false, i = 0, clock = 0;
  const go = (x, y) => { spring(tgt, { x, y }, { stiffness: 900, damping: 60 }); dots.forEach((d, j) => spring(d, { x, y }, cfg[j])); };
  const hop = () => {
    const W = stage.clientWidth, H = stage.clientHeight;
    const a = (i++ * TAU * 2) / 5 - Math.PI / 2;  // pentagram
    go(W / 2 + Math.cos(a) * W * 0.3, H / 2 + Math.sin(a) * H * 0.32);
  };
  const W0 = stage.clientWidth, H0 = stage.clientHeight;
  animate([tgt, ...dots], { x: W0 / 2, y: H0 / 2 }, { duration: 0 });
  stage.addEventListener('pointermove', (e) => { const r = stage.getBoundingClientRect(); pointer = true; go(e.clientX - r.left, e.clientY - r.top); });
  stage.addEventListener('pointerleave', () => { pointer = false; clock = 1; });
  whileNear($('#springs'), (dt) => { clock += dt; if (!pointer && clock > 1.25) { clock = 0; hop(); } });
  magnetic('.pill.mag', { strength: 0.4, radius: 90 });
  // Hovering a label pulses its dot.
  $$('.pill.mag').forEach((p) => p.addEventListener('pointerenter', () => animate(dots[+p.dataset.k], { scale: [1, 1.8, 1] }, { duration: 0.6, ease: 'inOutSine' })));
}

// 05 · SVG morph scrubbed by scroll.
{
  const shapes = [circle(200, 150, 92), star(200, 150, 5, 118, 50, -90), rect(110, 70, 180, 160, 28), star(200, 150, 12, 112, 82), circle(200, 150, 92)];
  $('#morph').setAttribute('d', shapes[0]);
  $('#morph-echo').setAttribute('d', shapes[0]);
  scroll(animate('#morph', { d: shapes }, { autoplay: false, ease: 'linear' }), { target: $('#svg'), offset: ['start start', 'end end'], smooth: 0.15 });
  scroll(animate('#morph-echo', { d: shapes.slice(1).concat(shapes[1]) }, { autoplay: false, ease: 'linear' }), { target: $('#svg'), offset: ['start start', 'end end'], smooth: 0.5 });
  if (!reduced) animate('#morph-g', { rotate: [0, 360] }, { duration: 48, repeat: -1, ease: 'linear' });
}

// 06 · FLIP grid: shuffles and re-flows on its own.
{
  const grid = $('#tiles');
  const cols = ['#ffcf8a', '#ff8a6b', '#ff6b9a', '#c9a6ff', '#8fb4ff', '#7fe0c8'];
  for (let i = 0; i < 12; i++) { const t = document.createElement('div'); t.className = 'tile'; t.style.setProperty('--c', cols[i % cols.length]); t.textContent = String(i + 1).padStart(2, '0'); grid.append(t); }
  const layouts = ['', 'g4', 'g3'];
  let step = 0, clock = 1.2;
  whileNear($('#layout'), (dt) => {
    clock += dt;
    if (clock < 2.6 || reduced) return;
    clock = 0;
    step++;
    flip('#tiles .tile', () => {
      if (step % 2) { const kids = [...grid.children]; for (let i = kids.length - 1; i > 0; i--) { const j = Math.floor(Math.random() * (i + 1)); [kids[i], kids[j]] = [kids[j], kids[i]]; } grid.append(...kids); }
      else { grid.className = 'tiles ' + layouts[(step / 2) % 3]; }
    }, { duration: 0.9, ease: 'spring(bounce: 0.2, duration: 0.9)' });
  });
}

// 07 · Benchmarks: counters and bars when the panel arrives.
{
  const counts = $$('#numbers .n-count'), bars = $$('#numbers .n-row u');
  counts.forEach((c) => (c.textContent = (0).toFixed(+c.dataset.dec)));
  if (!reduced) bars.forEach((b) => (b.style.transform = 'scaleX(0)'));
  inView($('#numbers .n-chart'), () => {
    counts.forEach((c, i) => { const o = { v: 0 }, dec = +c.dataset.dec; animate(o, { v: +c.dataset.to }, { duration: 1.6, ease: 'outExpo', delay: i * 0.08, onUpdate: () => (c.textContent = o.v.toFixed(dec)) }); });
    if (!reduced) animate(bars, { scaleX: [0, 1] }, { duration: 1.4, ease: 'outExpo', delay: stagger(0.12, { start: 0.2 }) });
  }, { once: true, amount: 0.6 });
}

// Ending: copy button.
$('#copy').addEventListener('click', async (e) => {
  const b = e.currentTarget;
  try { await navigator.clipboard.writeText($('#install-cmd').textContent); b.textContent = 'Copied'; } catch { b.textContent = 'Select'; }
  animate(b, { scale: [0.92, 1] }, { ease: 'spring(bounce: 0.5, duration: 0.5)' });
  setTimeout(() => (b.textContent = 'Copy'), 1600);
});

layout();
