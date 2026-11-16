// Summit: the page. One loop (Anikit's frame clock) drives the mountain, the hall seen through the portal
// and the hands on top: in ONE shared WebGL2 context once every module supports it (CONTRACT.md),
// otherwise on three stacked canvases.
// Scroll progress is smoothed with an Anikit spring and mapped to p, g, push and k (see CONTRACT.md).
import { ready, spring, engine, animate } from '../../../packages/anikit/dist/anikit.lean.js';
import * as HandsM from './hands.js';
import * as HallM from './hall.js';

const root = document.documentElement;
const $ = (s) => document.querySelector(s);
const reduced = matchMedia('(prefers-reduced-motion: reduce)').matches;
const clamp01 = (v) => (v < 0 ? 0 : v > 1 ? 1 : v);

// A stand-in for mountain.js until it lands: sky, sun and "anikit" in the clouds, on a 2D canvas.
function stubMountain(canvas) {
  const ctx = canvas.getContext('2d');
  let w = 1, h = 1, d = 1, lastKey = '';
  return {
    resize(cssW, cssH, dpr) { w = cssW; h = cssH; d = Math.min(dpr || 1, 2); canvas.width = Math.round(w * d); canvas.height = Math.round(h * d); lastKey = ''; },
    frame(t, dt, p, look, push) {
      const key = `${p.toFixed(3)}|${look.x.toFixed(3)}|${look.y.toFixed(3)}|${push.toFixed(3)}`;
      if (key === lastKey) return;
      lastKey = key;
      ctx.setTransform(d, 0, 0, d, 0, 0);
      const s = 1 + push * 0.6, ox = -look.x * 14, oy = look.y * 8;
      const g = ctx.createLinearGradient(0, 0, 0, h);
      g.addColorStop(0, '#6f8fc6'); g.addColorStop(0.4, '#a7b2cf'); g.addColorStop(0.64, '#f2c99e'); g.addColorStop(0.72, '#fbe3c4');
      ctx.fillStyle = g; ctx.fillRect(0, 0, w, h);
      const sx = w * 0.72 + ox, sy = h * 0.66 + oy;
      const sun = ctx.createRadialGradient(sx, sy, 0, sx, sy, h * 0.35);
      sun.addColorStop(0, 'rgba(255,240,210,1)'); sun.addColorStop(0.08, 'rgba(255,214,150,0.9)'); sun.addColorStop(1, 'rgba(255,214,150,0)');
      ctx.fillStyle = sun; ctx.fillRect(0, 0, w, h);
      ctx.save();
      ctx.translate(w / 2 + ox * 0.5, h * 0.36 + oy * 0.5); ctx.scale(s, s);
      ctx.font = `800 ${Math.min(w * 0.18, 260)}px system-ui, -apple-system, sans-serif`;
      ctx.textAlign = 'center'; ctx.textBaseline = 'middle';
      ctx.shadowColor = 'rgba(255,255,255,0.7)'; ctx.shadowBlur = 40;
      ctx.fillStyle = `rgba(255,255,255,${0.92 * (1 - p * 0.3)})`;
      ctx.fillText('anikit', 0, 0);
      ctx.restore();
      // Snowy ridge.
      ctx.fillStyle = '#eef1f6';
      ctx.beginPath();
      const base = h * (0.73 + push * 0.4);
      ctx.moveTo(0, base);
      for (let i = 0; i <= 10; i++) ctx.lineTo((w * i) / 10 + ox * 1.5, base - Math.abs(Math.sin(i * 1.7)) * h * 0.08 + oy);
      ctx.lineTo(w, h); ctx.lineTo(0, h); ctx.closePath(); ctx.fill();
    },
  };
}

await ready;
let MountainM = { createMountain: stubMountain };
try { MountainM = await import('./mountain.js'); } catch { /* the stand-in until mountain.js lands */ }

