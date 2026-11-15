// Software 3D renderer throughput (WebAssembly, in Node): 1920×1080 scenes
// with per-pixel lit chrome, grid lines and instanced towers.
//   node scripts/bench-raster.mjs
import { performance } from 'node:perf_hooks';
const A = await import('../dist/anikit.js');
await A.ready;
const W = 1920, H = 1080;
const r3 = A.renderer3d({ width: W, height: H });
const cam = A.scene3d({ fov: 50, width: W, height: H, near: 8, far: 40000, camera: { x: 300, y: 250, z: 1350 }, target: { x: 0, y: -50, z: -200 } });
const knot = A.mesh.torusKnot({ radius: 300, spread: 120, tube: 58, segments: 600, sides: 28 });
const cube = A.mesh.cube();
const env = { background: '#04050a', fog: [1800, 5200], sky: [0.05, 0.07, 0.16], horizon: [0.9, 0.5, 0.95], ground: [0.01, 0.01, 0.02], strips: [2.2, 2.3, 2.6] };
const towers = A.instances(900, (i) => { const c = i % 30, r = Math.floor(i / 30), h = 40 + ((i * 7919) % 97) * 4; return { x: (c - 15) * 120, y: -500 + h / 2, z: -r * 120 + 600, sx: 80, sy: h, sz: 80, color: [0.04, 0.045, 0.06] }; });
const swarm = A.instances(6000, (i) => ({ x: ((i * 7919) % 2000) - 1000, y: ((i * 104729) % 1200) - 600, z: ((i * 1299709) % 1600) - 800, rx: i, ry: i * 3, scale: 60, color: [0.9, 0.5, 0.3] }));
const scenes = {
  'knot (chrome, lines, pulses)': [{ mesh: knot, instances: A.instances(1, () => ({ rx: 20, ry: 30, color: [0.06, 0.06, 0.08] })), material: { metal: 0.9, iridescence: 0.25, specular: 1.4, shininess: 120, lines: 1.2, grid: [0, 6], pulse: 2.5, pulseCount: 3 } }],
  '900 towers (edge lines)': [{ mesh: cube, instances: towers, material: { lines: 1.6, grid: [1, 1], lineWidth: 0.035, pulse: 1.5, pulseDir: [0, 0, 0.0015] } }],
  '6000 overlapping cubes': [{ mesh: cube, instances: swarm }],
};
for (const [name, items] of Object.entries(scenes)) {
  for (let i = 0; i < 15; i++) r3.render(cam, items, env);
  const n = 30, t = performance.now();
  for (let i = 0; i < n; i++) r3.render(cam, items, env);
  console.log(`${name.padEnd(32)} ${((performance.now() - t) / n).toFixed(1).padStart(6)} ms/frame  (${r3.triangles} triangles)`);
}
