// A starter 3D "journey": one camera flying fast through a neon city,
// through chrome gates, past a knot that shatters, into a 3D wordmark.
//   npx anikit-render journey.mjs --review review/           (contact sheet + pacing report)
//   npx anikit-render journey.mjs --out journey.mp4 --workers 4
//
// Recipe for 3D that doesn't look plain: near-black bodies, colour in light
// (chrome + studio strips, glowing grid lines, travelling pulses, emissive),
// bloom + streaks in the grade, and a camera that never stops moving.
export default async function journey({ anikit: A }) {
  const { composition, scene3d, renderer3d, mesh, instances, flightPath, shatter } = A;
  const W = 1920, H = 1080, D = 8;
  let seed = 11;
  const rnd = () => ((seed = (seed * 1664525 + 1013904223) >>> 0) / 4294967296);

  // --- World (one long corridor along −z) -------------------------------------------
  const cube = mesh.cube(), floor = mesh.plane(1), ring = mesh.torus(300, 22, 96, 18);
  const towers = instances(440, (i) => { // rows end before the plaza where the wordmark stands
    const side = i % 2 ? 1 : -1, z = -Math.floor(i / 2) * 150, h = 200 + rnd() * 1500;
    return { x: side * (300 + rnd() * 900), y: -300 + h / 2, z, sx: 100, sy: h, sz: 100, color: [0.03, 0.035, 0.05] };
  });
  const gates = instances(10, (i) => ({ x: 0, y: 150, z: -20000 - i * 450, color: [0.9, 0.92, 1] }));
  const knotMesh = mesh.torusKnot({ p: 2, q: 3, radius: 420, spread: 160, tube: 80, segments: 400, sides: 20 });
  const KNOT = { x: 0, y: 200, z: -27200, color: [0.05, 0.05, 0.07] };
  const shards = shatter(knotMesh, { transform: KNOT, force: 1400, spin: 600, drag: 0.8 });
  const word = mesh.text('anikit', { font: '900 160px sans-serif', depth: 0.06 });
  // Light trails racing along the route, offset from the camera path.
  const trailPts = (k) => path.points(500, 0, path.length * 0.62).map(([x, y, z], i) => [x + k * 220 + Math.sin(i * 0.05) * 60, y - 140 + Math.cos(i * 0.07 + k) * 40, z]);
  const stars = new Float32Array(20000 * 3);
  for (let i = 0; i < 20000; i++) stars.set([(rnd() - 0.5) * 7000, -200 + rnd() * 3500, 3000 - rnd() * 40000], i * 3);

  // --- Camera: a flight path and a speed curve (seconds → fraction of the path) ------
  const path = flightPath([
    [0, 400, 3000], [0, 80, -3000], [250, 120, -9000], [-250, 60, -15000], [0, 150, -20000],
    [0, 150, -24500], [700, 350, -27000], [0, 250, -29500], [0, 200, -34500],
  ], { lookAhead: 900, bank: 1.3 });
  const s = path.timing([[0, 0], [2.4, 0.42], [3.6, 0.6], [4.6, 0.7], [5.8, 0.85], [7, 1]]);
  const HIT = 4.9; // the knot shatters as we skim past it
  const trails = [-1, 1].map((k) => mesh.tube(trailPts(k), { radius: 10, sides: 8, taper: [0.2, 1] }));
  // Instances are created once; draw() only updates the ones that move.
  const floorAt = instances(1, () => ({ y: -300, z: -18000, sx: 14000, sz: 44000, color: [0.02, 0.02, 0.03] }));
  const knotAt = instances(1, () => KNOT);
  const wordAt = instances(1, () => ({ x: 0, y: 250, z: -36000, rx: 90, sx: 3400, sy: 3400, sz: 3400 * word.aspect, color: [0.95, 0.95, 1] }));
  const trailAt = [instances(1, () => ({ color: [0.3, 0.6, 1] })), instances(1, () => ({ color: [1, 0.4, 0.2] }))];

  const cam = scene3d({ fov: 64, width: W, height: H, near: 4, far: 40000 });
  const r3 = renderer3d({ width: W, height: H });
  const film = composition({
    width: W, height: H, fps: 60, duration: D, motionBlur: 1, // camera motion blur comes from the renderer (depth-based)
    post: {
      aa: 1, rays: 0.8, raysCenter: [0.5, 0.42],
      bloom: 0.9, bloomThreshold: 0.5, bloomRadius: 44, streaks: 0.5, streakLength: 300,
      aberration: 0.7, vignette: 0.45, grain: 4, contrast: 1.06, saturation: 1.1,

    },
  });
  film.impact(0, { flash: '#ffffff', flashOpacity: 0.8, decay: 6 });
  film.impact(HIT, { shake: 30, flash: '#bfe3ff', flashOpacity: 0.6, aberration: 8 });
  film.impact(7.2, { shake: 18, aberration: 4 });

  // Choreography runs on a clock advanced by the *scaled* dt, so the
  // scene's timeScale (bullet time) slows the camera and the shards too.
  let clock = 0, prevClock = 0;

  film.scene('journey', 0, D, {
    // Bullet time right after the hit: everything slows together.
    timeScale: (t) => 1 - 0.75 * Math.exp(-(((t - HIT - 0.25) / 0.4) ** 2)),
    step({ dt }) { prevClock = clock; clock += dt; },
    draw({ ctx }) {
      const t = clock;
      const pose = (t) => {
        const roll = t > 5.5 && t < 6.5 ? (1 - Math.cos(((t - 5.5) / 1) * Math.PI)) * 180 : 0; // a barrel roll out of the blast
        path.apply(cam, s(t), roll);
        if (t > 6.9) { // land in front of the wordmark
          const k = Math.min(1, (t - 6.9) / 0.6), e = 1 - (1 - k) ** 3;
          Object.assign(cam.camera, { x: 0, y: 260 - 60 * e, z: -33400 + 300 * e });
          Object.assign(cam.target, { x: 0, y: 250, z: -36000 });
          cam.roll *= 1 - e;
        }
      };
      // The camera one frame ago drives motion blur and star streaks, even in previews.
      pose(prevClock);
      const prevCamera = cam.viewProjection().matrix;
      pose(t);
      shards.update(t - HIT);
      const items = [
        { mirror: true, mesh: floor, instances: floorAt, material: { lines: 1.4, lineColor: [0.3, 0.55, 1.6], grid: [70, 220], lineWidth: 0.03 } },
        { mesh: cube, instances: towers, material: { metal: 0.6, specular: 1, shininess: 80, lines: 1.6, pattern: 'hex', lineColor: [1.6, 0.3, 1.0], grid: [3, 12], lineWidth: 0.05, pulse: 2, pulsePhase: t * 2, pulseDir: [0, 0, 0.0004] } },
        { mesh: ring, instances: gates, material: { metal: 1, iridescence: 0.7, specular: 1.5, shininess: 150, pulse: 3, lineColor: [0.4, 0.9, 1.8], pulseCount: 2, pulsePhase: t * 3 } },
        t < HIT
          ? { mesh: knotMesh, instances: knotAt.set(0, { ...KNOT, ry: t * 40 }), material: { metal: 0.9, iridescence: 0.3, specular: 1.4, shininess: 120, lines: 1.2, lineColor: [0.3, 0.8, 1.6], grid: [0, 6], pulse: 2.5, pulseCount: 3, pulsePhase: t * 2 } }
          : { mesh: shards.mesh, instances: shards.instances, cull: false, material: { metal: 0.9, iridescence: 0.4, specular: 1.4, shininess: 120, emissive: 0.4 } },
        { mesh: word, instances: wordAt, material: { metal: 0.95, iridescence: 0.35, specular: 1.6, shininess: 160 } },
        ...trails.map((m, k) => ({ mesh: m, blend: 'additive', material: { ambient: 0, diffuse: 0, specular: 0, rim: 0, emissive: 0.15, pulse: 3, pulseCount: 8, pulsePhase: t * 2 + k * 0.5, lineColor: k ? [1.6, 0.5, 0.2] : [0.3, 0.8, 1.6] }, instances: trailAt[k] })),
        { points: stars, size: 3, intensity: 2.5, streak: 0.4, fog: [9000, 22000] },
      ];
      const px = r3.render(cam, items, { background: '#03040a', fog: [7000, 24000], fogColor: [0.02, 0.025, 0.06], sky: [0.05, 0.06, 0.16], horizon: [0.95, 0.35, 0.7], ground: [0.01, 0.01, 0.02], strips: [1.8, 1.8, 2.2],
        key: [0, 0.14, -1], keyColor: [1, 0.7, 0.9], skybox: { sun: 2.2 }, mirror: { y: -300, strength: 0.45 }, motionBlur: { shutter: 0.55 }, prevCamera });
      ctx.putImageData(new globalThis.ImageData(px, W, H), 0, 0);
    },
  });
  return film;
}
