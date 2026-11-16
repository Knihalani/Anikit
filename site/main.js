// Anikit landing page: you fall into a black hole as you scroll. One smoothed scroll
// value drives the camera distance continuously: the hole grows, you cross the
// horizon, the demos come at paced stops in the tunnel inside, and you leave through
// a white hole. The renderer sits behind a tiny adapter with the same shape as
// anikit's core `blackHole()` (render({ distance, inclination, fov, time, ... })).
import { ready, engine, inView, set, animate } from '../packages/anikit/dist/anikit.lean.js';

const $ = (s) => document.querySelector(s);
const reduced = matchMedia('(prefers-reduced-motion: reduce)').matches;
const fine = matchMedia('(hover: hover) and (pointer: fine)').matches;
const small = innerWidth < 760;

// ---------------------------------------------------------------------------
// Renderer adapter: a worker with an OffscreenCanvas when available, so scrolling
// never waits for a frame; otherwise the same renderer on the main thread.
// ---------------------------------------------------------------------------
const canvas = $('#sky-canvas');
let budget = small ? 150_000 : 320_000;
let vw = innerWidth, vh = innerHeight;
const renderSize = () => { const k = Math.min(1, Math.sqrt(budget / (vw * vh))); return [Math.round(vw * k), Math.round(vh * k)]; };
const stats = { ms: 0, w: 0, h: 0, main: 0, frames: 0 };
window.__stage = stats;

async function createSky() {
  const [w, h] = renderSize();
  if (canvas.transferControlToOffscreen && typeof Worker !== 'undefined' && !location.search.includes('noworker')) {
    const off = canvas.transferControlToOffscreen();
    const worker = new Worker(new URL('./hole-worker.js', import.meta.url), { type: 'module' });
    worker.onmessage = (e) => { stats.ms = stats.ms * 0.9 + e.data.ms * 0.1; stats.w = e.data.w; stats.h = e.data.h; stats.frames++; };
    worker.postMessage({ canvas: off, width: w, height: h }, [off]);
    return { render: (opts) => worker.postMessage({ opts }), resize: (W, H) => worker.postMessage({ resize: [W, H] }) };
  }
  const { blackHole } = await import('./hole.js');
  const bh = blackHole({ width: w, height: h, canvas });
  return {
    render: (opts) => { const t0 = performance.now(); bh.render(opts); stats.ms = stats.ms * 0.9 + (performance.now() - t0) * 0.1; stats.w = bh.width; stats.h = bh.height; stats.frames++; },
    resize: (W, H) => bh.resize(W, H),
  };
}
const sky = await createSky();

// ---------------------------------------------------------------------------
// The journey: scroll position → camera distance (M), with paced stops inside
// ---------------------------------------------------------------------------
const D0 = small ? 34 : 46, DH = 2.02, DEND = 0.12;
const stops = [...document.querySelectorAll('.stop')];
const panels = stops.map((s) => s.querySelector('.panel'));
stops.forEach((s) => s.style.setProperty('--accent', s.dataset.accent));
let fallTop = 0, fallEnd = 1, tops = [], exitTop = 1, installTop = 1;
const measure = () => {
  vw = innerWidth; vh = innerHeight;
  const y = scrollY, top = (el) => el.getBoundingClientRect().top + y;
  fallTop = 0;
  fallEnd = top(stops[0]); // the horizon is crossed as the first stop arrives
  tops = stops.map((s) => ({ top: top(s), h: s.offsetHeight }));
  exitTop = top($('#horizon')); installTop = top($('#install'));
};
const clamp = (v) => (v < 0 ? 0 : v > 1 ? 1 : v);
const ease = (x) => (x < 0.5 ? 4 * x * x * x : 1 - (-2 * x + 2) ** 3 / 2);
/** Camera distance for a scroll position. Outside: a smooth fall from D0 to the horizon. */
const distanceAt = (y) => {
  if (y < fallEnd) {
    const p = clamp(y / fallEnd);
    return D0 * (DH / D0) ** (p ** 1.6);
  }
  // Inside: each stop slows the fall while its demo is centred, then moves on.
  const N = stops.length;
  for (let k = 0; k < N; k++) {
    const T = tops[k], u = (y - T.top) / (T.h - vh);
    if (u <= 1 || k === N - 1) {
      const uu = clamp(u);
      const e = uu + (0.8 * Math.sin(TAU * uu)) / TAU; // fast at the edges, slow in the middle
      const P = (k + e) / N;
      return DH * (DEND / DH) ** P;
    }
  }
  return DEND;
};
const TAU = Math.PI * 2;

