// Bundled by scripts/bench-dom.mjs. Animates N absolutely positioned boxes with
// x, y, rotation and opacity (infinite yoyo) using the library in ?lib=.
import * as ak from '../src/index';
import { gsap } from 'gsap';
import { animate as motionAnimate } from 'motion';

const q = new URLSearchParams(location.search);
// Startup: when the core is usable, in ms since navigation (bundle parse,
// base64 decode and WebAssembly compile included). Printed by MANUAL=1.
window.readyAt = NaN;
// Time of the animate() call on the N elements (creation, reads of current values).
window.createMs = NaN;
ak.ready.then(() => (window.readyAt = performance.now()));
import { setTypedOM } from '../src/props';
setTypedOM(q.get('typed') === '1');
const lib = q.get('lib');
const n = Number(q.get('n') || 1000);
const stage = document.getElementById('stage');
const els = [];
for (let i = 0; i < n; i++) {
  const d = document.createElement('div');
  d.className = 'b';
  d.style.left = (i % 50) * 20 + 'px';
  d.style.top = Math.floor(i / 50) * 20 + 'px';
  stage.appendChild(d);
  els.push(d);
}
const delay = (i) => (i % 50) * 0.01;
window.start = async () => {
  if (lib === 'anikit' || lib === 'anikit-typed') {
    await ak.ready;
    const t0 = performance.now();
    ak.animate(els, { x: [0, 300], y: [0, 200], rotate: [0, 360], opacity: [1, 0.3] }, { duration: 2, ease: 'inOutQuad', repeat: Infinity, yoyo: true, delay: (_e, i) => delay(i) });
    window.createMs = performance.now() - t0;
  } else if (lib === 'anikit-compositor') {
    await ak.ready;
    ak.compositor(els, { x: [0, 300], y: [0, 200], rotate: [0, 360], opacity: [1, 0.3] }, { duration: 2, ease: 'inOutQuad', repeat: Infinity, yoyo: true, delay: (_e, i) => delay(i) });
  } else if (lib === 'gsap') {
    gsap.to(els, { x: 300, y: 200, rotation: 360, opacity: 0.3, duration: 2, ease: 'power1.inOut', repeat: -1, yoyo: true, stagger: { each: 0.01, from: 0, grid: 'auto' }, delay: 0 });
  } else if (lib === 'motion') {
    motionAnimate(els, { x: [0, 300], y: [0, 200], rotate: [0, 360], opacity: [1, 0.3] }, { duration: 2, ease: 'easeInOut', repeat: Infinity, repeatType: 'reverse', delay: (i) => delay(i) });
  }
};

// Scripted frames for `MANUAL=1 node scripts/bench-dom.mjs`: time only the
// library's frame (anikit: engine.tick = core + writes; GSAP: updateRoot),
// with style recalc forced outside the timed region. Returns ms per frame.
window.manualFrames = (frames) => {
  let tick;
  if (lib === 'gsap') {
    gsap.ticker.lagSmoothing(0);
    gsap.ticker.sleep();
    tick = () => gsap.updateRoot(gsap.globalTimeline.time() + 0.016);
  } else if (lib === 'anikit' || lib === 'anikit-typed') {
    ak.engine.manual(true);
    tick = () => ak.engine.tick(16);
  } else return NaN;
  const run = (n) => {
    let acc = 0;
    for (let f = 0; f < n; f++) {
      const t = performance.now();
      tick();
      acc += performance.now() - t;
      void document.body.offsetWidth;
    }
    return acc / n;
  };
  run(100); // warm-up (V8 tiers JS and wasm up lazily)
  const ts = [run(frames), run(frames), run(frames)].sort((a, b) => a - b);
  return ts[1];
};
