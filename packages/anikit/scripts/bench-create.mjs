// Set-up cost benchmark in Node (no DOM): how long it takes to *create*
// animations (first frame included), how much memory they keep per target,
// and a few per-frame paths bench.mjs does not cover (staggered tweens,
// timelines, motion values). Plain-object targets, so it measures the JS
// layer and the wasm calls, not browser style writes.
// Usage: node --expose-gc scripts/bench-create.mjs [N]   (default 10000)
// Without --expose-gc the "retained" column is skipped.
const t0 = performance.now();
const ak = await import('../dist/anikit.js');
const tImport = performance.now() - t0;
await ak.ready;
const tReady = performance.now() - t0;
const { animate, timeline, stagger, motionValue, derived, engine, setWarnings } = ak;
import gsap from 'gsap';

setWarnings(false);
engine.manual(true);
engine.lagSmoothing(0);
gsap.ticker.lagSmoothing(0);
gsap.ticker.sleep();

const N = Number(process.argv[2]) || 10000;
const RUNS = +(process.env.RUNS || 7);
const WARM = +(process.env.WARM || 5);
const median = (a) => a.sort((x, y) => x - y)[a.length >> 1];
const objs = (n, make) => Array.from({ length: n }, make);
const gc = globalThis.gc;


const akFrame = () => engine.tick(16);
// From GSAP's own root time: waking its ticker (first tween) can move the root time base.
const gsFrame = () => gsap.updateRoot(gsap.globalTimeline.time() + 0.016);
const kill = (a) => a.kill();
const killAll = (as) => { for (const a of as) a.kill(); };
const gsapKill = () => gsap.globalTimeline.clear();

/** Fail loudly if a case animated nothing (e.g. a timeline that never started). */
function moved(t, key, from, label) {
  if (t[key] === from) throw new Error(`bench-create: "${label}" did not animate (${key} still ${from})`);
}

/**
 * Time `setup` plus the first frame, RUNS times after WARM untimed runs. The
 * first frame is included because GSAP defers most of its set-up (reading
 * and parsing start values) to the first render; anikit applies its first
 * values then too. Returns [ms, retained bytes per target or NaN].
 */
function create(label, targets, key, setup, cleanup, first) {
  const ts = [];
  for (let r = 0; r < WARM + RUNS; r++) {
    for (const t of targets) t[key] = 0;
    const t = performance.now();
    const a = setup();
    first();
    const dt = performance.now() - t;
    first(); // (untimed) check that the animation runs
    moved(targets[0], key, 0, label);
    cleanup(a);
    engine.tick(16); // let killed work be reclaimed like a real frame would
    if (r >= WARM) ts.push(dt);
  }
  // Retained heap: one more (untimed) run, heap with it alive minus heap
  // after it was killed and collected.
  let bytes = NaN;
  if (gc) {
    let a = setup();
    first();
    gc(); gc();
    const h1 = process.memoryUsage().heapUsed;
    cleanup(a);
    a = null;
    engine.tick(16);
    engine.tick(16);
    gc(); gc();
    bytes = (h1 - process.memoryUsage().heapUsed) / targets.length;
  }
  return [median(ts), bytes];
}
/** Per-frame cost of an already created set-up. */
function frames(label, targets, key, setup, frame, cleanup, count = 60) {
  const ts = [];
  for (let r = 0; r < 3; r++) {
    const a = setup();
    for (let i = 0; i < 100; i++) frame(a);
    const v = targets[0][key];
    const t = performance.now();
    for (let i = 0; i < count; i++) frame(a);
    ts.push((performance.now() - t) / count);
    moved(targets[0], key, v, label);
    cleanup(a);
  }
  return median(ts);
}

