// Anikit drives everything that isn't the planet: the smoothed scroll, the orbit
// camera (a critically damped spring on scroll progress), pointer tilt, and every
// piece of UI motion. The demos come at moments along the orbit, night into dawn.
import {
  ready, engine, animate, set, timeline, stagger, splitText, spring, magnetic, scroll, smoothScroll,
  motionValue, inView, world, pointCloud, glow, scene3d, flip, effects, circle, star, rect, pathThrough, noise2,
} from '../../../packages/anikit/dist/anikit.lean.js';
import { planet } from './planet.js';

const $ = (s, r = document) => r.querySelector(s);
const $$ = (s, r = document) => Array.from(r.querySelectorAll(s));
const reduced = matchMedia('(prefers-reduced-motion: reduce)').matches;
const fine = matchMedia('(hover: hover) and (pointer: fine)').matches;
const small = innerWidth < 760;
const clamp = (v, a = 0, b = 1) => (v < a ? a : v > b ? b : v);
const ease = (x) => (x < 0.5 ? 4 * x * x * x : 1 - (-2 * x + 2) ** 3 / 2);

engine.reducedMotion = 'user';
engine.reducedMotionMode = 'fade';
await ready;

// ---------------------------------------------------------------------------
// Scroll and camera
// ---------------------------------------------------------------------------
if (!reduced) smoothScroll({ smooth: 0.9 });
const orbit = motionValue(0);
const tiltX = motionValue(0), tiltY = motionValue(0);
const maxScroll = () => Math.max(1, document.documentElement.scrollHeight - innerHeight);
const follow = () => {
  const p = clamp(scrollY / maxScroll());
  // Critically damped: damping = 2·√stiffness. No overshoot, no lag that feels like drag.
  if (reduced) orbit.set(p); else orbit.spring(p, { stiffness: 30, damping: 2 * Math.sqrt(30) });
};
addEventListener('scroll', follow, { passive: true });
addEventListener('resize', follow);
follow();
if (fine && !reduced) {
  addEventListener('pointermove', (e) => {
    tiltX.spring((e.clientX / innerWidth - 0.5) * 0.05, { stiffness: 40, damping: 2 * Math.sqrt(40) });
    tiltY.spring(-(e.clientY / innerHeight - 0.5) * 0.03, { stiffness: 40, damping: 2 * Math.sqrt(40) });
  });
}

let t = 30, hidden = document.hidden;
document.addEventListener('visibilitychange', () => (hidden = document.hidden));
let lastKey = '';
engine.onFrame((_ms, dt) => {
  if (!planet || hidden || window.__freeze) return;
  if (!reduced) t += Math.min(dt, 50) / 1000;
  const s = planet.state;
  s.u = orbit.get(); s.tiltX = tiltX.get(); s.tiltY = tiltY.get(); s.time = t;
  // Under reduced motion the planet only redraws when the camera moves.
  const key = reduced ? s.u.toFixed(4) : '';
  if (reduced && key === lastKey) return;
  lastKey = key;
  planet.render();
});

// ---------------------------------------------------------------------------
// Hero: the wordmark rises in, then everything fades as the orbit begins
// ---------------------------------------------------------------------------
const mark = $('#mark');
const letters = splitText(mark, { type: 'words,chars', charClass: 'ch', wordClass: 'gw' }).chars;
set(letters, { yPercent: 105 });
set(mark, { opacity: 1 });
timeline({ defaults: { ease: 'outExpo', duration: 1.6 } })
  .add(letters, { yPercent: [105, 0], opacity: [0, 1] }, { delay: stagger(0.05) }, 0.15)
  .add('#lede', { opacity: [0, 1], y: [10, 0] }, { duration: 1.4, ease: 'outCubic' }, 0.7)
  .add('.nav', { opacity: [0, 1] }, { duration: 1.2 }, 0.9);
scroll((p) => set('.hero', { opacity: 1 - ease(clamp(p * 1.6)), y: -p * 80 }), { target: $('.hero'), offset: ['start start', 'end start'] });