// ?perf shows real frame times per phase; ?perf=no-sparks,no-hall,no-mountain,no-hands switches passes off to rank their cost.
const perfArg = new URLSearchParams(location.search).get('perf');
const urlParams = new URLSearchParams(location.search);
const OFF = {};
if (perfArg) for (const k of perfArg.split(',')) if (k.startsWith('no-')) OFF[k.slice(3)] = true;
globalThis.__summitOff = OFF;

const params = new URLSearchParams(location.search);
const SHARED = !!(MountainM.SHARED_GL && HallM.SHARED_GL && HandsM.SHARED_GL) && !params.has('legacy');
const mountainCanvas = $('#mountain'), hallCanvas = $('#hall'), handsCanvas = $('#hands'), glCanvas = $('#gl');
let gl = null;
if (SHARED) {
  root.classList.add('shared');
  gl = glCanvas.getContext('webgl2', { antialias: false, alpha: false, depth: true, stencil: false, premultipliedAlpha: false, powerPreference: 'high-performance', desynchronized: true });
}
const mountain = MountainM.createMountain(SHARED ? gl : mountainCanvas);
const hall = HallM.createHall(SHARED ? gl : hallCanvas, $('#overlay'));
const hands = HandsM.createHands(SHARED ? gl : handsCanvas);
const modules = [mountain, hall, hands];

// Resolution is chosen at load; only a real width change (or the quality governor's last tier) re-sizes.
let lastW = 0, renderScale = 1;
function resize(force) {
  if (!force && innerWidth === lastW) return;
  lastW = innerWidth;
  const w = innerWidth, h = innerHeight;
  if (SHARED) {
    const d = Math.min(devicePixelRatio || 1, 2) * renderScale;
    glCanvas.width = Math.max(2, Math.round(w * d)); glCanvas.height = Math.max(2, Math.round(h * d));
    const real = glCanvas.width / w;
    for (const m of modules) m.resize(w, h, real);
  } else {
    const d = devicePixelRatio || 1;
    mountain.resize(w, h, d); hall.resize(w, h, d); hands.resize(w, h, d);
  }
}
resize(true);

// Scroll → a critically damped spring.
let maxScroll = 1;
const measure = () => { maxScroll = Math.max(1, root.scrollHeight - innerHeight); };
measure();
const sp = { p: clamp01(scrollY / maxScroll) };
const SPRING = { stiffness: 170, damping: 2 * Math.sqrt(170) };
function onScroll() {
  if (inHall) return;
  const target = clamp01(scrollY / maxScroll);
  if (target > 0.004) root.classList.add('scrolled'); else root.classList.remove('scrolled');
  spring(sp, { p: target }, SPRING);
}
addEventListener('scroll', onScroll, { passive: true });
addEventListener('resize', () => { measure(); resize(false); });

// Pointer look, spring-smoothed.
const look = { x: 0, y: 0 };
addEventListener('pointermove', (e) => {
  if (inHall || reduced) return;
  spring(look, { x: (e.clientX / innerWidth) * 2 - 1, y: 1 - (e.clientY / innerHeight) * 2 }, { stiffness: 20, damping: 9 });
}, { passive: true });

// The hall: entered at the end of the scroll, left with Esc or the button.
let inHall = false, armed = true;
const back = $('.back');
function enterHall() {
  if (inHall) return;
  inHall = true;
  root.classList.add('in-hall');
  hallCanvas.style.clipPath = '';
  hall.enter();
}
function exitHall() {
  if (!inHall) return;
  hall.exit();
  if (document.pointerLockElement) document.exitPointerLock();
  inHall = false;
  armed = false;
  root.classList.remove('in-hall');
  // Back to just outside the portal; the spring carries the camera back from there.
  const p = 0.7;
  scrollTo({ top: p * maxScroll, behavior: 'instant' });
  spring(sp, { p }, SPRING);
}
back.addEventListener('click', exitHall);
addEventListener('keydown', (e) => { if (e.key === 'Escape' && inHall && !document.pointerLockElement) exitHall(); });

