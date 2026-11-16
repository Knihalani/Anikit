// Renders the watch off the main thread into a transferred OffscreenCanvas, so scrolling
// never waits for it. Only the latest frame request is drawn.
import { createWatch } from './watch.js';

let watch = null, next = null, scheduled = false, pending = null, lastState = null;
const labels = new Float32Array(14);
const raf = self.requestAnimationFrame ? (f) => self.requestAnimationFrame(f) : (f) => setTimeout(f, 0);
const draw = () => {
  scheduled = false;
  if (!watch || !next) return;
  const s = next; next = null; lastState = s;
  if (s.wantLabels) s.labels = labels;
  const t0 = performance.now();
  watch.render(s);
  const ms = performance.now() - t0;
  raf(() => { if (!next) watch.present(); });
  self.postMessage({ ms, w: watch.width, h: watch.height, tris: watch.triangles, labels: s.wantLabels ? labels : null });
};
const request = () => { if (!scheduled) { scheduled = true; raf(draw); } };
self.onmessage = async (e) => {
  const m = e.data;
  if (m.canvas) {
    pending = createWatch({ width: m.width, height: m.height, canvas: m.canvas });
    watch = await pending;
    self.postMessage({ ready: true });
    request();
    return;
  }
  if (m.resize) {
    await pending; watch.resize(m.resize[0], m.resize[1]);
    // A resize clears the canvas: draw the last frame again at once, so it never flashes empty.
    if (lastState && !m.state) { next = lastState; draw(); }
  }
  if (m.state) { next = m.state; request(); }
};
