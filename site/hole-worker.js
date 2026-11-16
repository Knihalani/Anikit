// Renders the black hole off the main thread into a transferred OffscreenCanvas, so
// scrolling never waits for it. Only the latest frame request is drawn.
import { blackHole } from './hole.js';

let bh = null, next = null, scheduled = false;
const raf = self.requestAnimationFrame ? (f) => self.requestAnimationFrame(f) : (f) => setTimeout(f, 0);
const draw = () => {
  scheduled = false;
  if (!bh || !next) return;
  const opts = next; next = null;
  const t0 = performance.now();
  bh.render(opts);
  self.postMessage({ ms: performance.now() - t0, w: bh.width, h: bh.height });
};
self.onmessage = (e) => {
  const m = e.data;
  if (m.canvas) bh = blackHole({ width: m.width, height: m.height, canvas: m.canvas });
  else if (m.resize) bh?.resize(m.resize[0], m.resize[1]);
  if (m.opts) { next = m.opts; if (!scheduled) { scheduled = true; raf(draw); } }
};
