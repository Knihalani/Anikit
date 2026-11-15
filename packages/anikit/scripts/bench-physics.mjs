// Physics benchmark: Anikit world (Rust/WASM) vs matter.js (JS), same scene:
// N circles dropped into a box, settled, then timed. Usage: node scripts/bench-physics.mjs
import { world, ready, engine } from '../dist/anikit.js';
import Matter from 'matter-js';

await ready;
engine.manual(true);
const SIDE = 1600;
const median = (a) => a.sort((x, y) => x - y)[a.length >> 1];

function layout(n) {
  const cols = Math.ceil(Math.sqrt(n));
  const sp = SIDE / cols;
  return Array.from({ length: n }, (_, i) => ({ x: (i % cols) * sp + sp / 2, y: Math.floor(i / cols) * sp + sp / 2, r: sp * 0.3 }));
}

function timeIt(step, settle, frames) {
  for (let i = 0; i < settle; i++) step();
  const runs = [];
  for (let r = 0; r < 3; r++) {
    const t = performance.now();
    for (let i = 0; i < frames; i++) step();
    runs.push((performance.now() - t) / frames);
  }
  return median(runs);
}

console.log('bodies   Anikit (WASM)   matter.js     speed-up');
for (const n of [250, 1000, 2000, 5000]) {
  const bodies = layout(n);
  // Anikit
  const w = world({ bounds: [0, 0, SIDE, SIDE], autoStep: false });
  w.addMany(bodies.map((b) => ({ ...b, mass: 1 })));
  const ak = timeIt(() => w.step(1 / 60), 120, 60);
  w.destroy();
  // matter.js
  const me = Matter.Engine.create();
  me.gravity.y = 1;
  const walls = [
    Matter.Bodies.rectangle(SIDE / 2, -50, SIDE * 2, 100, { isStatic: true }),
    Matter.Bodies.rectangle(SIDE / 2, SIDE + 50, SIDE * 2, 100, { isStatic: true }),
    Matter.Bodies.rectangle(-50, SIDE / 2, 100, SIDE * 2, { isStatic: true }),
    Matter.Bodies.rectangle(SIDE + 50, SIDE / 2, 100, SIDE * 2, { isStatic: true }),
  ];
  Matter.Composite.add(me.world, walls);
  Matter.Composite.add(me.world, bodies.map((b) => Matter.Bodies.circle(b.x, b.y, b.r, { restitution: 0.2, friction: 0.05 })));
  const frames = n >= 5000 ? 10 : 30;
  const mj = timeIt(() => Matter.Engine.update(me, 1000 / 60), n >= 5000 ? 30 : 120, frames);
  console.log(`${String(n).padStart(6)}   ${ak.toFixed(3).padStart(9)} ms   ${mj.toFixed(3).padStart(9)} ms   ${(mj / ak).toFixed(1).padStart(6)}x`);
}
