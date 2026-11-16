// Anikit landing page. A mechanical watch is rendered live by anikit's CPU rasteriser
// (no WebGL) in a worker. One spring-smoothed scroll value drives everything: the watch
// opens into an exploded view, the camera dives into the movement, the demos come and
// go at paced stops, and the watch reassembles above the install command.
import { ready, engine, inView, set, spring } from '../packages/anikit/dist/anikit.lean.js';

const $ = (s) => document.querySelector(s);
const $$ = (s) => Array.from(document.querySelectorAll(s));
const reduced = matchMedia('(prefers-reduced-motion: reduce)').matches;
const fine = matchMedia('(hover: hover) and (pointer: fine)').matches;
const small = innerWidth < 760;
const clamp = (v, a = 0, b = 1) => (v < a ? a : v > b ? b : v);
const ease = (x) => (x < 0.5 ? 4 * x * x * x : 1 - (-2 * x + 2) ** 3 / 2);
const sstep = (a, b, x) => { const t = clamp((x - a) / (b - a)); return t * t * (3 - 2 * t); };

// ---------------------------------------------------------------------------
// Renderer: a worker drawing into a transferred OffscreenCanvas when available,
// otherwise the same renderer on the main thread. Resolution adapts to hold ~8 ms.
// ---------------------------------------------------------------------------
const stageEl = $('#stage'), canvas = $('#watch');
let vw = innerWidth, vh = innerHeight;
const MAXPX = small ? 380_000 : 900_000, MINPX = small ? 90_000 : 160_000;
let budget = small ? 200_000 : 420_000;
const renderSize = () => { const k = Math.sqrt(Math.min(budget, vw * vh * Math.min(2, devicePixelRatio || 1)) / (vw * vh)); return [Math.max(2, Math.round(vw * k)), Math.max(2, Math.round(vh * k))]; };
const stats = { ms: 0, w: 0, h: 0, tris: 0, frames: 0, main: 0, worker: true };
window.__stage = stats;
window.__state = null;
let labelPos = null;
const onFrameStats = (d) => {
  if (!stats.frames) stageEl.classList.add('live');
  stats.ms = stats.frames ? stats.ms * 0.85 + d.ms * 0.15 : d.ms;
  stats.w = d.w; stats.h = d.h; stats.tris = d.tris; stats.frames++;
  if (d.labels) labelPos = d.labels;
  (stats.log ??= []).push(d.ms);
  if (stats.log.length > 4000) stats.log.splice(0, 2000);
};

async function createRenderer() {
  const [w, h] = renderSize();
  if (canvas.transferControlToOffscreen && typeof Worker !== 'undefined' && !location.search.includes('noworker')) {
    const off = canvas.transferControlToOffscreen();
    const worker = new Worker(new URL('./watch-worker.js', import.meta.url), { type: 'module' });
    worker.onmessage = (e) => { if (!e.data.ready) onFrameStats(e.data); };
    worker.onerror = (e) => console.warn('anikit: watch worker failed', e.message);
    worker.postMessage({ canvas: off, width: w, height: h }, [off]);
    return { render: (state) => worker.postMessage({ state }), resize: (W, H) => worker.postMessage({ resize: [W, H] }) };
  }
  stats.worker = false;
  canvas.width = w; canvas.height = h;
  const { createWatch } = await import('./watch.js');
  const watch = await createWatch({ width: w, height: h, canvas });
  const labels = new Float32Array(14);
  const render = (state) => {
      if (state.wantLabels) state.labels = labels;
      const t0 = performance.now(); watch.render(state);
      onFrameStats({ ms: performance.now() - t0, w: watch.width, h: watch.height, tris: watch.triangles, labels: state.wantLabels ? labels : null });
  };
  return { render, resize: (W, H) => { watch.resize(W, H); render(state); } };
}
const renderer = createRenderer();

let lastAdapt = 0, curSize = renderSize();
function adapt(now) {
  if (now - lastAdapt < 700 || stats.frames < 8) return;
  lastAdapt = now;
  const ms = stats.ms;
  if (ms > 9.5 && budget > MINPX) budget = Math.max(MINPX, budget * (ms > 14 ? 0.7 : 0.85));
  else if (ms < 5.5 && budget < MAXPX) budget = Math.min(MAXPX, budget * 1.12);
  else return;
  const s = renderSize();
  if (Math.abs(s[0] - curSize[0]) > 8) { curSize = s; renderer.then((r) => r.resize(s[0], s[1])); }
}