// Map the smoothed progress to each module's inputs.
const io = { p: 0, g: 0, push: 0, k: 0 };
function mapProgress(p) {
  io.p = p;
  // A little scroll triggers the formation (it then plays on its own clock); later scroll carries you through.
  io.g = p < 0.08 ? 0 : p < 0.35 ? 0.1 + ((p - 0.08) / 0.27) * 0.64 : 0.75 + clamp01((p - 0.35) / 0.55) * 0.25;
}


// Frame-time readout (?perf): p50 / p95 / max over the last 120 frames, per phase. Allocation-free.
const perf = perfArg !== null ? (() => {
  const el = document.createElement('pre');
  el.style.cssText = 'position:fixed;right:8px;top:48px;z-index:9;margin:0;padding:8px 10px;font:12px/1.35 ui-monospace,monospace;background:rgba(0,0,0,.6);color:#fff;border-radius:8px;pointer-events:none;white-space:pre';
  document.body.append(el);
  const names = ['summit', 'portal', 'hall'];
  const buf = [new Float32Array(120), new Float32Array(120), new Float32Array(120)];
  const len = [0, 0, 0], at = [0, 0, 0], scratch = new Float32Array(120);
  let n = 0;
  return { el, names, buf, len, at, scratch, get n() { return n; }, set n(v) { n = v; } };
})() : null;
function perfTick(dtMs, phase) {
  const P = perf;
  P.buf[phase][P.at[phase]] = dtMs; P.at[phase] = (P.at[phase] + 1) % 120; P.len[phase] = Math.min(120, P.len[phase] + 1);
  if (++P.n % 30) return;
  let txt = 'frame ms   p50   p95   max';
  for (let i = 0; i < 3; i++) {
    const L = P.len[i];
    if (!L) continue;
    const s = P.scratch.subarray(0, L);
    s.set(P.buf[i].subarray(0, L)); s.sort();
    txt += `\n${P.names[i].padEnd(8)} ${s[Math.floor(L * 0.5)].toFixed(1).padStart(5)} ${s[Math.min(L - 1, Math.floor(L * 0.95))].toFixed(1).padStart(5)} ${s[L - 1].toFixed(1).padStart(5)}`;
  }
  if (gpuT) txt += `\ngpu ms   mountain ${gpuT.ms[0].toFixed(2)}  hall ${gpuT.ms[1].toFixed(2)}  hands ${gpuT.ms[2].toFixed(2)}`;
  txt += `\n${SHARED ? 'shared context' : 'stacked canvases'} · tier ${gov.tier}`;
  P.el.textContent = txt;
  window.__summitPerf = txt;
}

