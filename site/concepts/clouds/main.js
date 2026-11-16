// Anikit — clouds. You fly up through a golden-hour cloudscape as you scroll. The clouds
// are ray marched in a WebGL fragment shader (sky.js); anikit drives everything else:
// the camera's flight path, the spring-smoothed scroll, the pointer tilt, the UI motion
// and the demos. Each demo sits in a clear pocket of sky; between them a cloud bank
// swallows you. At the end you rise above the deck into the sunset.
import { createSky } from './sky.js';

const $ = (s) => document.querySelector(s);
const $$ = (s) => Array.from(document.querySelectorAll(s));
const reduced = matchMedia('(prefers-reduced-motion: reduce)').matches;
const fine = matchMedia('(hover: hover) and (pointer: fine)').matches;
const small = innerWidth < 760;
const clamp = (v, a = 0, b = 1) => (v < a ? a : v > b ? b : v);
const ease = (x) => (x < 0.5 ? 4 * x * x * x : 1 - (-2 * x + 2) ** 3 / 2);
const sstep = (a, b, x) => { const t = clamp((x - a) / (b - a)); return t * t * (3 - 2 * t); };
const lerp = (a, b, k) => a + (b - a) * k;

// ---------------------------------------------------------------------------
// The sky. Its first frame is drawn before anikit's WebAssembly has loaded.
// ---------------------------------------------------------------------------
const canvas = $('#sky');
let sky = null;
try { sky = createSky(canvas); } catch (e) { console.warn('anikit: sky shader unavailable', e.message); }
const dprCap = small ? 1.25 : 1.5;
let vw = innerWidth, vh = innerHeight;
let comp = 1; // composite resolution, lowered only when the clouds alone can't hold the frame rate
const fit = () => { const d = Math.min(dprCap, devicePixelRatio || 1) * comp; canvas.width = Math.round(vw * d); canvas.height = Math.round(vh * d); };
fit();
if (sky) sky.scale = small ? 0.32 : 0.42;
const SMAX = small ? 0.5 : 0.7, SMIN = 0.18;
const stats = { frames: 0, dt: 16.7, log: [], scale: sky?.scale ?? 0 };
window.__stage = stats;

// A flight path rising from under the cloud base, through the deck, into the open sky above.
// Points in metres: x (sideways drift), y (altitude), z (forward is −z).
const PTS = [[0, 650, 0], [380, 900, -4200], [-260, 1550, -8400], [420, 1850, -12600], [-380, 2150, -16800], [300, 2450, -21000], [-420, 2700, -25200], [240, 2950, -29400], [0, 3750, -33600], [0, 4150, -37800]];

// ---------------------------------------------------------------------------
// Layout
// ---------------------------------------------------------------------------
const stops = $$('.stop'), panels = stops.map((s) => s.querySelector('.panel'));
const L = { stops: [], above: 0, max: 1 };
const panelR = (k, y) => {
  const T = L.stops[k]; if (!T) return 0;
  const u = (y - T.top) / (T.h - vh);
  const a = clamp((u + 0.12) / 0.34), b = clamp((u - 0.74) / 0.34);
  return ease(a) * (1 - ease(b));
};
const pocket = (y) => { let r = 0; for (let k = 0; k < stops.length; k++) r = Math.max(r, panelR(k, y)); return r; };
// Scroll → flight progress, slower while a demo is shown (so the pocket holds) and faster through the banks.
let table = new Float32Array(2), STEP = 10;
const measure = () => {
  vw = innerWidth; vh = innerHeight;
  const y0 = scrollY, top = (el) => el.getBoundingClientRect().top + y0;
  L.stops = stops.map((s) => ({ top: top(s), h: s.offsetHeight }));
  L.above = top($('#install'));
  L.max = Math.max(1, document.documentElement.scrollHeight - vh);
  const n = Math.ceil(L.max / STEP) + 2;
  table = new Float32Array(n);
  for (let i = 1; i < n; i++) { const y = i * STEP; table[i] = table[i - 1] + 0.3 + 0.7 * (1 - pocket(y)); }
  const end = table[n - 1]; for (let i = 0; i < n; i++) table[i] /= end;
};
const progressAt = (y) => { const f = clamp(y, 0, L.max) / STEP, i = Math.floor(f); return lerp(table[i] ?? 1, table[i + 1] ?? 1, f - i); };

