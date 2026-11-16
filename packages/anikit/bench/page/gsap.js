import { buildPage } from './common.js';
import { gsap } from 'gsap';
import { ScrollTrigger } from 'gsap/ScrollTrigger';
gsap.registerPlugin(ScrollTrigger);
buildPage();
// SplitText is a Club plugin in older builds; split words by hand (same markup).
const t = document.querySelector('.title');
t.innerHTML = t.textContent.split(' ').map((w) => `<span style="display:inline-block">${w}</span>`).join(' ');
gsap.from(t.children, { opacity: 0, y: 30, duration: 0.8, stagger: 0.05, ease: 'power3.out' });
for (const s of document.querySelectorAll('section.s')) {
  gsap.from(s.querySelectorAll('.card'), { opacity: 0, y: 40, duration: 0.6, stagger: 0.06, scrollTrigger: { trigger: s, start: 'top 80%' } });
}
gsap.to('.pin', { scale: 0.8, opacity: 0.6, ease: 'none', scrollTrigger: { trigger: '.pin', start: 'top top', end: '+=800', pin: true, scrub: true } });
for (const b of document.querySelectorAll('.btn')) {
  const xTo = gsap.quickTo(b, 'x', { duration: 0.4 }), yTo = gsap.quickTo(b, 'y', { duration: 0.4 });
  window.addEventListener('pointermove', (e) => {
    const r = b.getBoundingClientRect();
    const dx = e.clientX - (r.left + r.width / 2), dy = e.clientY - (r.top + r.height / 2);
    const near = Math.abs(dx) < r.width / 2 + 80 && Math.abs(dy) < r.height / 2 + 80;
    xTo(near ? dx * 0.3 : 0); yTo(near ? dy * 0.3 : 0);
  }, { passive: true });
}
const tr = document.querySelector('.track');
tr.innerHTML += tr.innerHTML;
gsap.to(tr, { xPercent: -50, duration: tr.scrollWidth / 2 / 80, ease: 'none', repeat: -1 });
window.__ready = performance.now();
