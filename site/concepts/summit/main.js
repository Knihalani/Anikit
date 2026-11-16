// Summit: the page. One loop (Anikit's frame clock) drives three canvases:
// the mountain at the bottom, the hall seen through the portal, the hands on top.
// Scroll progress is smoothed with an Anikit spring and mapped to p, g, push and k (see CONTRACT.md).
import { ready, spring, engine, animate } from '../../../packages/anikit/dist/anikit.lean.js';
import { createHands } from './hands.js';
import { createHall } from './hall.js';

const root = document.documentElement;
const $ = (s) => document.querySelector(s);
const reduced = matchMedia('(prefers-reduced-motion: reduce)').matches;
const clamp01 = (v) => (v < 0 ? 0 : v > 1 ? 1 : v);

// A stand-in for mountain.js until it lands: sky, sun and "anikit" in the clouds, on a 2D canvas.
function stubMountain(canvas) {
  const ctx = canvas.getContext('2d');
  let w = 1, h = 1, d = 1, lastKey = '';
  return {
    resize(cssW, cssH, dpr) { w = cssW; h = cssH; d = Math.min(dpr || 1, 2); canvas.width = Math.round(w * d); canvas.height = Math.round(h * d); lastKey = ''; },
    frame(t, dt, p, look, push) {
      const key = `${p.toFixed(3)}|${look.x.toFixed(3)}|${look.y.toFixed(3)}|${push.toFixed(3)}`;
      if (key === lastKey) return;
      lastKey = key;
      ctx.setTransform(d, 0, 0, d, 0, 0);
      const s = 1 + push * 0.6, ox = -look.x * 14, oy = look.y * 8;
      const g = ctx.createLinearGradient(0, 0, 0, h);
      g.addColorStop(0, '#6f8fc6'); g.addColorStop(0.4, '#a7b2cf'); g.addColorStop(0.64, '#f2c99e'); g.addColorStop(0.72, '#fbe3c4');
      ctx.fillStyle = g; ctx.fillRect(0, 0, w, h);
      const sx = w * 0.72 + ox, sy = h * 0.66 + oy;
      const sun = ctx.createRadialGradient(sx, sy, 0, sx, sy, h * 0.35);
      sun.addColorStop(0, 'rgba(255,240,210,1)'); sun.addColorStop(0.08, 'rgba(255,214,150,0.9)'); sun.addColorStop(1, 'rgba(255,214,150,0)');
      ctx.fillStyle = sun; ctx.fillRect(0, 0, w, h);
      ctx.save();
      ctx.translate(w / 2 + ox * 0.5, h * 0.36 + oy * 0.5); ctx.scale(s, s);
      ctx.font = `800 ${Math.min(w * 0.18, 260)}px system-ui, -apple-system, sans-serif`;
      ctx.textAlign = 'center'; ctx.textBaseline = 'middle';
      ctx.shadowColor = 'rgba(255,255,255,0.7)'; ctx.shadowBlur = 40;
      ctx.fillStyle = `rgba(255,255,255,${0.92 * (1 - p * 0.3)})`;
      ctx.fillText('anikit', 0, 0);
      ctx.restore();
      // Snowy ridge.
      ctx.fillStyle = '#eef1f6';
      ctx.beginPath();
      const base = h * (0.73 + push * 0.4);
      ctx.moveTo(0, base);
      for (let i = 0; i <= 10; i++) ctx.lineTo((w * i) / 10 + ox * 1.5, base - Math.abs(Math.sin(i * 1.7)) * h * 0.08 + oy);
      ctx.lineTo(w, h); ctx.lineTo(0, h); ctx.closePath(); ctx.fill();
    },
  };
}

await ready;
let createMountain = stubMountain;
try { ({ createMountain } = await import('./mountain.js')); } catch { /* the stand-in until mountain.js lands */ }

const mountainCanvas = $('#mountain'), hallCanvas = $('#hall'), handsCanvas = $('#hands');
const mountain = createMountain(mountainCanvas);
const hall = createHall(hallCanvas, $('#overlay'));
const hands = createHands(handsCanvas);

// Resolution is chosen at load; only a real width change re-sizes.
let lastW = 0;
function resize(force) {
  if (!force && innerWidth === lastW) return;
  lastW = innerWidth;
  const w = innerWidth, h = innerHeight, d = devicePixelRatio || 1;
  mountain.resize(w, h, d); hall.resize(w, h, d); hands.resize(w, h, d);
}
resize(true);

