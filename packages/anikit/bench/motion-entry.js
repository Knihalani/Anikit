// Bundled by scripts/bench-motion.mjs: one case per page (?lib=&case=&n=).
import * as ak from '../src/index';
import { animate as mAnimate, stagger as mStagger, scroll as mScroll } from 'motion';
import { gsap } from 'gsap';
import { ScrollTrigger as GST } from 'gsap/ScrollTrigger';

const q = new URLSearchParams(location.search);
const lib = q.get('lib');
const kase = q.get('case');
const n = Number(q.get('n'));
const stage = document.getElementById('stage');
const els = [];
for (let i = 0; i < n; i++) {
  const d = document.createElement(kase === 'scroll' ? 'section' : 'div');
  if (kase === 'scroll') {
    const b = document.createElement('div');
    b.className = 'b';
    d.appendChild(b);
    els.push([d, b]);
  } else {
    d.className = 'box';
    els.push(d);
  }
  stage.appendChild(d);
}
gsap.registerPlugin(GST);

window.setup = async () => {
  await ak.ready;
  ak.setWarnings(false);
  void document.body.offsetWidth;
  const t0 = performance.now();
  if (kase === 'stagger') {
    if (lib === 'anikit') ak.animate(els, { opacity: [0, 1], y: [20, 0] }, { duration: 1.2, delay: ak.stagger(0.001), ease: 'outCubic' });
    else if (lib === 'gsap') gsap.fromTo(els, { opacity: 0, y: 20 }, { opacity: 1, y: 0, duration: 1.2, stagger: 0.001, ease: 'power3.out' });
    else mAnimate(els, { opacity: [0, 1], y: [20, 0] }, { duration: 1.2, delay: mStagger(0.001), ease: 'easeOut' });
  } else if (kase === 'spring') {
    // A follower: every element retargeted to a new x every 100 ms.
    let k = 0;
    const go = () => {
      k++;
      const x = (k % 2) * 200;
      if (lib === 'anikit') for (const el of els) ak.spring(el, { x }, { stiffness: 170, damping: 12 });
      else if (lib === 'gsap') for (const el of els) gsap.to(el, { x, duration: 0.8, ease: 'elastic.out(1, 0.5)', overwrite: true });
      else for (const el of els) mAnimate(el, { x }, { type: 'spring', stiffness: 170, damping: 12 });
    };
    go();
    window.__iv = setInterval(go, 100);
  } else if (kase === 'scroll') {
    for (const [s, b] of els) {
      if (lib === 'anikit') ak.scroll(ak.animate(b, { x: [0, 300], opacity: [1, 0.5] }, { ease: 'linear', autoplay: false }), { target: s });
      else if (lib === 'anikit-compositor') ak.scroll(ak.compositor(b, { x: [0, 300], opacity: [1, 0.5] }, { ease: 'linear' }), { target: s });
      else if (lib === 'gsap') gsap.fromTo(b, { x: 0, opacity: 1 }, { x: 300, opacity: 0.5, ease: 'none', scrollTrigger: { trigger: s, start: 'top bottom', end: 'bottom top', scrub: true } });
      else mScroll(mAnimate(b, { x: [0, 300], opacity: [1, 0.5] }, { ease: 'linear' }), { target: s, offset: ['start end', 'end start'] });
    }
  }
  const create = performance.now() - t0;
  void document.body.offsetWidth;
  return create;
};
// Drive the page (scroll case) for `ms`, one step per frame.
window.drive = (ms) => new Promise((res) => {
  const t0 = performance.now();
  let y = 0, dir = 1;
  const max = document.documentElement.scrollHeight - innerHeight;
  const f = () => {
    if (kase === 'scroll') {
      y += dir * 23;
      if (y > max || y < 0) { dir = -dir; y += 2 * dir * 23; }
      window.scrollTo(0, y);
    }
    if (performance.now() - t0 < ms) requestAnimationFrame(f);
    else res();
  };
  requestAnimationFrame(f);
});
window.check = () => {
  const s = new Set();
  for (const e of els.slice(0, 50)) s.add(getComputedStyle(kase === 'scroll' ? e[1] : e).transform);
  // Moved: transforms differ between elements, or (all at one x) are not identity.
  const first = [...s][0];
  return s.size >= 2 || (first && first !== 'none' && first !== 'matrix(1, 0, 0, 1, 0, 0)') ? 2 : s.size;
};