let clipKey = '', hallShown = false, mountainOn = true;
const hallRect = { x: 0, y: 0, w: 1, h: 1 };
const status = { frames: 0, p: 0, r: 0, inHall: false };
window.__summit = status;
const portalDev = { x: 0, y: 0, r: 0 };
// GPU time per module (?perf, shared context, EXT_disjoint_timer_query_webgl2): a small pool of queries per
// module, read back a few frames later; smoothed ms shown in the overlay.
const gpuT = (() => {
  if (!SHARED || perfArg === null) return null;
  const ext = gl.getExtension('EXT_disjoint_timer_query_webgl2');
  if (!ext) return null;
  const names = ['mountain', 'hall', 'hands'], N = 4;
  const pools = names.map(() => Array.from({ length: N }, () => ({ q: gl.createQuery(), busy: false })));
  const ms = new Float32Array(3), at = new Int32Array(3);
  let open = -1;
  return {
    names, ms,
    begin(i) {
      const slot = pools[i][at[i]];
      if (slot.busy) return;            // still waiting for this one: skip timing this frame
      gl.beginQuery(ext.TIME_ELAPSED_EXT, slot.q); slot.busy = true; open = i;
    },
    end(i) { if (open !== i) return; gl.endQuery(ext.TIME_ELAPSED_EXT); open = -1; at[i] = (at[i] + 1) % N; },
    poll() {
      const disjoint = gl.getParameter(ext.GPU_DISJOINT_EXT);
      for (let i = 0; i < 3; i++) for (const slot of pools[i]) {
        if (!slot.busy || !gl.getQueryParameter(slot.q, gl.QUERY_RESULT_AVAILABLE)) continue;
        const ns = gl.getQueryParameter(slot.q, gl.QUERY_RESULT);
        slot.busy = false;
        if (!disjoint) ms[i] = ms[i] * 0.9 + (ns / 1e6) * 0.1;
      }
    },
  };
})();
function frameShared(t, dt, dtMs) {
  const W = innerWidth, H = innerHeight, k = glCanvas.width / W;
  gl.bindFramebuffer(gl.FRAMEBUFFER, null);
  gl.viewport(0, 0, glCanvas.width, glCanvas.height);
  if (inHall) { hall.frame(t, dt); status.inHall = true; governor(dtMs); if (perf) perfTick(dtMs, 2); return; }
  status.inHall = false;
  mapProgress(clamp01(sp.p));
  status.p = io.p;
  const portal = hands.update(t, dt, io.g, look).portal;
  status.r = portal.r; status.ring = portal.ring;
  io.push = io.k = portal.step;
  const cover = portal.r > 0 && portal.r >= Math.hypot(Math.max(portal.x, W - portal.x), Math.max(portal.y, H - portal.y));
  // 1. The mountain (only while some of it shows).
  if (gpuT) { gpuT.poll(); gpuT.begin(0); }
  if (!cover && !OFF.mountain) mountain.frame(t, dt, io.p, look, io.push, portal);
  else if (!cover) { gl.clearColor(0.7, 0.75, 0.8, 1); gl.clear(gl.COLOR_BUFFER_BIT); }
  if (gpuT) { gpuT.end(0); gpuT.begin(1); }
  // 2. The hall, inside the portal only.
  if (portal.r > 0.5 && !OFF.hall) {
    const x0 = Math.max(0, portal.x - portal.r), y0 = Math.max(0, portal.y - portal.r);
    hallRect.x = Math.floor(x0 * k); hallRect.y = Math.floor(y0 * k);
    hallRect.w = Math.ceil((Math.min(W, portal.x + portal.r) - x0) * k); hallRect.h = Math.ceil((Math.min(H, portal.y + portal.r) - y0) * k);
    portalDev.x = portal.x * k; portalDev.y = portal.y * k; portalDev.r = cover ? 1e6 : portal.r * k;
    hall.preview(t, dt, io.k, hallRect, portalDev);
  }
  gl.viewport(0, 0, glCanvas.width, glCanvas.height);
  if (gpuT) { gpuT.end(1); gpuT.begin(2); }
  // 3. Hands and sparks on top.
  hands.render();
  if (gpuT) gpuT.end(2);
  if (portal.step < 0.9) armed = true;
  if (armed && portal.step >= 0.97 && io.p >= 0.85) enterHall();
  governor(dtMs);
  if (perf) perfTick(dtMs, portal.r > 0 || portal.ring > 0 ? 1 : 0);
}

// Quality governor: real frame times, p90 over 90 frames; step down above 17.5 ms (at most every 3 s),
// back up only after 10 s under 12 ms. Tier 1-2: modules' setQuality; tier 3: render scale 0.85.
const gov = { buf: new Float32Array(90), scratch: new Float32Array(90), n: 0, at: 0, tier: 0, last: 0, good: 0 };
function governor(dtMs) {
  if (dtMs <= 0 || dtMs > 250) return;     // tab switches and stalls say nothing about steady cost
  gov.buf[gov.at] = dtMs; gov.at = (gov.at + 1) % 90; gov.n = Math.min(90, gov.n + 1);
  if (gov.n < 90 || gov.at % 15) return;
  gov.scratch.set(gov.buf); gov.scratch.sort();
  const p90 = gov.scratch[80];
  const now = performance.now();
  if (p90 > 17.5) { gov.good = 0; if (gov.tier < 3 && now - gov.last > 3000) setTier(gov.tier + 1, p90); }
  else if (p90 < 12) { if (!gov.good) gov.good = now; else if (gov.tier > 0 && now - gov.good > 10000 && now - gov.last > 10000) { setTier(gov.tier - 1, p90); gov.good = now; } }
  else gov.good = 0;
}
function setTier(tier, p90) {
  gov.tier = tier; gov.last = performance.now(); gov.n = 0;
  for (const m of modules) if (m.setQuality) m.setQuality(Math.min(2, tier));
  const scale = tier >= 3 ? 0.85 : 1;
  if (SHARED && scale !== renderScale) { renderScale = scale; resize(true); }
  if (perf) console.info(`anikit summit: quality tier ${tier} (p90 ${p90.toFixed(1)} ms)`);
  status.tier = tier;
}