// Scroll → a critically damped spring.
let maxScroll = 1;
const measure = () => { maxScroll = Math.max(1, root.scrollHeight - innerHeight); };
measure();
const sp = { p: clamp01(scrollY / maxScroll) };
const SPRING = { stiffness: 55, damping: 2 * Math.sqrt(55) };
function onScroll() {
  if (inHall) return;
  const target = clamp01(scrollY / maxScroll);
  if (target > 0.004) root.classList.add('scrolled'); else root.classList.remove('scrolled');
  spring(sp, { p: target }, SPRING);
}
addEventListener('scroll', onScroll, { passive: true });
addEventListener('resize', () => { measure(); resize(false); });

// Pointer look, spring-smoothed.
const look = { x: 0, y: 0 };
addEventListener('pointermove', (e) => {
  if (inHall || reduced) return;
  spring(look, { x: (e.clientX / innerWidth) * 2 - 1, y: 1 - (e.clientY / innerHeight) * 2 }, { stiffness: 20, damping: 9 });
}, { passive: true });

// The hall: entered at the end of the scroll, left with Esc or the button.
let inHall = false;
const back = $('.back');
function enterHall() {
  if (inHall) return;
  inHall = true;
  root.classList.add('in-hall');
  hallCanvas.style.clipPath = '';
  hall.enter();
}
function exitHall() {
  if (!inHall) return;
  hall.exit();
  if (document.pointerLockElement) document.exitPointerLock();
  inHall = false;
  root.classList.remove('in-hall');
  // Back to just outside the portal; the spring carries the camera back from there.
  const p = 0.86;
  sp.p = 0.94;
  scrollTo({ top: p * maxScroll, behavior: 'instant' });
  spring(sp, { p }, SPRING);
}
back.addEventListener('click', exitHall);
addEventListener('keydown', (e) => { if (e.key === 'Escape' && inHall && !document.pointerLockElement) exitHall(); });

// Map the smoothed progress to each module's inputs.
const io = { p: 0, g: 0, push: 0, k: 0 };
function mapProgress(p) {
  io.p = p;
  io.g = p < 0.15 ? 0 : p < 0.75 ? ((p - 0.15) / 0.6) * 0.75 : 0.75 + clamp01((p - 0.75) / 0.17) * 0.25;
  io.push = clamp01((p - 0.75) / 0.17);
  io.k = io.push;
}

let clipKey = '', hallShown = false, mountainOn = true;
const status = { frames: 0, p: 0, r: 0, inHall: false };
window.__summit = status;
function frame(timeMs, dtMs) {
  const t = timeMs / 1000, dt = Math.min(dtMs, 50) / 1000;
  status.frames++;
  if (inHall) { hall.frame(t, dt); status.inHall = true; return; }
  status.inHall = false;
  mapProgress(clamp01(sp.p));
  status.p = io.p;
  const { portal } = hands.frame(t, dt, io.g, look);
  status.r = portal.r;
  // The mountain stops once the portal covers the whole screen.
  const cover = portal.r > 0 && portal.r >= Math.hypot(Math.max(portal.x, innerWidth - portal.x), Math.max(portal.y, innerHeight - portal.y));
  if (cover === mountainOn) { mountainOn = !cover; mountainCanvas.style.visibility = cover ? 'hidden' : ''; }
  if (mountainOn) mountain.frame(t, dt, io.p, look, io.push);
  // The hall shows only inside the portal.
  if (portal.r > 0.5) {
    const key = cover ? 'none' : `circle(${portal.r.toFixed(1)}px at ${portal.x.toFixed(1)}px ${portal.y.toFixed(1)}px)`;
    if (key !== clipKey) { clipKey = key; hallCanvas.style.clipPath = cover ? '' : key; }
    if (!hallShown) { hallShown = true; hallCanvas.style.visibility = 'visible'; }
    hall.preview(t, dt, io.k);
  } else if (hallShown) { hallShown = false; hallCanvas.style.visibility = ''; clipKey = ''; }
  if (io.p >= 0.95 && sp.p >= 0.95) enterHall();
}

if (reduced) {
  // Reduced motion: no journey, a cross-fade straight into the hall.
  engine.reducedMotion = 'always';
  hallCanvas.style.opacity = '0';
  hallCanvas.style.transition = 'opacity 0.8s ease';
  enterHall();
  requestAnimationFrame(() => { hallCanvas.style.opacity = '1'; });
}
engine.onFrame(frame);
root.classList.add('live');
