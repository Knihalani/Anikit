// Bundled by scripts/bench-scroll.mjs. N sections, each with a box animated
// by a scroll-linked animation: ?mode=trigger (scrollTrigger, scrub: true) or
// ?mode=scroll (scroll(), progress over the section); trigger-smooth
// (scrub: 0.5) and scroll-smooth (smooth: 0.3) follow through a spring that
// every scroll event retargets, with one engine frame per scroll step
// (outside the timed region). ?mode=gsap is GSAP's ScrollTrigger with
// scrub: true; it defers its work to the next animation frame, so the timed
// region also runs its ScrollTrigger.update().
import * as ak from '../src/index';
import { gsap } from 'gsap';
import { ScrollTrigger as GST } from 'gsap/ScrollTrigger';

const q = new URLSearchParams(location.search);
const n = Number(q.get('n') || 50);
const mode = q.get('mode') || 'trigger';
const smooth = mode.endsWith('-smooth');
const isGsap = mode === 'gsap';
const stage = document.getElementById('stage');
const boxes = [];
for (let i = 0; i < n; i++) {
  const s = document.createElement('section');
  const b = document.createElement('div');
  b.className = 'b';
  s.appendChild(b);
  stage.appendChild(s);
  boxes.push([s, b]);
}
window.start = async () => {
  await ak.ready;
  ak.engine.manual(true);
  if (isGsap) {
    gsap.registerPlugin(GST);
    gsap.ticker.sleep();
    for (const [s, b] of boxes) {
      gsap.fromTo(b, { x: 0, rotation: 0, opacity: 1 }, { x: 300, rotation: 90, opacity: 0.5, ease: 'none', scrollTrigger: { trigger: s, start: 'top bottom', end: 'bottom top', scrub: true } });
    }
    GST.refresh();
    void document.body.offsetWidth;
    return;
  }
  for (const [s, b] of boxes) {
    const anim = ak.animate(b, { x: [0, 300], rotate: [0, 90], opacity: [1, 0.5] }, { duration: 1, ease: 'linear', autoplay: false });
    if (mode === 'trigger') ak.scrollTrigger({ trigger: s, start: 'top bottom', end: 'bottom top', scrub: true, animation: anim });
    else if (mode === 'trigger-smooth') ak.scrollTrigger({ trigger: s, start: 'top bottom', end: 'bottom top', scrub: 0.5, animation: anim });
    else if (mode === 'scroll-smooth') ak.scroll(anim, { target: s, offset: ['start end', 'end start'], smooth: 0.3 });
    else ak.scroll(anim, { target: s, offset: ['start end', 'end start'] });
  }
  void document.body.offsetWidth;
};
// Scroll by `step` px per frame and time the synchronous 'scroll' handling
// (what the page's listeners do: reads, progress, writes, any layout they
// force). Layout left for the browser is flushed outside the timed region.
window.run = (frames, step) => {
  const max = document.documentElement.scrollHeight - innerHeight;
  let y = 0, dir = 1, acc = 0;
  const one = () => {
    y += dir * step;
    if (y > max || y < 0) { dir = -dir; y += 2 * dir * step; }
    window.scrollTo(0, y);
    void document.body.offsetWidth;
    const t = performance.now();
    window.dispatchEvent(new Event('scroll'));
    if (isGsap) GST.update();
    acc += performance.now() - t;
    if (smooth) ak.engine.tick(16);
    void document.body.offsetWidth;
  };
  for (let i = 0; i < 60; i++) one();
  acc = 0;
  const seen = new Set();
  for (let i = 0; i < frames; i++) {
    one();
    if (i % 10 === 0) for (const [, b] of boxes) seen.add(b.style.transform);
  }
  // Something must have moved, or the numbers mean nothing.
  if (seen.size < 3) throw new Error('bench-scroll: the boxes did not animate');
  return acc / frames;
};
