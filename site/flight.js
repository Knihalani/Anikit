// "Flight": a scroll-driven journey through a neon city, rendered every frame by
// anikit's WebAssembly rasterizer (no WebGL) — per-pixel chrome, hex-lit towers,
// a mirror floor, light trails, streaking stars, a sky with a sun. Scroll = fly.
import { renderer3d, scene3d, mesh, instances, flightPath } from '../packages/anikit/dist/anikit.js';

export function flight({ canvas, stats, onProgress, whileVisible, reduced }) {
  let scale = 0.5;
  const size = () => {
    const r = canvas.getBoundingClientRect();
    return [Math.max(160, Math.round(r.width * scale)), Math.max(90, Math.round(r.height * scale))];
  };
  let [W, H] = size();
  canvas.width = W; canvas.height = H;
  const r3 = renderer3d(canvas);
  const cam = scene3d({ fov: 64, width: W, height: H, near: 4, far: 60000 });
  let seed = 9;
  const rnd = () => ((seed = (seed * 1664525 + 1013904223) >>> 0) / 4294967296);
  const towers = instances(420, (i) => {
    const side = i % 2 ? 1 : -1, z = -Math.floor(i / 2) * 150, h = 240 + rnd() * 1700;
    return { x: side * (300 + rnd() * 1000), y: -300 + h / 2, z, sx: 110, sy: h, sz: 110, color: [0.03, 0.035, 0.05] };
  });
  const gates = instances(8, (i) => ({ x: 0, y: 160, z: -16000 - i * 520, color: [0.9, 0.92, 1] }));
  const path = flightPath([[0, 500, 2600], [0, 90, -2000], [260, 120, -8000], [-260, 70, -13000], [0, 160, -17000], [0, 160, -21000], [0, 260, -28000]], { lookAhead: 900, bank: 1.4 });
  const trails = [-1, 1].map((k) => mesh.tube(path.points(300, 0, path.length * 0.9).map(([x, y, z], i) => [x + k * 230 + Math.sin(i * 0.06) * 60, y - 130 + Math.cos(i * 0.08 + k) * 40, z]), { radius: 10, sides: 6, taper: [0.2, 1] }));
  const stars = new Float32Array(12000 * 3);
  for (let i = 0; i < 12000; i++) stars.set([(rnd() - 0.5) * 7000, -200 + rnd() * 3500, 3000 - rnd() * 34000], i * 3);
  const cube = mesh.cube(), floor = mesh.plane(1), ring = mesh.torus(300, 22, 56, 12);
  const floorInst = instances(1, () => ({ y: -300, z: -14000, sx: 14000, sz: 40000, color: [0.02, 0.02, 0.03] }));
  const trailAt = [instances(1, () => ({ color: [0.3, 0.6, 1] })), instances(1, () => ({ color: [1, 0.4, 0.2] }))];
  let target = 0, s = 0, avg = 20, t = 0;
  onProgress((p) => (target = p));
  whileVisible((dt) => {
    t += dt / 1000;
    // Ease towards the scroll position, with a slow drift so it never freezes.
    s += (target - s) * Math.min(1, (dt / 1000) * 4);
    const L = path.length * 0.92;
    const at = Math.min(L, (s + (reduced ? 0 : (Math.sin(t * 0.3) * 0.004 + 0.004))) * L);
    path.apply(cam, at);
    const t0 = performance.now();
    r3.render(cam, [
      { mirror: true, mesh: floor, instances: floorInst, material: { lines: 1.4, lineColor: [0.3, 0.55, 1.6], grid: [70, 200], lineWidth: 0.03, specular: 0.2 } },
      { mesh: cube, instances: towers, material: { metal: 0.6, specular: 1, shininess: 80, lines: 1.6, pattern: 'hex', lineColor: [1.6, 0.3, 1.0], grid: [3, 12], lineWidth: 0.05, pulse: 2, pulsePhase: t * 1.5, pulseDir: [0, 0, 0.0004] } },
      { mesh: ring, instances: gates, material: { metal: 1, iridescence: 0.7, specular: 1.5, shininess: 150, pulse: 3, lineColor: [0.4, 0.9, 1.8], pulseCount: 2, pulsePhase: t * 3 } },
      ...trails.map((m, k) => ({ mesh: m, blend: 'additive', material: { ambient: 0, diffuse: 0, specular: 0, rim: 0, emissive: 0.15, pulse: 3, pulseCount: 8, pulsePhase: t * 2 + k * 0.5, lineColor: k ? [1.6, 0.5, 0.2] : [0.3, 0.8, 1.6] }, instances: trailAt[k] })),
      { points: stars, size: 3, intensity: 2.2, streak: 1.2, fog: [8000, 20000] },
    ], { background: '#03040a', fog: [9000, 26000], fogColor: [0.18, 0.06, 0.2], sky: [0.02, 0.02, 0.08], horizon: [0.95, 0.35, 0.7], ground: [0.01, 0.01, 0.02], strips: [1.8, 1.8, 2.2], key: [0, 0.14, -1], keyColor: [1, 0.7, 0.9], skybox: { sun: 2.2 }, mirror: { y: -300, strength: 0.45 } });
    const ms = performance.now() - t0;
    avg = avg * 0.92 + ms * 0.08;
    // Hold ~22 ms of render time per frame by adapting the resolution.
    const want = Math.min(0.75, Math.max(0.28, scale * Math.sqrt(22 / Math.max(4, avg))));
    if (Math.abs(want - scale) > 0.05) {
      scale = want; [W, H] = size(); canvas.width = W; canvas.height = H; r3.resize(W, H); cam.width = W; cam.height = H;
    }
    stats.textContent = `${W}×${H} · ${avg.toFixed(1)} ms · ${r3.triangles.toLocaleString('en-US')} triangles`;
  });
}
