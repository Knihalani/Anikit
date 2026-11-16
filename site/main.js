// Anikit landing page: a ray-traced Schwarzschild black hole that you fall into as
// you scroll. The lensing is precomputed per camera-distance band (hole.js); each
// frame shades the rotating disk from lookup tables. Anikit runs the frame loop,
// the scroll choreography and every demo.
import { Lens, shade, diskTexture, shadowScale } from './hole.js';
import { ready, engine, inView, set, animate } from '../packages/anikit/dist/anikit.lean.js';

const $ = (s) => document.querySelector(s);
const reduced = matchMedia('(prefers-reduced-motion: reduce)').matches;
const fine = matchMedia('(hover: hover) and (pointer: fine)').matches;
const small = innerWidth < 760;

// ---------------------------------------------------------------------------
// Sky: two layers (so a new distance band can fade in over the old one) + bloom
// ---------------------------------------------------------------------------
const OVER = 1.12;
const BLUR = 'blur(5px)';
const D0 = small ? 30 : 60, DMIN = 2.12, BAND = 0.9, ELEV = (4.5 * Math.PI) / 180;
const tex = diskTexture();
const bloom = document.createElement('canvas'), bctx = bloom.getContext('2d', { alpha: false });
let budget = small ? 150_000 : 230_000;
let vw = innerWidth, vh = innerHeight;
const layers = ['#sky-a', '#sky-b'].map((s) => ({ el: $(s), ctx: null, img: null, out: null, lens: null, band: -1, scale: 1 }));
let F0 = 1, front = 0, gen = null, genLayer = -1, genBand = -1, fadeUntil = 0;

const size = () => {
  const k = Math.sqrt(budget / (vw * vh * OVER * OVER));
  return [Math.max(64, Math.round(vw * OVER * k)), Math.max(64, Math.round(vh * OVER * k))];
};
const vfovFor = (w, h) => (w >= h ? 40 : (2 * Math.atan(Math.tan((40 * Math.PI) / 180) * (h / w)) * 180) / Math.PI);
const alloc = () => {
  const [w, h] = size();
  for (const L of layers) {
    L.el.width = w; L.el.height = h;
    L.ctx = L.el.getContext('2d', { alpha: false });
    L.img = L.ctx.createImageData(w, h);
    L.out = new Uint32Array(L.img.data.buffer);
    L.lens = new Lens(w, h);
    L.band = -1;
  }
  F0 = h / 2 / Math.tan((vfovFor(w, h) * Math.PI) / 360);
  bloom.width = Math.max(32, w >> 2); bloom.height = Math.max(32, h >> 2);
};
const bandD = (k) => D0 * BAND ** k;
const bandFor = (D) => Math.max(0, Math.floor(Math.log(D / D0) / Math.log(BAND) + 1e-6));

// Smoothed scroll → fall depth. Every visual below is a pure function of `sy`.
let sy = scrollY, horizonTop = 1, installTop = 1, docH = 1;
const measure = () => {
  const y = scrollY;
  horizonTop = $('#horizon').getBoundingClientRect().top + y;
  installTop = $('#install').getBoundingClientRect().top + y;
  docH = document.documentElement.scrollHeight;
};
const clamp = (v) => (v < 0 ? 0 : v > 1 ? 1 : v);
// The fall: slow through the demos (down to DM), then quickly through the horizon.
const DM = 9;
// The field of view widens as you fall, so the shadow never quite swallows the page.
const focal = (D) => { const u = clamp(Math.log(D0 / D) / Math.log(D0 / DMIN)); return F0 / (1 + 2.5 * u ** 1.2); };
const fallP = () => clamp(sy / (horizonTop + vh * 0.6));
const depth = (p) => {
  const a = horizonTop / (horizonTop + vh * 0.6);
  return p <= a ? D0 * (DM / D0) ** ((p / a) ** 1.25) : DM * (DMIN / DM) ** ((p - a) / (1 - a));
};

let t = 0, exposure = 2.4;
const canBlur = 'filter' in bctx; // no canvas filters (older WebKit): skip the bloom pass
const scaleOf = (L, D) => (shadowScale(D) * focal(D)) / (shadowScale(L.lens.D) * L.lens.F);
const zoom = (L, D) => { L.el.style.transform = `scale(${scaleOf(L, D).toFixed(4)})`; };
const draw = (L, D, p) => {
  // Blue shift and aberration as the fall speeds up; time near the hole runs slow.
  const sb = 1 + 0.9 * p * p, sr = 1 - 0.45 * p * p;
  shade(L.lens, tex, L.out, t, exposure * (1 + p * 0.1), sr * (1 + 1.5 * p * p), (1 + 1.2 * p * p), sb * (1 + 1.5 * p * p), 1, frames);
  L.ctx.putImageData(L.img, 0, 0);
  if (!canBlur) return;
  // Bloom: blur a quarter-size copy, then screen it back over the frame.
  bctx.filter = BLUR;
  bctx.drawImage(L.el, 0, 0, bloom.width, bloom.height);
  L.ctx.globalCompositeOperation = 'screen';
  L.ctx.globalAlpha = 0.6;
  L.ctx.drawImage(bloom, 0, 0, L.el.width, L.el.height);
  L.ctx.globalAlpha = 1;
  L.ctx.globalCompositeOperation = 'source-over';
};

