// ANIKIT — VELOCITY. A 22-second 3D journey in one continuous flight, composed
// in code and rendered entirely by anikit (WebAssembly rasterizer, no WebGL):
//
//   anikit-render film.mjs --review review/          (contact sheet + pacing report)
//   anikit-render film.mjs --out anikit-film.mp4 --workers 3 --fade-out 0.6
//
// Ignition → a tunnel of circuit-lit rings → a neon city on a mirror floor
// (sunset sky, light trails, a barrel roll between two towers) → chrome gates
// → a black-chrome knot that shatters in bullet time → a vortex of light cubes
// → the wordmark, in chrome, on the mirror floor under the sun.
export default async function film({ anikit: A, node }) {
  const { composition, scene3d, renderer3d, mesh, instances, flightPath, shatter, emitter } = A;
  const W = 1920, H = 1080, FPS = 60, D = 21.8; // D: story seconds (the film itself is a little shorter, see `story`)
  node.registerFont('/usr/share/fonts/opentype/inter/InterDisplay-Black.otf', 'Display');

  let seed = 20261116;
  const rnd = () => ((seed = (seed * 1664525 + 1013904223) >>> 0) / 4294967296);
  const TAU = Math.PI * 2;
  const clamp = (v, a = 0, b = 1) => Math.min(b, Math.max(a, v));
  const mix = (a, b, t) => a + (b - a) * t;
  const smooth = (a, b, x) => { const t = clamp((x - a) / (b - a)); return t * t * (3 - 2 * t); };
  const bell = (x, c, w) => Math.exp(-(((x - c) / w) ** 2));

  // Palette: near-black bodies; the colour lives in light.
  const BODY = [0.03, 0.035, 0.05];
  const MAG = [1.6, 0.26, 0.92], CYAN = [0.28, 0.85, 1.7], ORANGE = [1.7, 0.62, 0.22], WHITE = [1.5, 1.5, 1.6];

  // ---- Time: the choreography runs on a clock slowed by bullet time ------------------
  const HIT = 12.55;  // clock time the knot shatters
  const LAND = 18.9;  // clock time we arrive at the wordmark
  // Pacing: the choreography below is written in "story" seconds; the film runs
  // through the long stretches faster (the tunnel, the beat after the hit, the
  // vortex), with speed changes eased so the camera never jerks.
  const win = (a, b, x, e = 0.35) => smooth(a - e, a + e, x) * (1 - smooth(b - e, b + e, x));
  const pace = (st) => 1 + 0.22 * win(0.7, 2.6, st) + 0.5 * win(13.3, 14.2, st) + 0.38 * win(14.3, 16.8, st);
  const storyTable = [0];
  for (let i = 1; storyTable[i - 1] < D + 1; i++) storyTable.push(storyTable[i - 1] + pace(storyTable[i - 1]) / 1000);
  const story = (c) => { const i = Math.max(0, c * 1000), k = Math.min(Math.floor(i), storyTable.length - 2); return storyTable[k] + (storyTable[k + 1] - storyTable[k]) * (i - k); };
  const unstory = (st) => { let lo = 0, hi = storyTable.length - 1; while (hi - lo > 1) { const m = (lo + hi) >> 1; if (storyTable[m] < st) lo = m; else hi = m; } return hi / 1000; };
  const timeScale = (t) => 1 - 0.82 * bell(t, unstory(HIT) + 0.42, 0.42);
  // Film length: until the story clock reaches the end of the old cut (~21.2 s), plus the time bullet time costs.
  const DUR = Math.round((unstory(21.2) + 0.61) * FPS) / FPS;
  const clockOf = (t) => story(film.clockAt('velocity', t)); // story seconds at composition time t
  const beat = (st) => film.timeAt('velocity', unstory(st)); // composition time of a story beat

  // ---- The route ------------------------------------------------------------------------
  const KNOT_C = [0, 320, -38000], ORBIT_R = 2000;
  const orbit = [];
  for (let k = 0; k <= 6; k++) {
    const a = Math.PI / 2 + (k / 6) * Math.PI * 1.5; // 270° around the knot, starting in front of it
    orbit.push([KNOT_C[0] + Math.cos(a) * ORBIT_R, KNOT_C[1] + 140 + Math.sin(k * 1.3) * 160, KNOT_C[2] + Math.sin(a) * ORBIT_R]);
  }
  const waypoints = [
    [0, 0, 900], [0, 0, -4500], [0, 0, -9800],                          // tunnel
    [0, 110, -11600], [260, 60, -14200], [-300, 90, -17200],            // city
    [0, 70, -20500], [320, 130, -23500], [-160, 170, -26400],           // the gap at −20500
    [0, 190, -28600], [120, 220, -30800], [-60, 240, -33000],           // gates
    ...orbit,                                                           // around the knot
    [900, 300, -41000], [0, 250, -43400], [0, 250, -48000], [0, 260, -52400], // vortex
    [0, 360, -55200], [0, 250, -57400], [0, 245, -59200],                 // the plaza (the last point only aims the camera)
  ];
  const path = flightPath(waypoints, { lookAhead: 950, bank: 1.25, maxBank: 40 });
  const samples = path.points(4000);
  const sNear = (p) => {
    let best = 0, bd = Infinity;
    samples.forEach((q, i) => { const d = (q[0] - p[0]) ** 2 + (q[1] - p[1]) ** 2 + (q[2] - p[2]) ** 2; if (d < bd) { bd = d; best = i; } });
    return best / (samples.length - 1);
  };
  const at = (i) => sNear(waypoints[i]);
  const iOrbit0 = 12, iOrbit1 = 12 + orbit.length - 1, iVortex0 = iOrbit1 + 2, iVortex1 = iOrbit1 + 4;
  const timing = path.timing([
    [0, 0], [0.45, 0.004], [2.95, at(2)], [5.0, at(5)], [7.2, at(8)], [9.4, at(11)],
    [11.4, at(iOrbit0 + 3)], [13.2, at(iOrbit1)], [14.2, at(iVortex0)], [16.9, at(iVortex1)], [LAND, at(iVortex1 + 2)], [D + 1, at(iVortex1 + 2)],
  ]);

  // ---- Zone A: a tunnel of circuit-lit rings ------------------------------------------
  const ringMesh = mesh.torus(460, 30, 96, 16);
  const rings = instances(38, (i) => ({ z: 300 - i * 270, rz: i * 17, color: BODY }));

  // ---- Zone B: the city ----------------------------------------------------------------
  const cube = mesh.cube();
  const towersInit = [];
  for (let z = -10600; z > -27400; z -= 170) {
    const p = path.point(sNear([0, 0, z]) * path.length);
    for (const side of [-1, 1]) {
      const gap = Math.abs(z + 20500) < 260; // two towers pinch in: the barrel-roll gap
      const off = gap ? 210 : 420 + rnd() * 1100;
      const h = gap ? 1500 : 260 + rnd() * 1900;
      const w = gap ? 150 : 100 + rnd() * 90;
      towersInit.push({ x: p[0] + side * (off + w / 2), y: -300 + h / 2, z: z + (rnd() - 0.5) * 60, sx: w, sy: h, sz: w, color: BODY });
    }
  }
  // One tower in four is a cyan circuit tower: two hues, more depth.
  const isAccent = (i) => ((i >> 1) * 3 + (i & 1) * 2) % 4 === 1; // both sides of the street
  const accentInit = towersInit.filter((_, i) => isAccent(i)), mainInit = towersInit.filter((_, i) => !isAccent(i));
  const towers = instances(mainInit.length, (i) => mainInit[i]);
  const accents = instances(accentInit.length, (i) => accentInit[i]);
  const trailPts = (k, from, to) => samples.filter((q, i) => { const f = i / (samples.length - 1); return f > from && f < to; })
    .map(([x, y, z], i) => [x + k * 240 + Math.sin(i * 0.11 + k) * 70, y - 150 + Math.cos(i * 0.09 + k * 2) * 50, z]);
  const trails = [-1, 1].map((k) => mesh.tube(trailPts(k, at(3) - 0.01, at(11)), { radius: 11, sides: 8, taper: [0.15, 1] }));
  const trailAt = [instances(1, () => ({ color: [0.3, 0.6, 1] })), instances(1, () => ({ color: [1, 0.45, 0.2] }))];

  // ---- Zone C: chrome gates ----------------------------------------------------------
  const gateZ = [-28300, -29300, -30300, -31300, -32300, -33300];
  const gateMesh = mesh.torus(380, 34, 120, 18);
  const gates = instances(gateZ.length, (i) => { const p = path.point(sNear([0, 0, gateZ[i]]) * path.length); return { x: p[0], y: p[1], z: p[2], color: [0.9, 0.92, 1] }; });

  // ---- Zone D: the knot, and its shards --------------------------------------------
  const knot = mesh.torusKnot({ p: 2, q: 3, radius: 880, spread: 340, tube: 170, segments: 640, sides: 28 });
  const KNOT_AT = { x: KNOT_C[0], y: KNOT_C[1], z: KNOT_C[2], rx: 25, color: [0.05, 0.05, 0.065] };
  const knotAt = instances(1, () => KNOT_AT);
  const KNOT_SPIN = 24; // degrees per second
  // Shards start in the pose the knot breaks in.
  const shards = shatter(knot, { transform: { ...KNOT_AT, ry: HIT * KNOT_SPIN }, force: 3200, spread: 0.75, spin: 640, drag: 0.6, ripple: 0.08 });
  // Sparks thrown out by the hit: light points, depth-tested against the shards.
  const sparks = emitter({ capacity: 7000, rate: 0, life: 1.4, lifeVariation: 0.5, origin: KNOT_C, shape: 'sphere', size: 700, speed: 2400, speedVariation: 0.7, spread: Math.PI, drag: 1.4, gravity: [0, -500, 0], colors: ['#ffffff', '#ffd6f0', '#ff4fb0', '#401030'], autoStep: false, seed: 7 });

  // ---- Zone E: the vortex -------------------------------------------------------------
  const NV = 2600;
  const vortexInit = Array.from({ length: NV }, (_, i) => {
    const f = i / NV, a = f * TAU * 26 + rnd() * 0.5, r = 650 + rnd() * 700, z = -41900 - f * 10900 + (rnd() - 0.5) * 200;
    const k = rnd(); // mostly dark glass, with cyan and magenta lights among them
    return { a, r, z, s: 50 + rnd() * 90, spin: rnd() * 360, color: k < 0.55 ? [0.02, 0.022, 0.035] : k < 0.8 ? CYAN.map((x) => x * 0.45) : MAG.map((x) => x * 0.45) };
  });
  const vortex = instances(NV);
  const placeVortex = (t) => {
    for (let i = 0; i < NV; i++) {
      const v = vortexInit[i], a = v.a + t * 0.9;
      vortex.set(i, { x: Math.cos(a) * v.r, y: 250 + Math.sin(a) * v.r, z: v.z, rx: v.spin + t * 90, ry: v.spin * 1.7 + t * 60, scale: v.s,
        color: v.color });
    }
  };

  // ---- Zone F: the wordmark --------------------------------------------------------
  const word = mesh.text('anikit', { font: '900 200px Display', depth: 0.055, resolution: 900, bevel: 1.4 });
  const WORD_Z = -60300;
  const wordAt = instances(1, () => ({ x: 0, y: 330, z: WORD_Z, rx: 90, sx: 3600, sy: 3600, sz: 3600 * word.aspect, color: [0.92, 0.9, 1] }));

  // ---- Shared: floor, stars --------------------------------------------------------
  const floor = mesh.plane(1);
  const floorAt = instances(1, () => ({ y: -300, z: -36000, sx: 34000, sz: 56000, color: [0.025, 0.025, 0.035] }));
  const NS = 26000, stars = new Float32Array(NS * 3), starCol = new Float32Array(NS * 3);
  for (let i = 0; i < NS; i++) {
    stars.set([(rnd() - 0.5) * 9000, -200 + rnd() * 4200, 3000 - rnd() * 66000], i * 3);
    const w = rnd();
    starCol.set(w < 0.12 ? [1.5, 0.5, 1.2] : w < 0.24 ? [0.5, 0.9, 1.6] : [1.1, 1.1, 1.2], i * 3);
  }

  const cam = scene3d({ fov: 66, width: W, height: H, near: 5, far: 60000 });
  const r3 = renderer3d({ width: W, height: H });

  // ---- The grade: functions of composition time (mapped onto the clock) -------------
  const film = composition({
    width: W, height: H, fps: FPS, duration: DUR, motionBlur: 1,
    post: {
      aa: 1,
      bloom: (t) => 0.85 + 0.6 * bell(t, 0.05, 0.3),
      bloomThreshold: 0.5, bloomRadius: 46,
      streaks: 0.45, streakLength: 300, streakTint: [0.7, 0.75, 1],
      rays: (t) => { const c = clockOf(t); return 0.35 + 0.55 * smooth(2.9, 3.4, c) * (1 - smooth(9.2, 9.8, c)) + 0.75 * smooth(17.4, 18.4, c); },
      raysCenter: [0.5, 0.4], raysLength: 0.7,
      zoomBlur: (t) => { const c = clockOf(t); return 0.1 * smooth(0.5, 1.4, c) * (1 - smooth(2.6, 3.1, c)) + 0.08 * smooth(14.3, 15, c) * (1 - smooth(16.8, 17.4, c)); },
      aberration: (t) => 0.5 + 2.5 * bell(clockOf(t), 3.0, 0.15),
      // Lens flares while the sun is in shot (city, gates, the plaza).
      flares: (t) => { const c = clockOf(t); return 0.15 + 0.6 * smooth(3.0, 3.6, c) * (1 - smooth(9.2, 9.8, c)) + 0.75 * smooth(17.6, 18.6, c); },
      vignette: 0.42, grain: 3.5,
      contrast: 1.07, saturation: 1.12,
      shadows: [0.0, 0.012, 0.035], highlights: [0.05, 0.015, -0.02],
    },
  });
  const pose = (c) => {
    path.apply(cam, timing(c),
      // Extra roll: a slow half-turn in the tunnel, a barrel roll through the gap, a spiral in the vortex.
      360 * smooth(0.6, 2.9, c) + 360 * smooth(5.45, 6.25, c) + 360 * smooth(14.4, 16.9, c),
      // A handheld wobble at speed, a kick after the hit, still for the landing.
      { shake: (5 * smooth(2.8, 3.4, c) + 16 * bell(c, HIT + 0.5, 0.5)) * (1 - smooth(LAND - 1.2, LAND - 0.2, c)), time: c, rate: 2.5 });
    // Around the knot, keep it in frame.
    const w = smooth(9.3, 10.2, c) * (1 - smooth(12.7, 13.6, c));
    if (w > 0) {
      cam.target.x = mix(cam.target.x, KNOT_C[0], w);
      cam.target.y = mix(cam.target.y, KNOT_C[1], w);
      cam.target.z = mix(cam.target.z, KNOT_C[2], w);
    }
    // The landing: settle on the wordmark, then a slow push in.
    if (c > LAND - 0.4) {
      const k = smooth(LAND - 0.4, LAND + 0.5, c), push = smooth(LAND, D + 1, c);
      cam.target.x = mix(cam.target.x, 0, k);
      cam.target.y = mix(cam.target.y, 300, k);
      cam.target.z = mix(cam.target.z, WORD_Z, k);
      cam.roll = ((((cam.roll + 180) % 360) + 360) % 360 - 180) * (1 - k); // unwind the shortest way
      cam.camera.z -= 260 * push;
      cam.camera.y -= 40 * push;
    }
  };

  let prevClock = 0, lastClock = 0;
  film.scene('velocity', 0, DUR, {
    timeScale, // bullet time as the knot breaks: camera, shards, pulses all slow together
    step({ clock, dt }) {
      if (story(lastClock) < HIT && story(clock) >= HIT) sparks.burst(6500);
      prevClock = lastClock; lastClock = clock;
      sparks.step(dt * pace(story(clock)));
    },
    draw({ ctx, clock }) {
      const c = story(clock);
      pose(story(prevClock));
      const prevCamera = cam.viewProjection().matrix;
      pose(c);
      const z = cam.camera.z;
      const items = [];
      const floorOn = z < -9600;
      if (floorOn) items.push({ mirror: true, mesh: floor, instances: floorAt, material: { ambient: 0.2, lines: 1.0, lineColor: [0.28, 0.5, 1.5], grid: [90, 150], lineWidth: 0.022, specular: 0.25 } });
      if (z > -12500) items.push({ mesh: ringMesh, instances: rings, material: { metal: 0.85, iridescence: 0.25, specular: 1.3, shininess: 110, lines: 2.2, pattern: 'circuit', lineColor: CYAN, grid: [40, 3], lineWidth: 0.07, pulse: 3.2, pulseCount: 1, pulsePhase: c * 2.6, pulseDir: [0, 0, 0.0009] } });
      if (z < -9000 && z > -30500) {
        items.push({ mesh: cube, instances: towers, material: { metal: 0.28, rim: 0.15, specular: 1.2, shininess: 90, lines: 1.25, pattern: 'hex', lineColor: MAG, grid: [3, 14], lineWidth: 0.032, pulse: 2.2, pulsePhase: c * 1.8, pulseDir: [0, 0, 0.00035] } });
        items.push({ mesh: cube, instances: accents, material: { metal: 0.25, iridescence: 0.2, rim: 0.15, specular: 1.3, shininess: 120, lines: 2, pattern: 'circuit', lineColor: CYAN, grid: [4, 16], lineWidth: 0.04, pulse: 2.6, pulseCount: 2, pulsePhase: c * 2.2, pulseDir: [0, 0, 0.0005] } });
        trails.forEach((m, k) => items.push({ mesh: m, instances: trailAt[k], blend: 'additive', material: { ambient: 0, diffuse: 0, specular: 0, rim: 0, emissive: 0.12, pulse: 3.5, pulseCount: 10, pulsePhase: c * 1.6 + k * 0.5, lineColor: k ? ORANGE : CYAN } }));
      }
      if (z < -26000 && z > -36500) items.push({ mesh: gateMesh, instances: gates, material: { metal: 1, iridescence: 0.75, hue: 0.1, specular: 1.6, shininess: 160, pulse: 3, pulseCount: 2, pulsePhase: c * 3.2, lineColor: CYAN } });
      if (z < -31000 && z > -46000) {
        if (c < HIT) items.push({ mesh: knot, instances: knotAt.set(0, { ...KNOT_AT, ry: c * KNOT_SPIN }), material: { metal: 0.92, iridescence: 0.3, specular: 1.5, shininess: 140, rim: 0.3, lines: 1.6, pattern: 'circuit', lineColor: CYAN, grid: [70, 5], lineWidth: 0.06, pulse: 2.8, pulseCount: 3, pulsePhase: c * 1.4 } });
        else {
          shards.update(c - HIT);
          items.push({ mesh: shards.mesh, instances: shards.instances, cull: false, material: { metal: 0.9, iridescence: 0.45, specular: 1.6, shininess: 140, emissive: 0.15 + 0.8 * Math.exp(-(c - HIT) * 1.5) } });
        }
      }
      if (z < -39500 && z > -54500) {
        placeVortex(c);
        items.push({ mesh: cube, instances: vortex, material: { ambient: 0.05, diffuse: 0.3, emissive: 0.9, metal: 0.55, specular: 1.3, shininess: 110, lines: 1.1, grid: [1, 1], lineWidth: 0.04, lineColor: WHITE, pulse: 1.6, pulsePhase: c * 3, pulseDir: [0, 0, 0.0006] } });
      }
      if (z < -50000) items.push({ mesh: word, instances: wordAt, material: { metal: 1, iridescence: 0.4, specular: 1.9, shininess: 190, rim: 0.3, lines: 1.2, lineColor: [1.5, 1.2, 1.6], grid: [0, 0], lineWidth: 0.015, pulse: 1.8, pulseCount: 1, pulsePhase: c * 0.55 - 0.2, lineColor: [1.4, 0.9, 1.2] } });
      items.push({ points: stars, colors: starCol, size: 3.2, intensity: 2.3, streak: 0.5, fog: [9000, 24000] });
      if (c > HIT && c < HIT + 2.2) items.push({ points: sparks, size: 7, intensity: 4.5, streak: 0.9, fog: [6000, 14000] });
      const tunnel = z > -10400;
      const lights = [];
      if (z < -10000 && z > -28000) {
        // Neon pools: light from the trail heads on the floor and the towers they pass.
        const p = path.point(timing(c) + 900);
        lights.push({ position: [p[0] - 240, p[1] - 150, p[2]], color: CYAN, radius: 700, intensity: 1.6 });
        lights.push({ position: [p[0] + 240, p[1] - 150, p[2]], color: ORANGE, radius: 700, intensity: 1.6 });
      }
      if (c > HIT && c < HIT + 2.2) lights.push({ position: KNOT_C, color: [1.6, 1.2, 1.5], radius: 2400, intensity: 3 * Math.exp(-(c - HIT) * 1.6) });
      const near = c > 9.4 && c < 13.4 ? Math.hypot(cam.camera.x - KNOT_C[0], cam.camera.y - KNOT_C[1], cam.camera.z - KNOT_C[2]) : 0;
      const px = r3.render(cam, items, {
        background: '#020309', fog: tunnel ? [3000, 11000] : [9000, 30000], fogColor: [0.05, 0.018, 0.06],
        key: [0, 0.25, -1], keyColor: [1, 0.72, 0.9], fill: [-0.4, 0.6, 0.6], fillColor: [0.1, 0.12, 0.3], rimColor: [0.7, 0.5, 1],
        sky: [0.015, 0.015, 0.06], horizon: [0.95, 0.32, 0.68], ground: [0.01, 0.01, 0.02], strips: [1.8, 1.8, 2.2],
        skybox: tunnel ? false : { sun: 1.5 },
        mirror: floorOn ? { y: -300, strength: 0.48 } : undefined,
        lights,
        dof: near ? { focus: near, aperture: 10 } : c > LAND - 0.2 ? { focus: Math.abs(cam.camera.z - WORD_Z), aperture: 12 * smooth(LAND - 0.2, LAND + 1, c) } : undefined,
        motionBlur: { shutter: 0.55, max: 110 }, prevCamera,
      });
      ctx.putImageData(new globalThis.ImageData(px, W, H), 0, 0);
    },
  });
  // Impacts land on story beats of the slowed clock.
  film.impact(0, { flash: '#ffffff', flashOpacity: 1, decay: 4, aberration: 6 }); // ignition: the film opens on a white flash
  film.impact(beat(3.0), { shake: 14, flash: '#8fe3ff', flashOpacity: 0.35, aberration: 5, shockwave: 22 });
  film.impact(beat(HIT), { shake: 34, flash: '#ffd1ef', flashOpacity: 0.75, aberration: 10, shockwave: 40, origin: [0.5, 0.45], shockSpeed: 0.8 });
  film.impact(beat(14.25), { shake: 16, flash: '#ffffff', flashOpacity: 0.3, aberration: 6 });
  film.impact(beat(LAND), { shake: 10, aberration: 3, shockwave: 16, origin: [0.5, 0.56], shockSpeed: 0.7 });
  return film;
}
