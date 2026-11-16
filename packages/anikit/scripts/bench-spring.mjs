// spring() and inertia() in Node on plain objects (engine + JS write path).
// Cases: creating N springs (3 props each), the frame of N running springs,
// a pointer follower (every frame retargets all N springs, then ticks), and
// N inertia() flings with bounds and snapping. Median of RUNS, ms (lower is better).
// With --expose-gc it also prints the heap kept per retarget call while the
// springs are still moving.
// Usage: node --expose-gc scripts/bench-spring.mjs [N...]   (DIST=path/to/dist to A/B)
import { pathToFileURL, fileURLToPath } from 'node:url';
import { resolve } from 'node:path';

const dist = resolve(process.env.DIST || fileURLToPath(new URL('../dist', import.meta.url)));
const { spring, inertia, engine, ready, setWarnings } = await import(pathToFileURL(resolve(dist, 'anikit.js')).href);
await ready;
setWarnings(false);
engine.manual(true);
engine.lagSmoothing(0);

const sizes = process.argv.slice(2).map(Number).filter(Boolean);
const NS = sizes.length ? sizes : [1000, 10000];
const RUNS = +(process.env.RUNS || 5);
const FRAMES = 60;
const kept = new Map(); // bytes still on the heap per retarget call (with --expose-gc)
const median = (a) => a.sort((x, y) => x - y)[a.length >> 1];
const step = () => engine.tick(1000 / 60);
const make = (n) => Array.from({ length: n }, (_, i) => ({ x: i, y: 0, r: 0 }));
const check = (objs) => {
  if (!objs.some((o) => o.x !== 0 && o.y !== 0)) throw new Error('nothing moved');
};

const cases = {
  'create (3 props)': (n) => {
    const objs = make(n);
    const s = performance.now();
    for (let i = 0; i < n; i++) spring(objs[i], { x: 500, y: 300, r: 90 }, { stiffness: 120, damping: 8 });
    step();
    const ms = performance.now() - s;
    check(objs);
    for (const o of objs) spring(o, { x: o.x, y: o.y, r: o.r }).stop();
    return ms;
  },
  'frame, N running': (n) => {
    const objs = make(n);
    const c = objs.map((o) => spring(o, { x: 500, y: 300, r: 90 }, { stiffness: 20, damping: 1 }));
    for (let i = 0; i < 30; i++) step();
    const s = performance.now();
    for (let i = 0; i < FRAMES; i++) step();
    const ms = (performance.now() - s) / FRAMES;
    check(objs);
    for (const k of c) k.stop();
    return ms;
  },
  'retarget all + frame': (n) => {
    const objs = make(n);
    for (const o of objs) spring(o, { x: 0, y: 0 }, { stiffness: 300, damping: 25 });
    let px = 0;
    const frame = () => {
      px += 7;
      for (let i = 0; i < n; i++) spring(objs[i], { x: px, y: px * 0.5 }, { stiffness: 300, damping: 25 });
      step();
    };
    for (let i = 0; i < 30; i++) frame();
    globalThis.gc?.();
    const heap0 = process.memoryUsage().heapUsed;
    const s = performance.now();
    for (let i = 0; i < FRAMES; i++) frame();
    const ms = (performance.now() - s) / FRAMES;
    globalThis.gc?.();
    if (globalThis.gc) kept.set(n, (process.memoryUsage().heapUsed - heap0) / (n * FRAMES));
    check(objs);
    for (const o of objs) spring(o, { x: o.x, y: o.y }).stop();
    return ms;
  },
  'inertia create + 10 frames': (n) => {
    const objs = make(n);
    const s = performance.now();
    for (let i = 0; i < n; i++) inertia(objs[i], { x: { velocity: 800, min: 0, max: 600, snap: 100 }, y: { velocity: -500 } });
    for (let i = 0; i < 10; i++) step();
    const ms = performance.now() - s;
    check(objs);
    for (const o of objs) spring(o, { x: o.x, y: o.y }).stop();
    return ms;
  },
};
const gsCases = {}; // GSAP's InertiaPlugin does not register under Node

console.log(`median of ${RUNS} runs, ms (lower is better); create cases include their frames\n`);
console.log('case'.padEnd(27), 'N'.padStart(6), 'Anikit'.padStart(9), 'GSAP'.padStart(9));
for (const [name, fn] of Object.entries(cases)) {
  for (const n of NS) {
    fn(Math.min(n, 1000));
    fn(Math.min(n, 1000)); // warm-up
    if (gsCases[name]) gsCases[name](Math.min(n, 1000));
    const a = [];
    const g = [];
    for (let r = 0; r < RUNS; r++) {
      a.push(fn(n));
      if (gsCases[name]) g.push(gsCases[name](n));
    }
    console.log(name.padEnd(27), String(n).padStart(6), median(a).toFixed(3).padStart(9), (g.length ? median(g).toFixed(3) : '—').padStart(9));
  }
}
for (const [n, b] of kept) console.log(`retarget: heap kept per call while moving, N=${n}: ${Math.max(0, b).toFixed(0)} bytes`);