// ---------------------------------------------------------------------------
// Frame loop
// ---------------------------------------------------------------------------
const state = { eye: [0, 0, 0], target: [0, 0, 0], roll: 0, time: 0, clear: 0, bank: 0, sun: [0, 0, -1], fov: small ? 70 : 58, exposure: 1.1, rays: 0.6 };
const tilt = { x: 0, y: 0 };
let path = null, sy = scrollY, sv = 0, last = performance.now(), t = 0, lastKey = '', lastHud = 0, slowFor = 0, fastFor = 0;
const heroEls = $$('.hero h1, .hero .lede'), install = $('#install-core'), hudLive = $('#hud-live');
const lastP = panels.map(() => -1), cache = new Map();
let setFn = null;
const setOnce = (el, key, props) => { if (cache.get(el) === key) return; cache.set(el, key); setFn(el, props); };

function pose(p) {
  if (!path) { // before anikit has loaded: interpolate the points directly
    const f = p * (PTS.length - 1), i = Math.min(PTS.length - 2, Math.floor(f)), k = f - i;
    for (let j = 0; j < 3; j++) { state.eye[j] = lerp(PTS[i][j], PTS[i + 1][j], k); state.target[j] = lerp(PTS[i][j], PTS[i + 1][j], Math.min(1, k + 0.3)); }
    state.roll = 0; return;
  }
  const q = path.at(p * path.length);
  for (let j = 0; j < 3; j++) { state.eye[j] = q.eye[j]; state.target[j] = q.target[j]; }
  state.roll = q.roll;
}

function frame(now) {
  const dt = Math.min(0.05, (now - last) / 1000); last = now;
  stats.dt = stats.dt * 0.92 + dt * 1000 * 0.08; stats.frames++;
  if (stats.log.length < 6000) stats.log.push(dt * 1000);
  // Scroll: a critically damped spring towards the real position.
  if (reduced) { sy = scrollY; sv = 0; }
  else {
    const k = 80, c = 2 * Math.sqrt(k);
    for (let i = 0, n = Math.ceil(dt / 0.008), h = dt / n; i < n; i++) { sv += (k * (scrollY - sy) - c * sv) * h; sy += sv * h; }
    if (Math.abs(scrollY - sy) < 0.2 && Math.abs(sv) < 2) { sy = scrollY; sv = 0; }
  }
  const p = progressAt(sy);
  if (!reduced) t += dt;
  pose(p);
  // Pointer tilt, in metres at the look-ahead point.
  state.target[0] += tilt.x * 260; state.target[1] -= tilt.y * 160;
  // Hero: look up at the cloud base.
  const hero = 1 - sstep(0, vh * 1.2, sy);
  state.target[1] += hero * 420;
  state.clear = clamp(pocket(sy) * 1.35);
  // Between pockets, inside the deck, a cloud bank closes around you.
  const inDeck = sstep(1450, 1750, state.eye[1]) * (1 - sstep(3050, 3350, state.eye[1]));
  state.bank = inDeck * (1 - state.clear) * (1 - hero);
  state.time = t;
  // The sun sinks from golden hour to sunset as you climb.
  const el = lerp(13, 2.2, p) * Math.PI / 180, az = lerp(48, 26, p) * Math.PI / 180;
  state.sun[0] = Math.sin(az) * Math.cos(el); state.sun[1] = Math.sin(el); state.sun[2] = -Math.cos(az) * Math.cos(el);
  state.rays = 0.45 + 0.55 * p; state.exposure = 1.0 - 0.22 * p * p;
  const key = reduced ? `${Math.round(sy)}|${canvas.width}` : '';
  if (sky && (!reduced || key !== lastKey)) { lastKey = key; sky.render(state); }
  if (setFn) choreograph();
  adapt(dt);
  if (now - lastHud > 500 && sky) {
    lastHud = now;
    const [w, h] = sky.size();
    hudLive.textContent = `${w}×${h} · ${Math.round(1000 / stats.dt)} fps`;
  }
}

