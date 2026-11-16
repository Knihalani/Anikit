// Black-hole renderer throughput (WebAssembly, in Node), 1280×800 unless
// --size WxH. Reports steady frames (time + azimuth animating: the hero) and
// camera-moving frames (distance changing every frame: the scroll journey).
//   node scripts/bench-blackhole.mjs [--size 1920x1080] [--scale 0.75]
import { performance } from 'node:perf_hooks';
const A = await import(process.env.DIST ? new URL(`file://${process.env.DIST.replace(/\/$/, '')}/anikit.js`).href : '../dist/anikit.js');
await A.ready;
const arg = (k, d) => { const i = process.argv.indexOf(k); return i > 0 ? process.argv[i + 1] : d; };
const [W, H] = arg('--size', '1280x800').split('x').map(Number);
const scale = Number(arg('--scale', '1'));
const median = (a) => a.slice().sort((x, y) => x - y)[a.length >> 1];
const shots = {
  'edge-on 84°, 30M (physical)': ['physical', { distance: 30, inclination: 84, fov: 34 }],
  'edge-on 84°, 30M (cinematic)': ['cinematic', { distance: 30, inclination: 84, fov: 34 }],
  'EHT 17°, 40M': ['physical', { distance: 40, inclination: 17, fov: 30 }],
  'close 10M, fov 70': ['physical', { distance: 10, inclination: 80, fov: 70 }],
};
console.log(`blackHole ${W}×${H} @ scale ${scale} (${((W * H * scale * scale) / 1e6).toFixed(2)} Mpx)`);
for (const [name, [mode, cam]] of Object.entries(shots)) {
  const bh = A.blackHole({ width: W, height: H, mode, scale });
  let t = 0;
  for (let i = 0; i < 120; i++) bh.render({ ...cam, time: (t += 1 / 60), azimuth: t * 3, jet: 0.3 }); // warm up (V8 tiers wasm up lazily)
  const steady = [];
  for (let i = 0; i < 120; i++) {
    const t0 = performance.now();
    bh.render({ ...cam, time: (t += 1 / 60), azimuth: t * 3, jet: 0.3 });
    steady.push(performance.now() - t0);
  }
  const moving = [];
  for (let i = 0; i < 20; i++) {
    const t0 = performance.now();
    bh.render({ ...cam, distance: cam.distance - i * 0.05, time: (t += 1 / 60) });
    moving.push(performance.now() - t0);
  }
  const s = bh.stats;
  console.log(`  ${name.padEnd(30)} steady ${median(steady).toFixed(2)} ms   camera moving ${median(moving).toFixed(2)} ms   samples ${s.samples.join('/')}  stars ${s.stars}`);
  bh.destroy();
}