// ---------------------------------------------------------------------------
// Layout: section positions, measured on load and resize.
// ---------------------------------------------------------------------------
const stops = $$('.stop'), panels = stops.map((s) => s.querySelector('.panel'));
const beats = { open: $('#open'), calibre: $('#calibre'), close: $('#close') };
const captions = { open: $('#open .caption'), calibre: $('#calibre .caption') };
const L = {};
const measure = () => {
  vw = innerWidth; vh = innerHeight;
  const y = scrollY, top = (el) => el.getBoundingClientRect().top + y;
  for (const k in beats) L[k] = { top: top(beats[k]), h: beats[k].offsetHeight };
  L.stops = stops.map((s) => ({ top: top(s), h: s.offsetHeight }));
  L.max = document.documentElement.scrollHeight - vh;
};

// ---------------------------------------------------------------------------
// The story: scroll → watch state
// ---------------------------------------------------------------------------
const state = window.__state = { t: 0, clock: 0, explode: 0, dive: 0, yaw: 0, pitch: 0, turn: 0, lift: 0, wantLabels: false };
const tilt = { x: 0, y: 0 };
const prog = (b, y) => (y - L[b].top) / (L[b].h - vh); // 0 when the beat is pinned, 1 when it leaves
/** How visible the watch is (0–1): fades out across the demos, eased over 60vh. */
const visibility = (y) => {
  const F = 0.3 * vh;
  const o = L.open, c = L.calibre, z = L.close;
  const out1 = 1 - sstep(o.top + o.h - vh - F * 0.4, o.top + o.h - vh + F * 1.6, y);
  const in2 = sstep(c.top - F * 0.3, c.top + F * 1.5, y), out2 = 1 - sstep(c.top + c.h - vh - F * 1.9, c.top + c.h - vh + F * 0.1, y);
  const in3 = sstep(z.top - F * 0.3, z.top + F * 1.5, y);
  if (y < c.top - F * 2) return out1;
  if (y < z.top - F * 2) return in2 * out2;
  return in3;
};
function story(y) {
  const near = (b) => y > L[b].top - 1.5 * vh;
  if (!near('calibre')) {
    // Hero → exploded view.
    const e = sstep(0.15 * vh, L.open.top + (L.open.h - vh) * 0.62, y);
    state.explode = e; state.dive = 0; state.lift = 130 * (1 - e) + 20 * e;
    // At rest the watch sways slowly, so the studio highlights travel across the case.
    const sway = reduced ? 0 : 0.09 * Math.sin(performance.now() / 2600) * (1 - e);
    state.turn = 0.25 * Math.sin(Math.PI * clamp(prog('open', y))) * e + sway;
  } else if (!near('close')) {
    // Into the movement.
    const u = prog('calibre', y);
    state.explode = 1; state.dive = sstep(0.0, 0.78, u); state.lift = 0; state.turn = 0;
  } else {
    // Reassembly above the install command.
    const u = prog('close', y);
    state.explode = 1 - sstep(0.02, 0.6, u); state.dive = 0; state.lift = 20 - 100 * sstep(0.02, 0.6, u); state.turn = -0.4 * (1 - sstep(0.02, 0.7, u)) + (reduced ? 0 : 0.09 * Math.sin(performance.now() / 2600) * sstep(0.5, 0.9, u));
  }
}

// ---------------------------------------------------------------------------
// Frame loop
// ---------------------------------------------------------------------------
let sy = scrollY, sv = 0, last = performance.now();
const t0 = performance.now();
const heroEls = $$('.hero h1, .hero .lede');
const labelsEl = $('#labels'), labelEls = $$('#labels span'), hud = $('#hud'), hudLive = $('#hud-live'), install = $('#install');
const lastP = panels.map(() => -1);
const cache = new Map();
const setOnce = (el, key, props) => { if (cache.get(el) === key) return; cache.set(el, key); set(el, props); };
let R = null, stageShown = -1, lastHud = 0, lastKey = '';

