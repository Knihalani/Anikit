// The story: one scrubbed timeline moves the night along the page (storm, colour,
// band distance, corona, camera), a spring-smoothed pointer adds parallax, and each
// demo panel is scrubbed in, held and scrubbed out by its own section.
import {
  ready, engine, animate, timeline, set, scroll, smoothScroll, motionValue, splitText, stagger, inView,
} from '../../../packages/anikit/dist/anikit.lean.js';
import { sky } from './sky.js';

const $ = (s, r = document) => r.querySelector(s);
const $$ = (s, r = document) => Array.from(r.querySelectorAll(s));
const reduced = matchMedia('(prefers-reduced-motion: reduce)').matches;
const fine = matchMedia('(hover: hover) and (pointer: fine)').matches;
const small = innerWidth < 700;

await ready;
engine.reducedMotion = 'user';
engine.reducedMotionMode = 'fade';
if (!reduced) smoothScroll({ smooth: 0.9 });

// ---------------------------------------------------------------------------
// The night, scrubbed by the page scroll.
// ---------------------------------------------------------------------------
const P = sky.params;
const story = { storm: 0.42, red: 0.2, bandZ: 900, corona: 0, night: 0.15, exposure: 1.7, camY: 4.2, camZ: 0, pitch: small ? 0.2 : 0.17, fov: small ? 0.95 : 1.15 };
const night = timeline({ autoplay: false, defaults: { ease: 'inOutSine' } })
  // hero → first demo: drop towards the ice, the first band brightens
  .add(story, { camY: 2.4, pitch: small ? 0.17 : 0.13, storm: 0.5, bandZ: 760, night: 0.4, camZ: 160 }, { duration: 1 }, 0)
  // the demos: the storm builds, the red upper layer appears, the bands come closer
  .add(story, { storm: 1, bandZ: 420, night: 1, exposure: 1.95, camY: 1.3, camZ: 1100 }, { duration: 7 }, 1)
  .add(story, { red: 0.85 }, { duration: 5, ease: 'inQuad' }, 3)
  // the end: the bands pass overhead and the camera looks up into the corona
  .add(story, { bandZ: -60, corona: 1, camY: 1.1, camZ: 1260, exposure: 1.55, fov: small ? 0.75 : 0.9 }, { duration: 2, ease: 'inOutSine' }, 8)
  .add(story, { pitch: 1.42 }, { duration: 1.7, ease: 'inOutSine' }, 8.15);
scroll(night, { smooth: reduced ? 0 : 0.08 });

// Pointer parallax: springs with momentum, composed with the story camera.
const yaw = motionValue(0), tilt = motionValue(0), side = motionValue(0);
if (fine && !reduced) {
  addEventListener('pointermove', (e) => {
    const nx = e.clientX / innerWidth - 0.5, ny = e.clientY / innerHeight - 0.5;
    yaw.spring(nx * 0.14, { stiffness: 30, damping: 12 });
    tilt.spring(-ny * 0.06, { stiffness: 30, damping: 12 });
    side.spring(nx * 3, { stiffness: 18, damping: 9 });
  });
}

let drift = 0, lastAur = '';
const root = document.documentElement;
engine.onFrame((_t, dt) => {
  if (!reduced) drift += (Math.min(dt, 50) / 1000) * 1.2; // the camera glides even at rest
  // a slow, breathing sway so the shot is never frozen
  const sway = reduced ? 0 : Math.sin(P.time * 0.07) * 0.035;
  P.storm = story.storm; P.red = story.red; P.bandZ = story.bandZ; P.corona = story.corona;
  P.night = story.night; P.exposure = story.exposure; P.camY = story.camY; P.fov = story.fov;
  P.camZ = story.camZ + drift;
  P.camX = side.get();
  P.yaw = yaw.get() + sway * (1 - story.corona);
  P.pitch = story.pitch + tilt.get();
  // The page is lit by the sky: the aurora colour feeds a CSS variable.
  const a = sky.ambient ? sky.ambient() : [0.2, 1, 0.45];
  const k = 255 / Math.max(a[0], a[1], a[2]);
  const aur = `${Math.round(a[0] * k)}, ${Math.round(a[1] * k)}, ${Math.round(a[2] * k)}`;
  if (aur !== lastAur) { lastAur = aur; root.style.setProperty('--aur', aur); }
});

// ---------------------------------------------------------------------------
// Hero: letters rise once, then the whole block drifts away with the scroll.
// ---------------------------------------------------------------------------
const title = splitText('#title', { type: 'chars', charClass: 'ch' });
timeline({ defaults: { ease: 'outExpo' } })
  .add(title.chars, { yPercent: [70, 0], opacity: [0, 1], rotateX: [-60, 0] }, { duration: 1.6, delay: stagger(0.07, { from: 'center' }) }, 0.15)
  .add('#lede', { opacity: [0, 1], y: [14, 0] }, { duration: 1.4 }, 0.7)
  .add('#cue', { opacity: [0, 0.8] }, { duration: 1.2 }, 1.4)
  .add('.nav', { opacity: [0, 1] }, { duration: 1.2 }, 1.2);
scroll(animate('.hero-in', { opacity: [1, 0], y: [0, -70], scale: [1, 0.96] }, { ease: 'inSine', autoplay: false }), { target: $('.hero'), offset: ['start start', 'end start'] });
if (!reduced) animate('#cue', { y: [0, 6, 0] }, { duration: 2.8, repeat: Infinity, ease: 'inOutSine', delay: 2.6 });

// ---------------------------------------------------------------------------
// Demo panels: in over the first quarter of the section, held, out over the last.
// ---------------------------------------------------------------------------
for (const s of $$('.demo')) {
  const panel = $('.panel', s);
  panel.style.setProperty('--hue', s.dataset.hue);
  const tl = timeline({ autoplay: false })
    .add(panel, { opacity: [0, 1], y: [90, 0], scale: [0.95, 1], rotateX: [8, 0] }, { duration: 1, ease: 'outCubic' }, 0)
    .add(panel, { opacity: [1, 0], y: [0, -90], scale: [1, 0.97] }, { duration: 1, ease: 'inCubic' }, 3);
  scroll(tl, { target: s, offset: ['start 35%', 'end 65%'], smooth: reduced ? 0 : 0.06 });
}
const inst = timeline({ autoplay: false })
  .add('#install-core', { opacity: [0, 1], y: [60, 0] }, { duration: 1, ease: 'outCubic' }, 1.1)
  .add('#install-core', { y: [0, 0] }, { duration: 0.4 }, 2.1);
scroll(inst, { target: $('#install'), offset: ['start end', 'end end'] });

// Demos load on approach.
let demos = null;
const load = () => (demos ??= import('./demos.js').then((m) => m.start({ reduced, fine, small })));
inView('#physics', () => { load(); }, { amount: 0, margin: '0px 0px 60% 0px' });
if (scrollY > innerHeight * 0.4) load();
setTimeout(load, 2500);

window.__aurora = { story, engine };
