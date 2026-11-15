// Frame-time benchmark in Node (no DOM — measures engine + JS write path):
// N plain objects animated by Anikit (Rust/WASM core) and GSAP 3 (pure JS).
// Each case runs in a fresh engine; reported value is the median over runs.
// Usage: node scripts/bench.mjs [N...]
import { animate, engine, ready } from '../dist/anikit.js';
import gsap from 'gsap';

await ready;
engine.manual(true);
engine.lagSmoothing(0);
gsap.ticker.lagSmoothing(0);
gsap.ticker.sleep();
let gsapClock = 0; // GSAP's root time must only move forward across cases

const sizes = process.argv.slice(2).map(Number).filter(Boolean);
const NS = sizes.length ? sizes : [1000, 10000, 50000];
const FRAMES = 60;
const RUNS = 5;
const median = (a) => a.sort((x, y) => x - y)[a.length >> 1];

function time(frame) {
  for (let i = 0; i < 5; i++) frame();
  const t = performance.now();
  for (let i = 0; i < FRAMES; i++) frame();
  return (performance.now() - t) / FRAMES;
}

const CASES = {
  '3 numbers, outCubic': {
    make: () => ({ x: 0, y: 0, o: 1 }),
    ak: (t) => animate(t, { x: 500, y: 300, o: 0 }, { duration: 1000, ease: 'outCubic' }),
    gs: (t) => gsap.to(t, { x: 500, y: 300, o: 0, duration: 1000, ease: 'power2.out' }),
  },
  'colour string': {
    make: () => ({ c: '#ff0000' }),
    ak: (t) => animate(t, { c: '#0000ff' }, { duration: 1000, ease: 'linear' }),
    gs: (t) => gsap.to(t, { c: '#0000ff', duration: 1000, ease: 'none' }),
  },
  '8 keyframes': {
    make: () => ({ x: 0 }),
    ak: (t) => animate(t, { x: [0, 50, 10, 80, 20, 90, 30, 100] }, { duration: 1000, ease: 'inOutSine' }),
    gs: (t) => gsap.to(t, { keyframes: { x: [0, 50, 10, 80, 20, 90, 30, 100] }, duration: 1000, ease: 'none' }),
  },
};

console.log(`median of ${RUNS} runs x ${FRAMES} frames, ms per frame (lower is better)\n`);
console.log('case'.padEnd(22), 'N'.padStart(7), 'Anikit'.padStart(9), 'GSAP'.padStart(9), '  ratio');
for (const [name, c] of Object.entries(CASES)) {
  for (const n of NS) {
    const ak = [];
    const gs = [];
    for (let r = 0; r < RUNS; r++) {
      const a = Array.from({ length: n }, c.make);
      const anim = c.ak(a);
      ak.push(time(() => engine.tick(16)));
      anim.kill();
      const g = Array.from({ length: n }, c.make);
      c.gs(g);
      gs.push(time(() => gsap.updateRoot((gsapClock += 16) / 1000)));
      gsap.globalTimeline.clear();
    }
    const A = median(ak);
    const G = median(gs);
    console.log(name.padEnd(22), String(n).padStart(7), A.toFixed(3).padStart(9), G.toFixed(3).padStart(9), `  ${(G / A).toFixed(2)}x`);
  }
}

// Instanced batches: no per-instance JS work at all.
import { batch } from '../dist/anikit.js';
console.log('\nbatch() — instanced, output is one Float32Array (no per-object JS writes)');
for (const n of NS) {
  const runs = [];
  for (let r = 0; r < RUNS; r++) {
    const b = batch(n, { x: (i) => [0, i], y: [0, 300], o: [1, 0] }, { duration: 1000, ease: 'outCubic' });
    runs.push(time(() => engine.tick(16)));
    b.kill();
  }
  console.log('3 channels'.padEnd(22), String(n).padStart(7), median(runs).toFixed(3).padStart(9));
}