// ---------------------------------------------------------------------------
// Each stop: the panel rises in over ~45vh, holds, and drifts out over ~45vh
// ---------------------------------------------------------------------------
const shade = $('.shade');
const shadeFor = new Map();
for (const sec of $$('.stop')) {
  const panel = $('.panel', sec);
  scroll((p) => {
    // p runs 0 → 1 while the section crosses the viewport (start end → end start).
    const H = sec.offsetHeight, vh = innerHeight, span = H + vh;
    const y = p * span; // px scrolled since the section's top hit the bottom of the viewport
    // Sticky from y = vh to y = H: rise in over the last 50vh before it, leave over 45vh before it unsticks.
    const a = ease(clamp((y - vh * 0.55) / (vh * 0.5)));
    const b = ease(clamp((y - (H - vh * 0.5)) / (vh * 0.45)));
    const o = a * (1 - b);
    set(panel, { opacity: o, y: (1 - a) * 60 - b * 60, scale: 0.97 + 0.03 * o });
    shadeFor.set(sec, o);
    let m = 0;
    for (const v of shadeFor.values()) m = Math.max(m, v);
    set(shade, { opacity: m });
  }, { target: sec, offset: ['start end', 'end start'] });
}
// The ending: the install line arrives as the sun clears the limb.
scroll((p) => {
  const o = ease(clamp((p - 0.45) / 0.3));
  set('.install-core', { opacity: o, y: (1 - o) * 30 });
}, { target: $('#install'), offset: ['start end', 'end end'] });

// Per-frame work only while a demo is on screen.
function whileVisible(el, fn) {
  inView(el, () => {
    const off = engine.onFrame((_t, dt) => fn(Math.min(dt, 50)));
    return () => off();
  }, { amount: 0 });
}
const soon = (el, fn) => inView(el, () => { fn(); }, { amount: 0, once: true, margin: '0px 0px 80% 0px' });

// ---------------------------------------------------------------------------
// 1 · Split text: a looping timeline of characters, then words
// ---------------------------------------------------------------------------
soon('#text', () => {
  const el = $('#reveal');
  const chars = splitText(el, { type: 'words,chars', charClass: 'ch', wordClass: 'gw' }).chars;
  set(chars, { yPercent: 110, opacity: 0 });
  const eases = ['spring(bounce: 0.35, duration: 0.9)', 'outExpo', 'outBack'];
  let k = 0, tl = null;
  const play = () => {
    tl?.kill();
    tl = timeline({ onComplete: () => { k = (k + 1) % eases.length; play(); } })
      .add(chars, { yPercent: [110, 0], rotate: [8, 0], opacity: [0, 1] }, { duration: 1, ease: eases[k], delay: stagger(0.018) })
      .add(chars, { yPercent: [0, -110], opacity: [1, 0] }, { duration: 0.6, ease: 'inCubic', delay: stagger(0.01, { from: 'last' }) }, '+=2.2');
  };
  if (reduced) set(chars, { yPercent: 0, opacity: 1 });
  else inView(el, () => { play(); return () => { tl?.pause(); }; }, { amount: 0.4 });
});

// ---------------------------------------------------------------------------
// 2 · Physics: city lights that fall, pile up and get thrown
// ---------------------------------------------------------------------------
soon('#physics', () => {
  const canvas = $('#world-canvas');
  const ctx = canvas.getContext('2d');
  const dpr = Math.min(devicePixelRatio || 1, 2);
  let W = 0, H = 0, w = null;
  const N = small ? 90 : 150;
  const palette = ['#ffcf8a', '#ffb066', '#ff8f4d', '#ffe3b8', '#9fd2ff'];
  const build = () => {
    W = canvas.clientWidth; H = canvas.clientHeight;
    canvas.width = W * dpr; canvas.height = H * dpr;
    w?.destroy();
    w = world({ container: canvas, bounds: [0, -H * 2, W, H], gravity: [0, 1400], substeps: 4, restitution: 0.35, friction: 0.08 });
    const s = small ? 7 : 10;
    w.addMany(Array.from({ length: N }, (_, i) => ({ x: 20 + ((i * 37) % (W - 40)), y: -((i * 13) % (H * 1.8)) - 10, r: s * (0.6 + ((i * 7919) % 100) / 110) })));
    w.dragging({ hitSlop: 10 });
  };
  build();
  addEventListener('resize', build);
  let clock = 0, side = 0;
  whileVisible(canvas, (dt) => {
    clock += dt / 1000;
    // Without interaction a slow pulse throws the pile up, alternating sides.
    if (clock > 3.6 && !reduced) { clock = 0; side = 1 - side; w.impulse(W * (0.3 + side * 0.4), H, W * 0.28, 2100); }
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, W, H);
    for (let i = 0; i < N; i++) {
      ctx.fillStyle = palette[i % palette.length];
      ctx.beginPath();
      ctx.arc(w.x(i), w.y(i), w.radius(i), 0, Math.PI * 2);
      ctx.fill();
    }
  });
});

