// Reference numbers for tests/test_raster.py, from the real packages/anikit/src/shatter.ts
// with its wasm and mesh imports stubbed (wasm.ts, mesh.ts here; mesh.json is
// raster.torus(100, 30, 12, 6) dumped from Python). To regenerate, from this directory:
//   sed "s#from './wasm'#from './wasm.ts'#; s#from './mesh'#from './mesh.ts'#" \
//     ../../../../../packages/anikit/src/shatter.ts > shatter.ts
//   node ref_shatter.ts > shatter_ref.json && rm shatter.ts
import { shatter } from './shatter.ts';
// Every 9th shard (rows of 15 floats) keeps the reference small.
const pick = (d: Float32Array) => Array.from(d).filter((_, k) => Math.floor(k / 15) % 9 === 0);
const src = { id: 0 } as any;
const out: any = {};
const a = shatter(src, { transform: { rx: 20, ry: -35, scale: 1.2, x: 30, y: -10, z: 5, color: [0.2, 0.4, 0.9] }, force: 900, spin: 540 });
out.a = { count: a.count, colors: Array.from(a.instances.data.slice(12, 15)), t: {} as any };
for (const t of [0, 0.35, 1.7]) out.a.t[t] = pick(a.update(t).instances.data);
const b = shatter(src, { center: [0, 200, 0], spread: 0.9, gravity: 600, drag: 0, ripple: 2, spin: 90 });
out.b = { t: {} as any };
for (const t of [0.1, 0.9]) out.b.t[t] = pick(b.update(t).instances.data);
console.log(JSON.stringify(out));
