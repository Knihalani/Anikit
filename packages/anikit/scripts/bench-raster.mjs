// Software 3D renderer throughput (WebAssembly, in Node): 1920×1080 scenes
// with per-pixel lit chrome, grid lines and instanced towers.
//   node scripts/bench-raster.mjs [--hash]
// --hash prints an FNV hash of each scene's last frame (colour + depth), to
// check that an optimisation leaves the output bit-identical.
import { performance } from 'node:perf_hooks';
// DIST=path/to/dist benchmarks another build (a saved baseline, for interleaved A/B runs).
const A = await import(process.env.DIST ? new URL(`file://${process.env.DIST.replace(/\/$/, '')}/anikit.js`).href : '../dist/anikit.js');
await A.ready;
const W = 1920, H = 1080;
const HASH = process.argv.includes('--hash');
const fnv = (u8) => { let h = 0x811c9dc5; for (let i = 0; i < u8.length; i++) h = Math.imul(h ^ u8[i], 16777619); return (h >>> 0).toString(16).padStart(8, '0'); };
const hashOf = (px) => { const d = r3.depth(); return HASH ? `  hash ${fnv(px)}/${fnv(new Uint8Array(d.buffer, d.byteOffset, d.byteLength))}` : ''; };
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
// Textures (albedo on towers, emissive on a sphere), key-light shadows and an additive tube.
{
  const tex = (w, h, f) => { const d = new Uint8ClampedArray(w * h * 4); for (let i = 0; i < w * h; i++) { const c = f(i % w, Math.floor(i / w)); d.set([c[0], c[1], c[2], 255], i * 4); } return A.texture({ width: w, height: h, data: d }); };
  const bricks = tex(256, 256, (x, y) => ((x >> 4) + (y >> 3)) % 2 ? [200, 120, 90] : [90, 60, 50]);
  const glowTex = tex(300, 150, (x, y) => [(x * 7) & 255, (y * 5) & 255, ((x ^ y) * 3) & 255]);
  const path = Array.from({ length: 120 }, (_, i) => [Math.cos(i / 12) * 700, -300 + i * 4, Math.sin(i / 12) * 700 - 400]);
  scenes['textures, shadows, additive'] = [
    { mesh: cube, instances: towers, material: { texture: bricks, textureRepeat: [2, 6] } },
    { mesh: A.mesh.sphere(24, 32), instances: A.instances(1, () => ({ y: 200, z: -300, scale: 260 })), material: { texture: glowTex, textureMode: 'emissive', textureGain: 1.3, ambient: 0.05 } },
    { mesh: A.mesh.tube(path, { radius: 14, taper: [0, 1] }), blend: 'additive', material: { emissive: 1, pulse: 2, pulseCount: 4 } },
  ];
}
const sceneOpts = { 'textures, shadows, additive': { shadows: { extent: 2200, size: 1024 } } };
for (const [name, items] of Object.entries(scenes)) {
  const o = { ...env, ...sceneOpts[name] };
  for (let i = 0; i < 15; i++) r3.render(cam, items, o);
  const n = 30, t = performance.now();
  let px;
  for (let i = 0; i < n; i++) px = r3.render(cam, items, o);
  console.log(`${name.padEnd(32)} ${((performance.now() - t) / n).toFixed(1).padStart(6)} ms/frame  (${r3.triangles} triangles)${hashOf(px)}`);
}

// A film-like frame with every pass on (mirror floor, sky + sun, point lights,
// stars, depth of field, camera motion blur), with the per-pass split from r3.stats.
{
  const floor = A.instances(1, () => ({ y: -500, z: -2000, sx: 14000, sz: 30000, color: [0.02, 0.02, 0.03] }));
  const stars = new Float32Array(20000 * 3).map((_, i) => (i % 3 === 0 ? ((i * 7919) % 6000) - 3000 : i % 3 === 1 ? ((i * 104729) % 3000) - 400 : -((i * 1299709) % 20000)));
  const items = [
    { mesh: A.mesh.plane(1), instances: floor, mirror: true, material: { lines: 1.2, lineColor: [0.3, 0.5, 1.5], grid: [60, 150], lineWidth: 0.03 } },
    ...scenes['900 towers (edge lines)'], ...scenes['knot (chrome, lines, pulses)'], ...scenes['textures, shadows, additive'].slice(1),
    { points: stars, size: 3, intensity: 2, streak: 0.4 },
  ];
  const prev = A.scene3d({ fov: 50, width: W, height: H, near: 8, far: 40000, camera: { x: 310, y: 250, z: 1450 }, target: { x: 0, y: -50, z: -200 } }).viewProjection().matrix;
  const opts = { ...env, skybox: { sun: 2 }, mirror: { y: -500, strength: 0.45 }, lights: [{ position: [200, -300, 600], color: [0.3, 0.8, 1.6], radius: 600 }, { position: [-300, -300, 200], color: [1.6, 0.6, 0.2], radius: 600 }], dof: { focus: 1400, aperture: 10 }, motionBlur: { shutter: 0.55 }, prevCamera: prev };
  for (let i = 0; i < 10; i++) r3.render(cam, items, opts);
  const n = 20, sum = {};
  const t = performance.now();
  let px;
  for (let i = 0; i < n; i++) { px = r3.render(cam, items, opts); for (const [k, v] of Object.entries(r3.stats)) sum[k] = (sum[k] ?? 0) + v; }
  console.log(`${'film-like frame (all passes)'.padEnd(32)} ${((performance.now() - t) / n).toFixed(1).padStart(6)} ms/frame${hashOf(px)}`);
  console.log('  per pass: ' + Object.entries(sum).filter(([, v]) => v / n >= 0.05).map(([k, v]) => `${k} ${(v / n).toFixed(1)}`).join(' · ') + ' ms');
}