// Build the band for a camera distance on the back layer, in slices; then cross-fade.
const FADE = 280;
let fadeStart = 0;
const startBuild = (k) => {
  genLayer = 1 - front; genBand = k;
  gen = layers[genLayer].lens.build(bandD(k), ELEV, focal(bandD(k)));
};
const finishBuild = (instant = reduced) => {
  const L = layers[genLayer];
  L.band = genBand; gen = null;
  front = genLayer;
  L.el.style.zIndex = '2'; layers[1 - front].el.style.zIndex = '1';
  lastSy = -1; // draw the new band on this frame, even if the scroll is still
  if (instant) { L.el.style.opacity = '1'; layers[1 - front].el.style.opacity = '0'; fadeUntil = 0; return; }
  L.el.style.opacity = '0';
  fadeStart = performance.now(); fadeUntil = fadeStart + FADE;
};

const stats = { ms: 0, shade: 0, w: 0, h: 0, last: 0, band: 0 };
window.__stage = stats;
let lastSy = -1, frames = 0, worst = 0;
const frame = (dt) => {
  sy += (scrollY - sy) * (reduced ? 1 : 1 - Math.exp(-dt / 90));
  if (Math.abs(scrollY - sy) < 0.5) sy = scrollY;
  const p = fallP(), D = depth(p);
  const tScale = 1 - 0.7 * p ** 3;
  if (!reduced) t += (Math.min(dt, 50) / 1000) * tScale;
  const t0 = performance.now();
  frames++;
  // When shading is expensive (close to the hole the disk fills the view), shade on
  // even frames and build on odd ones; the zoom is a CSS transform and runs every frame.
  const heavy = stats.shade > 6, even = (frames & 1) === 0;
  const k = bandFor(D);
  if (!gen && layers[front].band !== k && p < 0.995 && t0 > fadeUntil) startBuild(k);
  if (gen && (!heavy || !even) && gen.next().done) finishBuild();
  const F = layers[front];
  if (p < 0.999) {
    const still = reduced && sy === lastSy;
    if (!still && (!heavy || even || lastSy < 0)) {
      const s0 = performance.now();
      draw(F, D, p);
      stats.shade = stats.shade * 0.85 + (performance.now() - s0) * 0.15;
      lastSy = sy;
    }
    zoom(F, D);
    if (fadeUntil) {
      // The outgoing band keeps its last frame (rescaled) while the new one fades in.
      const O = layers[1 - front];
      const f = (t0 - fadeStart) / FADE;
      if (f < 1) { zoom(O, D); F.el.style.opacity = f.toFixed(3); }
      else { fadeUntil = 0; O.el.style.opacity = '0'; F.el.style.opacity = '1'; }
    }
  }
  choreograph(p);
  const ms = performance.now() - t0;
  stats.ms = stats.ms * 0.9 + ms * 0.1; stats.last = ms; stats.w = F.lens.w; stats.h = F.lens.h; stats.band = F.band;
  // Adaptive resolution: once, early, while the hero is idle (after V8 has warmed up).
  if (frames === 150 && !gen && sy < 2 && stats.shade > 7) {
    worst = stats.shade; budget = Math.round(budget * Math.min(0.8, 5.5 / worst));
    alloc(); startBuild(bandFor(D)); while (gen && !gen.next().done); finishBuild(true);
  }
};