// ---------------------------------------------------------------------------
// 3 · 100,000 particles: built in idle slices long before the stop arrives
// ---------------------------------------------------------------------------
{
  const N = reduced ? 30_000 : small ? 40_000 : 100_000;
  let seed = 3;
  const rnd = () => ((seed = (seed * 1664525 + 1013904223) >>> 0) / 4294967296);
  const TAU = Math.PI * 2;
  const shapes = [
    (i, a, o) => { const u = rnd() * 2 - 1, th = rnd() * TAU, r = 230 * Math.cbrt(0.85 + rnd() * 0.15), q = Math.sqrt(1 - u * u); a[o] = Math.cos(th) * q * r; a[o + 1] = u * r; a[o + 2] = Math.sin(th) * q * r; },
    // An auroral ring: a thin curtain, tall and rippled.
    (i, a, o) => { const th = rnd() * TAU, h = rnd(), R = 260 + Math.sin(th * 5) * 26; a[o] = Math.cos(th) * R; a[o + 1] = (h - 0.5) * 180 * (0.6 + 0.4 * Math.sin(th * 3)); a[o + 2] = Math.sin(th) * R; },
    (i, a, o) => {
      const t = (i / N) * TAU, r = 150 + 68 * Math.cos(3 * t), ph = rnd() * TAU, rr = 26 * Math.sqrt(rnd());
      a[o] = r * Math.cos(2 * t) + Math.cos(ph) * rr; a[o + 1] = r * Math.sin(2 * t) + Math.sin(ph) * rr; a[o + 2] = -68 * Math.sin(3 * t) + Math.cos(ph * 1.7) * rr;
    },
  ];
  let cloud = null, g = null;
  const jobs = [];
  jobs.push(() => { cloud = pointCloud(N, { autoStep: false, stiffness: 30, damping: 7, spread: 0.8, noise: 1.6, swirl: 60, frequency: 0.012 }); return true; });
  shapes.forEach((fn, k) => {
    let buf = null, i = 0;
    jobs.push((deadline) => {
      buf ??= new Float32Array(N * 3);
      const end = performance.now() + Math.min(6, Math.max(2, deadline.timeRemaining() - 1));
      while (i < N && performance.now() < end) { const stop = Math.min(N, i + 2000); for (; i < stop; i++) fn(i, buf, i * 3); }
      if (i < N) return false;
      cloud.shape(k, buf);
      if (k === 0) cloud.snap(0);
      return true;
    });
  });
  jobs.push(() => { g = glow('#cloud-canvas', { resolution: small ? 0.7 : 0.75, background: '#000000', trail: 0.4, budget: 5 }); return true; });
  {
    let i = 0, cols = null;
    jobs.push((deadline) => {
      cols ??= new Float32Array(N * 3);
      const end = performance.now() + Math.min(6, Math.max(2, deadline.timeRemaining() - 1));
      while (i < N && performance.now() < end) {
        const stop = Math.min(N, i + 4000);
        for (; i < stop; i++) {
          // Aurora greens shading into violet, with a few hot white sparks.
          const u = rnd(), k = rnd(), sp = k < 0.03 ? 2 : 0.7 + k * 0.4;
          cols[i * 3] = (0.2 + 0.6 * u * u) * sp; cols[i * 3 + 1] = (0.95 - 0.6 * u) * sp; cols[i * 3 + 2] = (0.5 + 0.4 * u) * sp;
        }
      }
      if (i < N) return false;
      g.colors(N, (j) => [cols[j * 3], cols[j * 3 + 1], cols[j * 3 + 2]]);
      return true;
    });
  }
  const cloudReady = new Promise((resolve) => {
    const idle = window.requestIdleCallback || ((f) => setTimeout(() => f({ timeRemaining: () => 8 }), 30));
    const run = (deadline) => { if (!jobs.length) return resolve(); if (jobs[0](deadline)) jobs.shift(); idle(run, { timeout: 400 }); };
    idle(run, { timeout: 400 });
  });
  soon('#cloud', () => cloudReady.then(() => {
    $('#cloud-n').textContent = N.toLocaleString('en-US');
    $('#cloud-code').textContent = `pointCloud(${N.toLocaleString('en-US').replaceAll(',', '_')}).morph(1, { duration: 2 })`;
    const canvas = $('#cloud-canvas');
    const scene = scene3d({ fov: 40, camera: { z: 1000 }, near: 10, far: 5000 });
    const fit = () => { const w = canvas.clientWidth, h = canvas.clientHeight; g.resize(w, h); scene.resize(w, h, 1); };
    fit();
    addEventListener('resize', fit);
    let next = 1, timer = 0, rot = 0;
    const ptr = { x: 0, y: 0, on: false };
    canvas.addEventListener('pointermove', (e) => { const r = canvas.getBoundingClientRect(); ptr.x = e.clientX - r.left; ptr.y = e.clientY - r.top; ptr.on = true; });
    canvas.addEventListener('pointerleave', () => (ptr.on = false));
    canvas.addEventListener('click', () => { cloud.explode(0, 0, 0, 700); cloud.morph(next, { duration: 1.6, ease: 'inOutCubic' }); next = (next + 1) % 3; timer = 0; });
    whileVisible(canvas, (dt) => {
      timer += dt / 1000;
      cloud.step(dt / 1000);
      if (!reduced) rot += dt / 1000;
      if (timer > 4.5 && !reduced) { timer = 0; cloud.morph(next, { duration: 2.2, ease: 'inOutCubic' }); next = (next + 1) % 3; }
      scene.camera.x = Math.sin(rot * 0.22) * 1000; scene.camera.z = Math.cos(rot * 0.22) * 1000; scene.camera.y = 260;
      g.render(scene, cloud, { size: 1.1, intensity: small ? 0.5 : 0.34, fog: [600, 2200], ...(ptr.on ? { lens: { x: ptr.x, y: ptr.y } } : {}) });
    });
  }));
}