const row = (name, ak, gs) => {
  const [a, ab] = Array.isArray(ak) ? ak : [ak, NaN];
  const [g, gb] = Array.isArray(gs) ? gs : [gs, NaN];
  const kb = (b) => (Number.isNaN(b) ? '' : ` ${(b / 1024).toFixed(2).padStart(6)} KB`);
  console.log(name.padEnd(44), a.toFixed(3).padStart(9), g === undefined ? '        —' : g.toFixed(3).padStart(9),
    g === undefined ? '       ' : `  ${(g / a).toFixed(2)}x`.padEnd(8), kb(ab), kb(gb));
};

console.log(`startup: import ${tImport.toFixed(1)} ms, ready (wasm instantiated) ${tReady.toFixed(1)} ms after import start`);
console.log(`N = ${N}; creation: median of ${RUNS} runs after ${WARM} warm-up runs, first frame included; frames: ms per frame`);
console.log(`retained: heap kept per target right after creation (--expose-gc), anikit then GSAP\n`);
console.log('case'.padEnd(44), 'Anikit'.padStart(9), 'GSAP'.padStart(9), '  ratio   retained');

{
  const t = objs(N, () => ({ x: 0, y: 0, o: 1 }));
  const l = 'create: one animate(), 3 numbers';
  row(l, create(l, t, 'x', () => animate(t, { x: 500, y: 300, o: 0 }, { duration: 1, ease: 'outCubic' }), kill, akFrame),
    create(l, t, 'x', () => gsap.to(t, { x: 500, y: 300, o: 0, duration: 1, ease: 'power2.out' }), gsapKill, gsFrame));
}
{
  const t = objs(N, () => ({ x: 0, y: 0, o: 1 }));
  const l = `create: ${N} animate() calls, 3 numbers`;
  row(l, create(l, t, 'x', () => t.map((o) => animate(o, { x: 500, y: 300, o: 0 }, { duration: 1 })), killAll, akFrame),
    create(l, t, 'x', () => t.map((o) => gsap.to(o, { x: 500, y: 300, o: 0, duration: 1 })), gsapKill, gsFrame));
}
{
  const t = objs(N, () => ({ x: 0 }));
  const l = 'create: one animate(), stagger delay';
  row(l, create(l, t, 'x', () => animate(t, { x: 100 }, { duration: 1, delay: stagger(0.00001) }), kill, akFrame),
    create(l, t, 'x', () => gsap.to(t, { x: 100, duration: 1, stagger: 0.00001 }), gsapKill, gsFrame));
}
{
  const t = objs(N, () => ({ x: 0 }));
  const l = 'create: one animate(), 8 keyframes';
  row(l, create(l, t, 'x', () => animate(t, { x: [0, 50, 10, 80, 20, 90, 30, 100] }, { duration: 1 }), kill, akFrame),
    create(l, t, 'x', () => gsap.to(t, { keyframes: { x: [0, 50, 10, 80, 20, 90, 30, 100] }, duration: 1 }), gsapKill, gsFrame));
}
{
  const t = objs(N, () => ({ c: '#ff0000' }));
  const l = 'create: one animate(), colour string';
  const reset = () => { for (const o of t) o.c = '#ff0000'; };
  const mark = () => { if (t[0].c !== '#ff0000') t[0].n = 1; };
  row(l, create(l, t, 'n', () => { reset(); return animate(t, { c: '#0000ff' }, { duration: 1 }); }, kill, () => { akFrame(); mark(); }),
    create(l, t, 'n', () => { reset(); return gsap.to(t, { c: '#0000ff', duration: 1 }); }, gsapKill, () => { gsFrame(); mark(); }));
}
{
  const M = Math.max(1, N / 10 | 0);
  const t = objs(M, () => ({ x: 0, y: 0 }));
  const l = `create: timeline of ${M} steps`;
  row(l, create(l, t, 'x', () => {
    const tl = timeline({ autoplay: false });
    for (let i = 0; i < M; i++) tl.add(t[i], { x: 100, y: 50 }, { duration: 0.1 }, i * 1e-6);
    return tl.play();
  }, kill, akFrame), create(l, t, 'x', () => {
    const tl = gsap.timeline();
    for (let i = 0; i < M; i++) tl.to(t[i], { x: 100, y: 50, duration: 0.1 }, i * 1e-6);
    return tl;
  }, (tl) => { tl.kill(); gsapKill(); }, gsFrame));
}