// ---------------------------------------------------------------------------
// Choreography: hero copy, panels, horizon, white hole, calm
// ---------------------------------------------------------------------------
const heroEls = [...document.querySelectorAll('.hero h1, .hero .lede, .hero .cue')];
const sections = [...document.querySelectorAll('.demo')];
const panels = sections.map((s) => s.querySelector('.panel'));
sections.forEach((s) => s.style.setProperty('--accent', s.dataset.accent));
let tops = [];
const measurePanels = () => { tops = sections.map((s) => { const b = s.getBoundingClientRect(); return { top: b.top + scrollY, h: b.height }; }); };
const lastP = panels.map(() => -1);
let lastHero = -1, lastV = -1;
const veil = $('#veil'), flash = $('#flash'), calm = $('#calm'), sky = $('#sky');
const ease = (x) => (x < 0.5 ? 4 * x * x * x : 1 - (-2 * x + 2) ** 3 / 2);
function choreograph() {
  const h = Math.round(clamp(sy / (vh * 0.5)) * 1000) / 1000;
  if (h !== lastHero) { lastHero = h; set(heroEls, { opacity: 1 - h, y: -h * 40 }); }
  for (let k = 0; k < panels.length; k++) {
    const T = tops[k];
    if (!T) continue;
    // In as the section arrives, out as it leaves: a continuous scrub.
    const a = clamp((sy + vh - T.top) / (vh * 0.55)), b = clamp((sy - (T.top + T.h - vh * 0.9)) / (vh * 0.5));
    const r = Math.round(ease(a) * (1 - ease(b)) * 1000) / 1000;
    if (r === lastP[k]) continue;
    lastP[k] = r;
    set(panels[k], { opacity: r, y: (1 - ease(a)) * 80 - ease(b) * 60, scale: 0.96 + r * 0.04 });
  }
  // The horizon: the sky goes dark; then a white bloom opens onto a quiet starfield.
  const x = clamp((sy - (horizonTop + vh * 0.05)) / (vh * 0.55));
  const w = clamp((sy - (installTop - vh * 1.0)) / (vh * 0.55));
  const v = Math.round(x * 1000) / 1000 + Math.round(w * 1000) * 10;
  if (v !== lastV) {
    lastV = v;
    veil.style.opacity = String(ease(x) * (1 - w));
    sky.style.visibility = w >= 1 ? 'hidden' : 'visible';
    const fl = Math.sin(Math.PI * Math.min(1, w * 1.25));
    flash.style.opacity = String(Math.min(1, fl * 1.5));
    flash.style.transform = `scale(${(0.2 + w * 1.6).toFixed(3)})`;
    calm.style.opacity = String(clamp(w * 1.5 - 0.3));
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
  const n = Math.round((vw * vh) / 2200);
  for (let i = 0; i < n; i++) {
    const m = rnd() ** 6, tt = rnd();
    const col = tt < 0.3 ? '255,214,170' : tt < 0.8 ? '255,250,240' : '196,214,255';
    c.fillStyle = `rgba(${col},${0.25 + m * 0.75})`;
    const r = (0.45 + m * 1.2) * dpr;
    c.beginPath(); c.arc(rnd() * calm.width, rnd() * calm.height, r, 0, Math.PI * 2); c.fill();
  }
};


// ---------------------------------------------------------------------------
// Start: the first frame is drawn before the WebAssembly core has loaded.
// ---------------------------------------------------------------------------
alloc();
measure(); measurePanels();
startBuild(bandFor(depth(fallP())));
while (gen && !gen.next().done);
finishBuild(true);
frame(16);
sky.classList.add('on');
drawCalm();

await ready;
engine.reducedMotion = 'user';
engine.onFrame((_t, dt) => frame(dt));
const onResize = () => {
  const nw = innerWidth, nh = innerHeight;
  const big = Math.abs(nw - vw) > 40 || Math.abs(nh - vh) > 140; // ignore mobile toolbar resizes
  vw = nw; vh = nh;
  measure(); measurePanels();
  if (big) { alloc(); startBuild(bandFor(depth(fallP()))); while (gen && !gen.next().done); finishBuild(true); drawCalm(); }
  lastP.fill(-1); lastV = -1; lastHero = -1;
};
addEventListener('resize', onResize);
new ResizeObserver(() => { measure(); measurePanels(); }).observe(document.body);
if (!reduced) animate('#cue', { opacity: [0.35, 1, 0.35] }, { duration: 2.6, repeat: Infinity, ease: 'inOutSine' });

if (location.search.includes('debug')) {
  window.__shade = (n = 20) => { const L = layers[front], out = []; for (let i = 0; i < n; i++) { const a = performance.now(); shade(L.lens, tex, L.out, i * 0.1, 1, 1, 1, 1, 1); L.ctx.putImageData(L.img, 0, 0); out.push(+(performance.now() - a).toFixed(2)); } return out; };
}

// Demos load on approach (dynamic import keeps them off the critical path).
let demos = null;
const loadDemos = () => (demos ??= import('./demos.js').then((m) => m.start({ reduced, fine, small })));
inView('#springs', () => { loadDemos(); }, { amount: 0, margin: '0px 0px 100% 0px' });
if (scrollY > innerHeight * 0.5) loadDemos();