function frame(now) {
  const dt = Math.min(0.05, (now - last) / 1000); last = now;
  const f0 = performance.now();
  // Scroll: a critically damped spring (ω ≈ 9.5 rad/s) towards the real position.
  const target = scrollY;
  if (reduced) { sy = target; sv = 0; }
  else {
    const k = 90, c = 2 * Math.sqrt(k);
    for (let i = 0, n = Math.ceil(dt / 0.008), h = dt / n; i < n; i++) { sv += (k * (target - sy) - c * sv) * h; sy += sv * h; }
    if (Math.abs(target - sy) < 0.2 && Math.abs(sv) < 2) { sy = target; sv = 0; }
  }
  story(sy);
  const vis = Math.round(visibility(sy) * 1000) / 1000;
  if (vis !== stageShown) { stageShown = vis; stageEl.style.opacity = String(vis); hud.style.opacity = String(vis); }

  // Pointer tilt on a spring; real time on the dial.
  const d = new Date();
  state.clock = d.getHours() * 3600 + d.getMinutes() * 60 + d.getSeconds() + d.getMilliseconds() / 1000;
  if (reduced) state.clock = Math.floor(state.clock / 60) * 60;
  state.t = reduced ? 0.4 : (now - t0) / 1000;
  state.yaw = tilt.x * (1 - state.dive); state.pitch = tilt.y * (1 - state.dive);
  const oEnd = L.open.top + L.open.h - vh;
  const labelsOn = sy < L.calibre.top - vh ? clamp((state.explode - 0.82) / 0.15) * (1 - sstep(oEnd - 0.3 * vh, oEnd, sy)) : 0;
  state.wantLabels = labelsOn > 0;
  if (vis > 0) {
    const key = reduced ? `${state.explode.toFixed(4)}${state.dive.toFixed(4)}${state.clock}${curSize[0]}` : '';
    if (!reduced || key !== lastKey || stats.frames < 30) { lastKey = key; R.render(state); }
  }
  labelsEl.style.opacity = String(Math.round(labelsOn * 100) / 100);
  if (labelsOn > 0 && labelPos) for (let i = 0; i < labelEls.length; i++) labelEls[i].style.transform = `translate(${(labelPos[i * 2] * vw + 10).toFixed(1)}px, ${(labelPos[i * 2 + 1] * vh - 9).toFixed(1)}px)`;
  choreograph();
  adapt(now);
  if (now - lastHud > 250 && stats.frames) {
    lastHud = now;
    hudLive.textContent = `${stats.ms.toFixed(1)} ms · ${stats.tris.toLocaleString('en-US')} triangles · ${stats.w}×${stats.h}`;
  }
  stats.main = stats.main * 0.9 + (performance.now() - f0) * 0.1;
}

function choreograph() {
  const h = Math.round(clamp(sy / (vh * 0.45)) * 1000) / 1000;
  setOnce(heroEls[0], h, { opacity: 1 - ease(h), y: -h * 30 });
  setOnce(heroEls[1], h, { opacity: 1 - ease(clamp(h * 1.15)), y: -h * 40 });
  // Captions under the watch in each beat.
  for (const k of ['open', 'calibre']) {
    const u = prog(k, sy), a = sstep(0.12, 0.4, u) * (1 - sstep(0.72, 0.95, u));
    const r = Math.round(a * 1000) / 1000;
    setOnce(captions[k], r, { opacity: r, y: (1 - r) * 24 });
  }
  // Install fades in under the reassembled watch.
  { const u = prog('close', sy), r = Math.round(sstep(0.45, 0.75, u) * 1000) / 1000; setOnce(install, r, { opacity: r, y: (1 - r) * 24 }); install.style.pointerEvents = r > 0.5 ? 'auto' : 'none'; }
  // Demo panels: rise in over ~45vh, hold, and rise out.
  for (let k = 0; k < panels.length; k++) {
    const T = L.stops[k];
    const u = (sy - T.top) / (T.h - vh);
    const a = clamp((u + 0.12) / 0.34), b = clamp((u - 0.74) / 0.34);
    const r = Math.round(ease(a) * (1 - ease(b)) * 1000) / 1000;
    const key = r + b * 7;
    if (key === lastP[k]) continue;
    lastP[k] = key;
    set(panels[k], { opacity: r, y: (1 - ease(a)) * 70 - ease(b) * 70, scale: 0.96 + 0.04 * ease(a) - 0.02 * ease(b) });
    panels[k].style.pointerEvents = r > 0.5 ? 'auto' : 'none';
  }
}

// ---------------------------------------------------------------------------
// Start: the poster is the static first frame; the first live frame fades in over it.
// ---------------------------------------------------------------------------
measure();
story(sy);
await ready;
engine.reducedMotion = 'user';
const r0 = await renderer;
R = r0;
measure();
engine.onFrame(() => frame(performance.now()));
addEventListener('resize', () => {
  const big = Math.abs(innerWidth - vw) > 40 || Math.abs(innerHeight - vh) > 140; // ignore mobile toolbar resizes
  measure();
  if (big) { curSize = renderSize(); r0.resize(curSize[0], curSize[1]); }
  lastP.fill(-1); cache.clear(); stageShown = -1;
});
new ResizeObserver(() => measure()).observe(document.body);
if (fine && !reduced) addEventListener('pointermove', (e) => { spring(tilt, { x: (e.clientX / vw - 0.5) * 0.42, y: (e.clientY / vh - 0.5) * 0.24 }, { stiffness: 50, damping: 12 }); }, { passive: true });

// Demos load on approach (dynamic import keeps them off the critical path).
let demos = null;
const loadDemos = () => (demos ??= import('./demos.js').then((m) => m.start({ reduced, fine, small })));
inView('#open', () => { loadDemos(); }, { amount: 0 });
if (scrollY > innerHeight * 0.5) loadDemos();
