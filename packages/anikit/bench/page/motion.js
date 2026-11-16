import { buildPage } from './common.js';
import { animate, stagger, inView, scroll } from 'motion';
buildPage();
const t = document.querySelector('.title');
t.innerHTML = t.textContent.split(' ').map((w) => `<span style="display:inline-block">${w}</span>`).join(' ');
animate(t.children, { opacity: [0, 1], y: [30, 0] }, { duration: 0.8, delay: stagger(0.05), ease: 'easeOut' });
for (const s of document.querySelectorAll('section.s')) {
  const cards = s.querySelectorAll('.card');
  for (const c of cards) c.style.opacity = '0';
  inView(s, () => { animate(cards, { opacity: [0, 1], y: [40, 0] }, { duration: 0.6, delay: stagger(0.06) }); }, { amount: 0.2 });
}
// No pinning in Motion: a sticky wrapper, scrubbed with scroll().
const wrap = document.querySelector('.pinwrap');
wrap.style.height = 'calc(100vh + 800px)';
const pin = document.querySelector('.pin');
pin.style.position = 'sticky';
pin.style.top = '0';
scroll(animate(pin, { scale: [1, 0.8], opacity: [1, 0.6] }, { ease: 'linear' }), { target: wrap, offset: ['start start', 'end end'] });
for (const b of document.querySelectorAll('.btn')) {
  window.addEventListener('pointermove', (e) => {
    const r = b.getBoundingClientRect();
    const dx = e.clientX - (r.left + r.width / 2), dy = e.clientY - (r.top + r.height / 2);
    const near = Math.abs(dx) < r.width / 2 + 80 && Math.abs(dy) < r.height / 2 + 80;
    animate(b, { x: near ? dx * 0.3 : 0, y: near ? dy * 0.3 : 0 }, { type: 'spring', stiffness: 220, damping: 16 });
  }, { passive: true });
}
const tr = document.querySelector('.track');
tr.innerHTML += tr.innerHTML;
animate(tr, { x: [0, -tr.scrollWidth / 2] }, { duration: tr.scrollWidth / 2 / 80, ease: 'linear', repeat: Infinity });
window.__ready = performance.now();