// Adaptive resolution: hold the frame rate by marching the clouds at fewer pixels.
function adapt(dt) {
  if (!sky || reduced) return;
  const ms = dt * 1000;
  if (ms > 19) { slowFor += dt; fastFor = 0; } else if (ms < 17.6) { fastFor += dt; slowFor = 0; }
  if (slowFor > 0.4 && sky.scale > SMIN) { sky.scale = Math.max(SMIN, sky.scale * 0.85); slowFor = 0; }
  else if (slowFor > 1 && comp > 0.65) { comp = Math.max(0.65, comp * 0.85); fit(); slowFor = 0; }
  if (fastFor > 2.5 && sky.scale < SMAX) { sky.scale = Math.min(SMAX, sky.scale * 1.06); fastFor = 0; }
  stats.scale = sky.scale;
}

function choreograph() {
  const h = Math.round(clamp(sy / (vh * 0.5)) * 1000) / 1000;
  setOnce(heroEls[0], h, { opacity: 1 - ease(h), y: -h * 40 });
  setOnce(heroEls[1], h, { opacity: 1 - ease(clamp(h * 1.2)), y: -h * 50 });
  for (let k = 0; k < panels.length; k++) {
    const T = L.stops[k]; if (!T) continue;
    const u = (sy - T.top) / (T.h - vh);
    const a = clamp((u + 0.12) / 0.34), b = clamp((u - 0.74) / 0.34);
    const r = Math.round(ease(a) * (1 - ease(b)) * 1000) / 1000;
    const key = r + b * 7;
    if (key === lastP[k]) continue;
    lastP[k] = key;
    setFn(panels[k], { opacity: r, y: (1 - ease(a)) * 80 - ease(b) * 80, scale: 0.95 + 0.05 * ease(a) - 0.03 * ease(b) });
    panels[k].style.pointerEvents = r > 0.5 ? 'auto' : 'none';
  }
  const u = (sy - L.above) / Math.max(1, L.max - L.above);
  const r = Math.round(sstep(0.25, 0.75, u) * 1000) / 1000;
  setOnce(install, r, { opacity: r, y: (1 - r) * 30 });
  install.style.pointerEvents = r > 0.5 ? 'auto' : 'none';
}

// ---------------------------------------------------------------------------
// Start: the first frame now, the loop once anikit is ready.
// ---------------------------------------------------------------------------
measure();
frame(performance.now());
const A = await import('../../../packages/anikit/dist/anikit.lean.js');
const { ready, engine, set, spring, flightPath, splitText, animate, stagger, inView } = A;
await ready;
engine.reducedMotion = 'user';
setFn = set;
path = flightPath(PTS, { lookAhead: 900, bank: 1.2, maxBank: 14 });
measure();
engine.onFrame(() => frame(performance.now()));
addEventListener('resize', () => {
  const big = Math.abs(innerWidth - vw) > 40 || Math.abs(innerHeight - vh) > 140;
  measure();
  if (big) fit();
  lastP.fill(-1); cache.clear(); lastKey = '';
});
new ResizeObserver(() => measure()).observe(document.body);
if (fine && !reduced) addEventListener('pointermove', (e) => { spring(tilt, { x: (e.clientX / vw - 0.5), y: (e.clientY / vh - 0.5) }, { stiffness: 40, damping: 11 }); }, { passive: true });

// The wordmark rises in, letter by letter.
if (!reduced) {
  const chars = splitText($('.hero h1'), { type: 'chars', charClass: 'hc' }).chars;
  animate(chars, { yPercent: [60, 0], opacity: [0, 1] }, { duration: 1.2, ease: 'outExpo', delay: stagger(0.05, { start: 0.15 }) });
  animate('.hero .lede', { y: [16, 0], opacity: [0, 1] }, { duration: 1.2, ease: 'outCubic', delay: 0.5 });
}

// Nav links land where each section is fully shown.
for (const a of $$('.nav a[href^="#"]')) a.addEventListener('click', (e) => {
  const id = a.getAttribute('href').slice(1), el = document.getElementById(id);
  if (!el) return;
  e.preventDefault();
  const top = el.getBoundingClientRect().top + scrollY;
  const y = id === 'top' ? 0 : id === 'install' ? L.max : top + (el.offsetHeight - vh) * 0.45;
  scrollTo({ top: y, behavior: reduced ? 'auto' : 'smooth' });
});

// Demos load on approach.
let demos = null;
const loadDemos = () => (demos ??= import('./demos.js').then((m) => m.start({ reduced, fine, small })));
inView('.bank', () => { loadDemos(); }, { amount: 0 });
if (scrollY > innerHeight * 0.5) loadDemos();