// ---------------------------------------------------------------------------
// 4 · Springs and magnetic buttons
// ---------------------------------------------------------------------------
soon('#springs', () => {
  const box = $('#spring-box');
  const presets = [{ stiffness: 260, damping: 16 }, { stiffness: 180, damping: 7 }, { stiffness: 40, damping: 10 }];
  let cfg = presets[1];
  const n = 12;
  const dots = Array.from({ length: n }, (_, i) => {
    const d = document.createElement('i');
    d.className = 'dot';
    d.style.opacity = String(1 - i * 0.07);
    d.style.scale = String(1 - i * 0.05);
    box.appendChild(d);
    return d;
  });
  const pos = dots.map(() => ({ x: 0, y: 0 }));
  const aim = { x: 0, y: 0 };
  let bw = box.clientWidth, bh = box.clientHeight;
  addEventListener('resize', () => { bw = box.clientWidth; bh = box.clientHeight; });
  aim.x = bw / 2; aim.y = bh / 2;
  pos.forEach((p) => { p.x = aim.x; p.y = aim.y; });
  let inside = false, idle = 0;
  box.addEventListener('pointermove', (e) => { const r = box.getBoundingClientRect(); inside = true; aim.x = e.clientX - r.left; aim.y = e.clientY - r.top; });
  box.addEventListener('pointerleave', () => { inside = false; });
  whileVisible(box, (dt) => {
    // Without a pointer, the head traces a slow figure eight.
    if (!inside && !reduced) { idle += dt / 1000; aim.x = bw / 2 + Math.sin(idle * 1.1) * bw * 0.33; aim.y = bh / 2 + Math.sin(idle * 2.2) * bh * 0.27; }
    spring(pos[0], aim, cfg);
    for (let i = 1; i < n; i++) spring(pos[i], pos[i - 1], cfg);
    for (let i = 0; i < n; i++) set(dots[i], { x: pos[i].x, y: pos[i].y });
  });
  const pills = $$('#springs .mag');
  pills[1].classList.add('on');
  pills.forEach((b, i) => b.addEventListener('click', () => { cfg = presets[i]; pills.forEach((p) => p.classList.toggle('on', p === b)); effects.pop(b, { duration: 0.3 }); }));
  if (fine) magnetic('#springs .mag, #copy', { strength: 0.3, radius: 70 });
});

