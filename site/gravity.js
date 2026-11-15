// Gravity mode: every word on screen becomes a rigid body (a chain of circles
// held stiff by distance constraints) in an anikit physics world. Throw them;
// switch it off and they spring back into the layout.
import { world, splitText, set, animate, stagger } from '../packages/anikit/dist/anikit.js';

const TEXT = '.hero-title, .giant, .m-word, .t-line, .hero-copy, .m-desc, .t-copy, .part-copy, .corner, .install-row code, .p-stat p';
const SOLID = '.pill, .card, .hold, .p-num, .t-key, .big-num';

export function gravity({ onChange } = {}) {
  let active = null;

  const visible = (r) => r.width > 2 && r.height > 2 && r.bottom > 0 && r.top < innerHeight && r.right > 0 && r.left < innerWidth;

  function collect() {
    const items = [];
    const taken = new Set();
    // Solid blocks first (they keep their contents).
    for (const el of document.querySelectorAll(SOLID)) {
      if (el.closest('.frame, .cursor') || [...taken].some((t) => t.contains(el))) continue;
      const r = el.getBoundingClientRect();
      if (!visible(r)) continue;
      if (r.width * r.height > innerWidth * innerHeight * 0.18) continue;
      items.push(el);
      taken.add(el);
    }
    for (const el of document.querySelectorAll(TEXT)) {
      if (el.closest('.frame, .cursor') || [...taken].some((t) => t.contains(el))) continue;
      const r = el.getBoundingClientRect();
      if (!visible(r)) continue;
      if (!el.dataset.gsplit) {
        el.dataset.gsplit = '1';
        splitText(el, { type: 'words', wordClass: 'gw' });
      }
      for (const w of el.querySelectorAll('.gw')) if (visible(w.getBoundingClientRect())) items.push(w);
    }
    return items;
  }

  function on() {
    if (active) return;
    const items = collect().slice(0, 700);
    const w = world({ gravity: [0, 2600], bounds: [0, 0, innerWidth, innerHeight], substeps: 4, iterations: 4, restitution: 0.15, wallRestitution: 0.2, friction: 0.35, damping: 0.02 });
    const bodies = [];
    for (const el of items) {
      // Current on-screen box, then remove any existing translate so we know the layout origin.
      const r = el.getBoundingClientRect();
      const h = Math.max(6, Math.min(r.height, 120));
      const n = Math.max(1, Math.min(8, Math.round(r.width / h)));
      const rad = Math.min(h, r.width / n) / 2;
      const cy = r.top + r.height / 2;
      const ids = [];
      for (let k = 0; k < n; k++) {
        const x = n === 1 ? r.left + r.width / 2 : r.left + rad + ((r.width - 2 * rad) * k) / (n - 1);
        ids.push(w.add(x, cy, Math.max(3, rad * 0.98), rad * rad * 0.02));
      }
      for (let k = 0; k + 1 < n; k++) w.link(ids[k], ids[k + 1], { stiffness: 1 });
      for (let k = 0; k + 2 < n; k++) w.link(ids[k], ids[k + 2], { stiffness: 1 });
      if (n > 2) w.link(ids[0], ids[n - 1], { stiffness: 1 });
      bodies.push({ el, ids, ox: r.left + r.width / 2, oy: cy });
      // A little chaos so nothing falls perfectly flat.
      for (const i of ids) w.setVelocity(i, (Math.random() - 0.5) * 160, -Math.random() * 260);
    }
    w.dragging({ hitSlop: 10 });
    w.onStep(() => {
      for (const b of bodies) {
        const a = b.ids[0], z = b.ids[b.ids.length - 1];
        const cx = (w.x(a) + w.x(z)) / 2, cy = (w.y(a) + w.y(z)) / 2;
        const ang = b.ids.length > 1 ? Math.atan2(w.y(z) - w.y(a), w.x(z) - w.x(a)) : 0;
        set(b.el, { x: cx - b.ox, y: cy - b.oy, rotate: (ang * 180) / Math.PI });
      }
    });
    document.documentElement.classList.add('gravity-on');
    // Double-click / double-tap: shockwave.
    const kick = (e) => w.impulse(e.clientX, e.clientY, 260, 2200);
    addEventListener('dblclick', kick);
    active = { w, bodies, kick };
    onChange?.(true, bodies.length);
  }

  function off() {
    if (!active) return;
    const { w, bodies, kick } = active;
    removeEventListener('dblclick', kick);
    w.destroy();
    animate(bodies.map((b) => b.el), { x: 0, y: 0, rotate: 0 }, {
      duration: 1.1,
      ease: 'spring(bounce: 0.28, duration: 0.9)',
      delay: stagger(0.004, { from: 'center' }),
      onComplete: () => document.documentElement.classList.remove('gravity-on'),
    });
    active = null;
    onChange?.(false, 0);
  }

  return {
    toggle: () => (active ? off() : on()),
    on,
    off,
    get active() {
      return !!active;
    },
  };
}