// ---------------------------------------------------------------------------
// Frame loop
// ---------------------------------------------------------------------------
let sy = scrollY, t = 0, lastKey = '';
const veil = $('#veil'), flash = $('#flash'), calm = $('#calm'), skyEl = $('#sky');
const heroEls = [...document.querySelectorAll('.hero h1, .hero .lede, .hero .cue')];
const lastP = panels.map(() => -1);
let lastHero = -1, lastEnd = -1;
const frame = (dt) => {
  const t0 = performance.now();
  sy += (scrollY - sy) * (reduced ? 1 : 1 - Math.exp(-dt / 110));
  if (Math.abs(scrollY - sy) < 0.3) sy = scrollY;
  const D = distanceAt(sy);
  // Clocks run slow near the horizon (subtly).
  if (!reduced) t += (Math.min(dt, 50) / 1000) * (D > DH ? 0.45 + 0.55 * clamp((D - 2.4) / 10) : 0.7);
  // The view widens as you fall in, so the shadow grows without swallowing the page.
  const near = clamp(Math.log(D0 / Math.max(D, DH)) / Math.log(D0 / DH));
  const fov = (small ? 62 : 42) + near ** 1.5 * (small ? 40 : 52);
  const p = clamp(sy / fallEnd);
  const blue = p * p;
  const tint = D > DH ? [1 - 0.3 * blue, 1, 1 + 0.6 * blue] : [1, 1, 1];
  const key = reduced ? `${D.toFixed(4)}` : '';
  if (!reduced || key !== lastKey) {
    lastKey = key;
    sky.render({ distance: D, inclination: 84.5, fov, time: t, exposure: 2.3 * (1 - 0.45 * near * near), bloom: 0.55, grain: 4, tint });
  }
  choreograph();
  stats.main = stats.main * 0.9 + (performance.now() - t0) * 0.1;
};

function choreograph() {
  const h = Math.round(clamp(sy / (vh * 0.5)) * 1000) / 1000;
  if (h !== lastHero) { lastHero = h; set(heroEls, { opacity: 1 - h, y: -h * 40 }); }
  // Each stop: the panel rises in, holds while the fall slows, and rises out.
  for (let k = 0; k < panels.length; k++) {
    const T = tops[k];
    if (!T) continue;
    const u = (sy - T.top) / (T.h - vh);
    const a = clamp((u + 0.08) / 0.3), b = clamp((u - 0.78) / 0.3);
    const r = Math.round(ease(a) * (1 - ease(b)) * 1000) / 1000;
    const key = r + b * 7;
    if (key === lastP[k]) continue;
    lastP[k] = key;
    set(panels[k], { opacity: r, y: (1 - ease(a)) * 90 - ease(b) * 90, scale: 0.94 + 0.06 * ease(a) - 0.04 * ease(b) });
    panels[k].style.pointerEvents = r > 0.5 ? 'auto' : 'none';
  }
  // The exit: the tunnel narrows to a white point that blooms open onto quiet stars.
  const x = clamp((sy - exitTop) / (installTop - exitTop));
  const v = Math.round(x * 1000);
  if (v !== lastEnd) {
    lastEnd = v;
    const fl = Math.sin(Math.PI * Math.min(1, x * 1.15));
    flash.style.opacity = String(Math.min(1, fl * 1.6));
    flash.style.transform = `scale(${(0.05 + x * 1.7).toFixed(3)})`;
    calm.style.opacity = String(clamp(x * 1.6 - 0.45));
    skyEl.style.visibility = x > 0.75 ? 'hidden' : 'visible';
    veil.style.opacity = '0';
  }
}

// A quiet starfield for the end, drawn once.
const drawCalm = () => {
  const dpr = Math.min(2, devicePixelRatio || 1);
  calm.width = Math.round(vw * dpr); calm.height = Math.round(vh * dpr);
  const c = calm.getContext('2d');
  c.fillStyle = '#000'; c.fillRect(0, 0, calm.width, calm.height);
  let s = 7;
  const rnd = () => ((s = (s * 1664525 + 1013904223) >>> 0) / 4294967296);
  const n = Math.round((vw * vh) / 2600);
  for (let i = 0; i < n; i++) {
    const m = rnd() ** 6, tt = rnd();
    const col = tt < 0.3 ? '255,214,170' : tt < 0.8 ? '255,250,240' : '196,214,255';
    c.fillStyle = `rgba(${col},${0.25 + m * 0.75})`;
    c.beginPath(); c.arc(rnd() * calm.width, rnd() * calm.height, (0.4 + m * 1.1) * dpr, 0, TAU); c.fill();
  }
};

// ---------------------------------------------------------------------------
// Start: the first frame is requested before the WebAssembly core has loaded.
// ---------------------------------------------------------------------------
measure();
frame(16);
skyEl.classList.add('on');
drawCalm();

await ready;
engine.reducedMotion = 'user';
engine.onFrame((_t, dt) => frame(dt));
addEventListener('resize', () => {
  const big = Math.abs(innerWidth - vw) > 40 || Math.abs(innerHeight - vh) > 140; // ignore mobile toolbar resizes
  measure();
  if (big) { sky.resize(...renderSize()); drawCalm(); }
  lastP.fill(-1); lastEnd = -1; lastHero = -1;
});
new ResizeObserver(() => measure()).observe(document.body);
if (!reduced) animate('#cue', { opacity: [0.35, 1, 0.35] }, { duration: 2.6, repeat: Infinity, ease: 'inOutSine' });

// Demos load on approach (dynamic import keeps them off the critical path).
let demos = null;
const loadDemos = () => (demos ??= import('./demos.js').then((m) => m.start({ reduced, fine, small })));
inView('#fall', () => { loadDemos(); }, { amount: 0 });
if (scrollY > innerHeight * 0.5) loadDemos();