// ---------------------------------------------------------------------------
// 5 · SVG: the morph is scrubbed by scroll(); the rider loops on its own
// ---------------------------------------------------------------------------
soon('#svg', () => {
  const blob = (seed, n = 9) => pathThrough(Array.from({ length: n }, (_, k) => {
    const a = (k / n) * Math.PI * 2, r = 82 + noise2(k * 0.6, seed) * 34;
    return [260 + Math.cos(a) * r, 150 + Math.sin(a) * r];
  }), { closed: true });
  const shapes = [circle(260, 150, 92), star(260, 150, 10, 110, 66), blob(2), rect(170, 60, 180, 180, 28), star(260, 150, 5, 112, 46), blob(7, 11), circle(260, 150, 92)];
  $('#blob').setAttribute('d', shapes[0]);
  const tl = timeline({ autoplay: false, defaults: { ease: 'inOutCubic', duration: 1 } });
  for (let k = 1; k < shapes.length; k++) tl.add('#blob', { d: shapes[k] }, k - 1);
  tl.add('#blob', { rotate: [0, 180] }, { duration: shapes.length - 1, ease: 'linear' }, 0);
  tl.add('#trail', { draw: [0, 1] }, { duration: 2, ease: 'outCubic' }, 0);
  scroll(tl, { target: $('#svg'), offset: ['start end', 'end start'], smooth: 0.2 });
  if (!reduced) animate('#rider', { path: { path: '#trail' } }, { duration: 6, ease: 'inOutSine', repeat: Infinity, yoyo: true });
});

// ---------------------------------------------------------------------------
// 6 · FLIP grid
// ---------------------------------------------------------------------------
soon('#layout', () => {
  const grid = $('#tiles');
  const cols = ['#ffd9a8', '#ffb37a', '#ff8f6b', '#c9b6ff', '#8fd6ff', '#9fffcf'];
  for (let i = 0; i < 12; i++) {
    const t = document.createElement('div');
    t.className = 'tile';
    t.style.setProperty('--c', cols[i % cols.length]);
    t.textContent = String(i + 1).padStart(2, '0');
    grid.appendChild(t);
  }
  const opts = { duration: 0.9, ease: 'spring(bounce: 0.2, duration: 0.8)', delay: stagger(0.025) };
  const shuffle = () => flip('.tile', () => { const ts = $$('.tile', grid); for (let i = ts.length - 1; i > 0; i--) { const j = Math.floor(Math.random() * (i + 1)); grid.appendChild(ts[j]); ts.splice(j, 1); } }, opts);
  const regrid = () => flip('.tile', () => { grid.classList.toggle('alt'); $$('.tile', grid)[0].classList.toggle('big'); }, opts);
  grid.addEventListener('click', shuffle);
  let n = 0;
  if (!reduced) inView(grid, () => { const id = setInterval(() => (++n % 3 ? shuffle() : regrid()), 2800); return () => clearInterval(id); }, { amount: 0.5 });
});

// ---------------------------------------------------------------------------
// 7 · Benchmarks: counters and bars run in once
// ---------------------------------------------------------------------------
soon('#numbers', () => {
  const section = $('#numbers');
  const counts = $$('.n-count', section);
  const bars = $$('.n-row u', section);
  set(bars, { scaleX: 0 });
  for (const c of counts) c.textContent = (0).toFixed(+c.dataset.dec);
  inView($('.panel', section), () => {
    animate(bars, { scaleX: 1 }, { duration: 1.6, ease: 'spring(bounce: 0.15, duration: 1.3)', delay: stagger(0.18, { start: 0.3 }) });
    counts.forEach((c, i) => { const d = +c.dataset.dec; animate(c, { textContent: +c.dataset.to }, { duration: 1.8, ease: 'outExpo', delay: 0.3 + i * 0.08, format: { minimumFractionDigits: d, maximumFractionDigits: d }, locale: 'en-US' }); });
  }, { amount: 0.5, once: true });
});

// ---------------------------------------------------------------------------
// Install
// ---------------------------------------------------------------------------
const copy = $('#copy');
copy.addEventListener('click', async () => {
  try { await navigator.clipboard.writeText('npm install anikit'); } catch { /* clipboard blocked */ }
  copy.textContent = 'Copied';
  effects.pop(copy, { duration: 0.35 });
  setTimeout(() => (copy.textContent = 'Copy'), 1800);
});
