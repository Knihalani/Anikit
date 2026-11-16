// The descent. Scroll position → camera depth through a critically damped
// spring; the depth drives the water shader, the life layer, the gauge and
// the light that reveals each demo.
import {
  engine, ready, smoothScroll, scrollProgress, motionValue, scroll, timeline, splitText, stagger, set, animate,
} from '../../../packages/anikit/dist/anikit.lean.js';
import { life } from './life.js';
import { start } from './demos.js';

const $ = (s, r = document) => r.querySelector(s);
const $$ = (s, r = document) => Array.from(r.querySelectorAll(s));
const reduced = matchMedia('(prefers-reduced-motion: reduce)').matches;
const fine = matchMedia('(pointer: fine)').matches;
const small = Math.min(innerWidth, innerHeight) < 700;

engine.reducedMotion = 'user';
engine.reducedMotionMode = 'fade';

// The water drew its first frame from boot.js before this module loaded.
const sea = window.__sea || null;

await ready;

// Camera depth: page progress, followed by a critically damped spring.
if (!reduced && fine) smoothScroll({ smooth: 0.9 });
const page = scrollProgress({ offset: ['start start', 'end end'] });
const depth = motionValue(page.get());
const K = 26;
page.on((v) => (reduced ? depth.set(v) : depth.spring(v, { stiffness: K, damping: 2 * Math.sqrt(K), restDelta: 0.00005 })));

// Pointer: a soft spring for the camera's sway.
const sway = { x: motionValue(0), y: motionValue(0) };
if (fine && !reduced) addEventListener('pointermove', (e) => {
  sway.x.spring((e.clientX / innerWidth) * 2 - 1, { stiffness: 30, damping: 12 });
  sway.y.spring(1 - (e.clientY / innerHeight) * 2, { stiffness: 30, damping: 12 });
}, { passive: true });

const bio = life($('#life'), { reduced, small });
const gauge = $('#gauge'), bar = $('#gauge-bar');
const MAX_M = 4000;
let hidden = document.hidden;
document.addEventListener('visibilitychange', () => (hidden = document.hidden));
let lastG = '';
engine.onFrame((_t, dt) => {
  if (hidden) return;
  const d = Math.min(1, Math.max(0, depth.get()));
  if (sea) {
    sea.state.depth = d;
    sea.state.speed = depth.velocity * 18;
    sea.state.t += reduced ? 0 : dt / 1000;
    sea.state.ptr[0] = sway.x.get(); sea.state.ptr[1] = sway.y.get();
    sea.adapt(dt);
    sea.draw();
  }
  bio.frame(reduced ? 0 : dt, d);
  const m = Math.round(Math.pow(d, 1.6) * MAX_M);
  const g = m.toLocaleString('en-US') + ' m';
  if (g !== lastG) { gauge.textContent = g; lastG = g; }
  set(bar, { y: d * innerHeight * 0.22 - 2 });
});

// Hero: the title surfaces, letter by letter.
{
  const { chars } = splitText('#title', { type: 'chars', charClass: 'ch' });
  set(chars, { opacity: 0, yPercent: 35 });
  set(['.lede', '#cue'], { opacity: 0 });
  timeline({ defaults: { ease: 'outCubic' } })
    .add(chars, { opacity: [0, 1], yPercent: [35, 0] }, { duration: 1.6, delay: stagger(0.08, { from: 'center' }) }, 0.15)
    .add('.lede', { opacity: [0, 1], y: [12, 0] }, { duration: 1.4 }, 0.7)
    .add('#cue', { opacity: [0, 0.8] }, { duration: 1.6 }, 1.4);
  scroll(animate('.hero', { opacity: [1, 0], y: [0, -80] }, { ease: 'linear', autoplay: false }), { target: $('.hero'), offset: ['start start', 'end start'], smooth: 0.15 });
}

// Each demo is lit from within as you reach it, holds, then goes dark again.
// The reveal is scrubbed by scroll over the first 40% of its pinned range
// (about 50vh), the fade over the last 38%.
for (const sec of $$('.stop')) {
  sec.style.setProperty('--hue', sec.dataset.hue);
  const panel = $('.panel', sec), lantern = $('.lantern', sec);
  const last = sec.classList.contains('end');
  const tl = timeline({ autoplay: false, defaults: { ease: 'inOutSine' } })
    .add(lantern, { opacity: [0, 1], scale: [0.55, 1] }, { duration: 0.42 }, 0)
    .add(panel, { opacity: [0, 1], y: [36, 0] }, { duration: 0.34 }, 0.06);
  if (!last) tl
    .add(lantern, { opacity: [1, 0], scale: [1, 1.25] }, { duration: 0.38 }, 0.62)
    .add(panel, { opacity: [1, 0], y: [0, -36] }, { duration: 0.32 }, 0.62);
  else tl.add(lantern, { opacity: [1, 1] }, { duration: 0.58 }, 0.42);
  scroll(tl, { target: sec, offset: ['start start', 'end end'], smooth: 0.12 });
}

start({ reduced, fine, small });