{
  // Re-triggering: a new animate() on the same targets and properties takes
  // every one of them over from the running one (GSAP: overwrite 'auto').
  const t = objs(N, () => ({ x: 0, y: 0 }));
  const ak = [], gs = [];
  for (let r = 0; r < WARM + RUNS; r++) {
    const a = animate(t, { x: 100, y: 5 }, { duration: 1 });
    akFrame();
    let s = performance.now();
    const b = animate(t, { x: 200, y: 9 }, { duration: 1 });
    akFrame();
    if (r >= WARM) ak.push(performance.now() - s);
    a.kill(); b.kill(); akFrame();
    gsap.to(t, { x: 100, y: 5, duration: 1 });
    gsFrame();
    s = performance.now();
    gsap.to(t, { x: 200, y: 9, duration: 1, overwrite: 'auto' });
    gsFrame();
    if (r >= WARM) gs.push(performance.now() - s);
    gsapKill();
  }
  row('create: animate() taking over a running one', median(ak), median(gs));
}
{
  // Teardown: kill() of a long timeline (each child is killed too).
  const M = Math.max(1, N / 2 | 0);
  const t = objs(M, () => ({ x: 0 }));
  const ak = [], gs = [];
  for (let r = 0; r < WARM + RUNS; r++) {
    const tl = timeline({ autoplay: false });
    for (let i = 0; i < M; i++) tl.add(t[i], { x: 100 }, { duration: 0.1 }, i * 1e-6);
    tl.play();
    akFrame();
    let s = performance.now();
    tl.kill();
    if (r >= WARM) ak.push(performance.now() - s);
    akFrame();
    const g = gsap.timeline();
    for (let i = 0; i < M; i++) g.to(t[i], { x: 100, duration: 0.1 }, i * 1e-6);
    gsFrame();
    s = performance.now();
    g.kill();
    if (r >= WARM) gs.push(performance.now() - s);
    gsapKill();
  }
  row(`kill: timeline of ${M} steps`, median(ak), median(gs));
}

console.log('');
{
  const t = objs(N, () => ({ x: 0 }));
  const l = `frame: ${N} targets, stagger delay`;
  row(l, frames(l, t, 'x', () => animate(t, { x: 100 }, { duration: 1000, delay: stagger(0.001) }), akFrame, kill),
    frames(l, t, 'x', () => gsap.to(t, { x: 100, duration: 1000, stagger: 0.001 }), gsFrame, gsapKill));
}
{
  const t = objs(N, () => ({ x: 0, y: 0 }));
  const M = Math.max(1, N / 10 | 0);
  const l = `frame: timeline of ${M} steps x 10 targets`;
  row(l, frames(l, t, 'x', () => {
    const tl = timeline({ repeat: -1, autoplay: false });
    for (let i = 0; i < M; i++) tl.add(t.slice(i * 10, i * 10 + 10), { x: 100, y: 50 }, { duration: 10 }, 0);
    return tl.play();
  }, akFrame, kill), frames(l, t, 'x', () => {
    const tl = gsap.timeline({ repeat: -1 });
    for (let i = 0; i < M; i++) tl.to(t.slice(i * 10, i * 10 + 10), { x: 100, y: 50, duration: 10 }, 0);
    return tl;
  }, gsFrame, gsapKill));
}
{
  const M = Math.max(1, N / 10 | 0);
  const sink = [{ v: 0 }];
  const l = `frame: motionValue -> ${M} derived + listeners`;
  row(l, frames(l, sink, 'v', () => {
    const v = motionValue(0);
    const offs = [];
    for (let i = 0; i < M; i++) {
      const d = derived([v], (a) => a * i);
      offs.push(d.on((x) => { sink[0].v = x; }));
    }
    return { v, offs };
  }, (s) => s.v.set(s.v.get() + 1), (s) => { for (const o of s.offs) o(); }));
}