function frame(timeMs, dtMs) {
  const t = timeMs / 1000, dt = Math.min(dtMs, 50) / 1000;
  status.frames++;
  if (SHARED) { frameShared(t, dt, dtMs); return; }
  if (inHall) { hall.frame(t, dt); status.inHall = true; governor(dtMs); if (perf) perfTick(dtMs, 2); return; }
  status.inHall = false;
  mapProgress(clamp01(sp.p));
  status.p = io.p;
  const { portal } = hands.frame(t, dt, io.g, look);
  status.r = portal.r; status.ring = portal.ring;
  io.push = io.k = portal.step;
  // Never two full-cost scenes: once the portal covers ~70% of the screen the mountain keeps its last frame;
  // once it covers everything the canvas is hidden.
  const W = innerWidth, H = innerHeight;
  const cover = portal.r > 0 && portal.r >= Math.hypot(Math.max(portal.x, W - portal.x), Math.max(portal.y, H - portal.y));
  if (cover === mountainOn) { mountainOn = !cover; mountainCanvas.style.visibility = cover ? 'hidden' : ''; }
  const rr = Math.max(portal.r, portal.ring || 0);
  const coverage = (Math.PI * rr * rr) / (W * H);
  if (mountainOn && coverage < 0.7 && !OFF.mountain) mountain.frame(t, dt, io.p, look, io.push, portal);
  // The hall shows only inside the portal.
  if (portal.r > 0.5) {
    const key = cover ? 'none' : `circle(${portal.r.toFixed(1)}px at ${portal.x.toFixed(1)}px ${portal.y.toFixed(1)}px)`;
    if (key !== clipKey) { clipKey = key; hallCanvas.style.clipPath = cover ? '' : key; }
    if (!hallShown) { hallShown = true; hallCanvas.style.visibility = 'visible'; }
    // The hall renders only its portal's box (device px of the hall canvas).
    const ks = hallCanvas.width / W;
    const x0 = Math.max(0, portal.x - portal.r), y0 = Math.max(0, portal.y - portal.r);
    hallRect.x = Math.floor(x0 * ks); hallRect.y = Math.floor(y0 * ks);
    hallRect.w = Math.ceil((Math.min(W, portal.x + portal.r) - x0) * ks); hallRect.h = Math.ceil((Math.min(H, portal.y + portal.r) - y0) * ks);
    if (!OFF.hall) hall.preview(t, dt, io.k, cover ? undefined : hallRect);
  } else if (hallShown) { hallShown = false; hallCanvas.style.visibility = ''; clipKey = ''; }
  // After leaving, the hall re-arms only once the camera has backed out of the doorway.
  if (portal.step < 0.9) armed = true;
  if (armed && portal.step >= 0.97 && io.p >= 0.85) enterHall();
  governor(dtMs);
  if (perf) perfTick(dtMs, inHall ? 2 : portal.r > 0 || portal.ring > 0 ? 1 : 0);
}

if (reduced) {
  // Reduced motion: no journey, a cross-fade straight into the hall.
  engine.reducedMotion = 'always';
  const fadeEl = SHARED ? glCanvas : hallCanvas;
  fadeEl.style.opacity = '0';
  fadeEl.style.transition = 'opacity 0.8s ease';
  enterHall();
  requestAnimationFrame(() => { fadeEl.style.opacity = '1'; });
}
engine.onFrame(frame);
root.classList.add('live');
