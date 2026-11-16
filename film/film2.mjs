// ANIKIT — EVENT HORIZON. A hyperspace dive through a twisting tunnel of two thousand chrome
// blocks, out over a black mirror where a monolith bursts in bullet time, and on towards a
// black hole whose accretion disk (100k points of light) goes supernova. Every pixel is
// rendered by anikit's WebAssembly rasteriser; no WebGL.
//
//   anikit-render film2.mjs --still 6 --out still.jpg
//   anikit-render film2.mjs --out film2.mp4 --workers 3 --fade-out 0.4
export default async function film2({ anikit: A }) {
  const { composition, scene3d, renderer3d, mesh, instances, flightPath, shatter } = A;
  const W = 1920, H = 1080, FPS = 60, D = 17;
  let seed = 7;
  const rnd = () => ((seed = (seed * 1664525 + 1013904223) >>> 0) / 4294967296);
  const clamp = (x, a = 0, b = 1) => Math.min(b, Math.max(a, x));
  const smooth = (a, b, x) => { const t = clamp((x - a) / (b - a)); return t * t * (3 - 2 * t); };

  // ---- The tunnel: twisting rings of chrome blocks around a wandering centre line ----------
  const TUN_END = -16000;
  const cx = (z) => (z > TUN_END ? 450 * Math.sin((z * Math.PI) / 8000) : 0);
  const cy = (z) => (z > TUN_END ? 250 * Math.sin((z * Math.PI) / 4000) : 0);
  const RINGS = 92, SEG = 22, R0 = 320;
  const cube = mesh.cube();
  const blocks = instances(RINGS * SEG, (i) => {
    const k = Math.floor(i / SEG), j = i % SEG, z = 2400 - k * 200;
    const a = (j / SEG) * Math.PI * 2 + k * 0.11, r = R0 + rnd() * 70;
    const d = 0.008 + rnd() * 0.014;
    return { x: cx(z) + Math.cos(a) * r, y: cy(z) + Math.sin(a) * r, z, rz: (a * 180) / Math.PI, sx: 40 + rnd() * 50, sy: 80, sz: 110 + rnd() * 50, color: [d, d, d * 1.3] };
  });
  const ring = mesh.torus(1, 0.012, 160, 8);
  const NEON = Math.floor(RINGS / 4);
  const neonRings = instances(NEON, (i) => { const z = 2400 - i * 800 - 100; return { x: cx(z), y: cy(z), z, scale: R0 + 120, color: [1, 1, 1] }; });
  // Warp dust inside the tunnel and stars all around.
  const NDUST = 9000, dust = new Float32Array(NDUST * 3), dustCol = new Float32Array(NDUST * 3);
  for (let i = 0; i < NDUST; i++) {
    const z = 2400 - rnd() * 19000, a = rnd() * Math.PI * 2, r = 40 + rnd() * 230;
    dust.set([cx(z) + Math.cos(a) * r, cy(z) + Math.sin(a) * r, z], i * 3);
    dustCol.set(rnd() < 0.5 ? [0.5, 0.9, 1.6] : [1.6, 0.5, 1.2], i * 3);
  }
  const NST = 14000, stars = new Float32Array(NST * 3), starCol = new Float32Array(NST * 3);
  for (let i = 0; i < NST; i++) {
    const u = rnd() * 2 - 1, a = rnd() * Math.PI * 2, s = Math.sqrt(1 - u * u), R = 60000 + rnd() * 20000;
    stars.set([Math.cos(a) * s * R, u * R * 0.6, -30000 + Math.sin(a) * s * R], i * 3);
    const b = 0.4 + 1.6 * rnd() ** 4;
    starCol.set([b * 0.9, b * 0.95, b * 1.2], i * 3);
  }

  // ---- The hall: a black mirror floor and an avenue of chrome monoliths -------------------
  const FLOOR = -700;
  const floor = mesh.plane(1), floorAt = instances(1, () => ({ y: FLOOR, z: -40000, sx: 240000, sz: 240000, color: [0.012, 0.012, 0.02] }));
  const avenue = instances(14, (i) => {
    const side = i % 2 ? 1 : -1, z = -17500 - Math.floor(i / 2) * 1500;
    return { x: side * (1500 + rnd() * 400), y: FLOOR + 850, z, ry: side * 12, sx: 420, sy: 1700, sz: 110, color: [0.02, 0.02, 0.03] };
  });
  // The monolith that bursts: a finely subdivided slab, so it breaks into many shards.
  const slab = (w, h, d, nx, ny) => {
    const P = [], N = [], I = [];
    const face = (o, u, v, n, nu, nv) => {
      const b = P.length / 3;
      for (let y = 0; y <= nv; y++) for (let x = 0; x <= nu; x++) {
        const fu = x / nu - 0.5, fv = y / nv - 0.5;
        const jit = x > 0 && x < nu && y > 0 && y < nv ? 0.35 : 0; // irregular shards
        const ju = (rnd() - 0.5) * jit / nu, jv = (rnd() - 0.5) * jit / nv;
        P.push(o[0] + u[0] * (fu + ju) + v[0] * (fv + jv), o[1] + u[1] * (fu + ju) + v[1] * (fv + jv), o[2] + u[2] * (fu + ju) + v[2] * (fv + jv));
        N.push(...n);
      }
      for (let y = 0; y < nv; y++) for (let x = 0; x < nu; x++) {
        const a = b + y * (nu + 1) + x, c = a + nu + 1;
        I.push(a, a + 1, c + 1, a, c + 1, c);
      }
    };
    face([0, 0, d / 2], [w, 0, 0], [0, h, 0], [0, 0, 1], nx, ny);
    face([0, 0, -d / 2], [-w, 0, 0], [0, h, 0], [0, 0, -1], nx, ny);
    face([w / 2, 0, 0], [0, 0, -d], [0, h, 0], [1, 0, 0], 1, ny);
    face([-w / 2, 0, 0], [0, 0, d], [0, h, 0], [-1, 0, 0], 1, ny);
    return mesh.custom(new Float32Array(P), new Float32Array(N), new Uint32Array(I));
  };
  const MONO = { x: 0, y: FLOOR + 800, z: -21200, color: [0.025, 0.025, 0.035] };
  const monoMesh = slab(560, 1600, 140, 9, 24);
  const monoAt = instances(1, () => MONO);
  const shards = shatter(monoMesh, { transform: MONO, force: 1500, spin: 700, drag: 0.6, gravity: 250, center: [0, 0, 0] });

  // ---- The black hole: a void, a photon ring, a lensed halo and a 100k-point disk ---------
  const BH = [0, 3200, -45500], RS = 1700, TILT = (18 * Math.PI) / 180;
  const sphere = mesh.sphere(48, 96), holeAt = instances(1, () => ({ x: BH[0], y: BH[1], z: BH[2], scale: RS, color: [0, 0, 0] }));
  const photonAt = instances(1, () => ({ x: BH[0], y: BH[1], z: BH[2] + 200, scale: RS * 1.06, color: [1, 1, 1] }));
  const NDISK = 100000, dR = new Float32Array(NDISK), dA = new Float32Array(NDISK), dY = new Float32Array(NDISK), dV = new Float32Array(NDISK);
  const disk = new Float32Array(NDISK * 3), diskCol = new Float32Array(NDISK * 3), diskOut = new Float32Array(NDISK * 3);
  for (let i = 0; i < NDISK; i++) {
    const u = rnd(), r = RS * (1.55 + 4.2 * u * u);
    dR[i] = r; dA[i] = rnd() * Math.PI * 2; dY[i] = (rnd() - 0.5) * (40 + r * 0.035); dV[i] = 0.4 + rnd();
    const h = clamp((r / RS - 1.55) / 4); // hot white-gold inside, ember red outside
    const b = (1.6 - 1.1 * h) * (0.6 + 0.6 * rnd());
    diskCol.set([b * 1.0, b * (0.85 - 0.55 * h), b * (0.6 - 0.5 * h)], i * 3);
  }
  const NHALO = 30000, hR = new Float32Array(NHALO), hA = new Float32Array(NHALO);
  const halo = new Float32Array(NHALO * 3), haloCol = new Float32Array(NHALO * 3);
  for (let i = 0; i < NHALO; i++) {
    hR[i] = RS * (1.12 + 0.9 * rnd() ** 2); hA[i] = rnd() * Math.PI * 2;
    const b = 1.4 * (1.2 - (hR[i] / RS - 1.12)) * (0.5 + 0.5 * rnd());
    haloCol.set([b, b * 0.75, b * 0.45], i * 3);
  }
  const NSHELL = 60000, shDir = new Float32Array(NSHELL * 3), shV = new Float32Array(NSHELL), shell = new Float32Array(NSHELL * 3), shellCol = new Float32Array(NSHELL * 3);
  for (let i = 0; i < NSHELL; i++) {
    const u = rnd() * 2 - 1, a = rnd() * Math.PI * 2, s = Math.sqrt(1 - u * u);
    shDir.set([Math.cos(a) * s, u * 0.55, Math.sin(a) * s], i * 3); shV[i] = 0.7 + 0.3 * rnd() ** 0.5;
    const hot = rnd();
    shellCol.set(hot < 0.5 ? [1.6, 1.3, 1.0] : hot < 0.8 ? [0.6, 0.9, 1.8] : [1.8, 0.5, 1.2], i * 3);
  }

  // ---- Camera ----------------------------------------------------------------------------
  const pts = [];
  for (let z = 2400; z >= TUN_END; z -= 2000) pts.push([cx(z), cy(z), z]);
  pts.push([0, 0, -18500], [0, -20, -20800]);
  // A log-spiral towards the hole: 28° of orbit while the distance falls from 21 km to 3.8 km.
  const spiral = (u) => { const th = (28 * Math.PI / 180) * u, d = 21000 * (2700 / 21000) ** u, h = -2900 + 4300 * (u * u * (3 - 2 * u)); return [BH[0] - d * Math.sin(th), BH[1] + h, BH[2] + d * Math.cos(th)]; };
  for (let k = 0; k <= 8; k++) pts.push(spiral(k / 8));
  const path = flightPath(pts, { lookAhead: 900, bank: 0.15, maxBank: 16, bankWindow: 250 });
  const NP = 2000, PP = path.points(NP + 1);
  const fracAtZ = (z) => { for (let i = 0; i <= NP; i++) if (PP[i][2] <= z) return i / NP; return 1; };
  const fracNear = (q) => { let bi = 0, bd = Infinity; for (let i = 0; i <= NP; i++) { const d = (PP[i][0] - q[0]) ** 2 + (PP[i][1] - q[1]) ** 2 + (PP[i][2] - q[2]) ** 2; if (d < bd) { bd = d; bi = i; } } return bi / NP; };
  const HIT = 5.55, SN = 12.6;
  const s = path.timing([[0, 0], [4.5, fracAtZ(TUN_END)], [HIT, fracAtZ(-20250)], [6.6, fracAtZ(-22300)], [8.0, fracNear(spiral(0.2))], [10.0, fracNear(spiral(0.55))], [12.6, fracNear(spiral(0.86))], [16.5, 1]]);
  const cam = scene3d({ fov: 70, width: W, height: H, near: 4, far: 140000 });
  const pose = (c) => {
    path.apply(cam, s(Math.max(0, c)), 0, { shake: 1.5 + 6 * Math.exp(-(((c - HIT) / 0.4) ** 2)) + 10 * smooth(SN, SN + 0.2, c) * Math.exp(-(c - SN) * 1.6), time: c, rate: 2 });
    const w = 0.85 * smooth(6.8, 9.0, c), tg = cam.target;
    tg.x += (BH[0] - tg.x) * w; tg.y += (BH[1] - tg.y) * w; tg.z += (BH[2] - tg.z) * w;
  };

  const r3 = renderer3d({ width: W, height: H });
  // Where the black hole is on screen (for rays and the supernova shockwave).
  const screenOf = (p) => {
    const m = cam.viewProjection().matrix;
    const w = m[3] * p[0] + m[7] * p[1] + m[11] * p[2] + m[15];
    if (w <= 0) return [0.5, 0.5];
    return [0.5 + 0.5 * (m[0] * p[0] + m[4] * p[1] + m[8] * p[2] + m[12]) / w, 0.5 - 0.5 * (m[1] * p[0] + m[5] * p[1] + m[9] * p[2] + m[13]) / w];
  };

  const holeScreen = new Map();
  const film = composition({
    width: W, height: H, fps: FPS, duration: D, motionBlur: 1,
    post: {
      aa: 1,
      bloom: (t) => 0.95 - 0.15 * smooth(SN - 0.3, SN + 0.3, t) * Math.exp(-Math.max(0, t - SN - 0.6)),
      bloomThreshold: 0.42, bloomRadius: 50,
      streaks: (t) => 0.45 + 0.35 * smooth(10, 13, t), streakLength: 340,
      rays: (t) => 0.7 * smooth(7.5, 10, t), raysCenter: (t) => holeScreen.get(Math.round(t * FPS)) ?? [0.5, 0.45], raysLength: 0.5,
      flares: 0,
      aberration: (t) => 0.6 + 0.9 * (1 - smooth(0, 4.4, t)), zoomBlur: (t) => 0.04 * (1 - smooth(3.8, 4.8, t)),
      vignette: 0.5, grain: 3.5, contrast: 1.08, saturation: 1.12, exposure: 1.02,
      shadows: [0.0, 0.004, 0.02], highlights: [0.03, 0.012, -0.01],
    },
  });

  let prevClock = 0, lastClock = 0;
  film.scene('dive', 0, D, {
    timeScale: (t) => 1 - 0.82 * Math.exp(-(((t - HIT - 0.3) / 0.5) ** 2)),
    step({ clock }) { prevClock = lastClock; lastClock = clock; },
    draw({ ctx, clock: c, t }) {
      pose(Math.min(prevClock, c - 1e-4));
      const prevCamera = cam.viewProjection().matrix;
      pose(c);
      const e = cam.camera;
      const hk = smooth(0.8, 2.4, c), hk2 = smooth(2.6, 4.2, c);
      const tunHue = [0.3 + 1.5 * hk - 0.0 * hk2, 0.9 - 0.6 * hk + 0.75 * hk2, 1.9 - 0.6 * hk - 1.1 * hk2];
      const stripK = (0.55 + 0.45 * smooth(4.2, 4.8, c)) * (1 - smooth(6.6, 8.6, c));
      holeScreen.set(Math.round(t * FPS), screenOf(BH));
      // Disk orbits (inner rings faster); after the supernova it is flung outward.
      const sn = Math.max(0, c - SN);
      const cT = Math.cos(TILT), sT = Math.sin(TILT);
      const vAz = Math.atan2(e.x - BH[0], e.z - BH[2]); // approaching side (towards the camera) is brighter and bluer
      for (let i = 0; i < NDISK; i++) {
        const r0 = dR[i], a = dA[i] + 2.6 * c * (RS * 1.55 / r0) ** 1.5;
        const r = r0 + sn * (700 + 2600 * dV[i]) * (1 - Math.exp(-sn * 1.2)) / 0.8;
        const x = r * Math.cos(a), z0 = r * Math.sin(a), y0 = dY[i] * (1 + sn * 6);
        const f = 1 + 0.75 * Math.cos(a - vAz); diskOut[i * 3] = diskCol[i * 3] * f; diskOut[i * 3 + 1] = diskCol[i * 3 + 1] * f * f; diskOut[i * 3 + 2] = diskCol[i * 3 + 2] * f * f * f;
        disk[i * 3] = BH[0] + x; disk[i * 3 + 1] = BH[1] + y0 * cT - z0 * sT; disk[i * 3 + 2] = BH[2] + y0 * sT + z0 * cT;
      }
      let vx = e.x - BH[0], vy = e.y - BH[1], vz = e.z - BH[2]; const vl = Math.hypot(vx, vy, vz); vx /= vl; vy /= vl; vz /= vl;
      let U1 = [vz, 0, -vx]; const ul = Math.hypot(U1[0], U1[2]); U1 = [U1[0] / ul, 0, U1[2] / ul];
      const U2 = [vy * U1[2] - vz * U1[1], vz * U1[0] - vx * U1[2], vx * U1[1] - vy * U1[0]];
      for (let i = 0; i < NHALO; i++) {
        const a = hA[i] + 1.4 * c, r = hR[i] * (1 + 0.08 * smooth(0, 1.5, sn));
        const ca = Math.cos(a) * r, sa = Math.sin(a) * r * 0.92;
        halo[i * 3] = BH[0] + ca * U1[0] + sa * U2[0]; halo[i * 3 + 1] = BH[1] + ca * U1[1] + sa * U2[1]; halo[i * 3 + 2] = BH[2] + ca * U1[2] + sa * U2[2];
      }
      const items = [
        { mirror: true, mesh: floor, instances: floorAt, material: { metal: 0.1, specular: 0.1, lines: 0.7 * (1 - smooth(7, 9.5, c)), lineColor: [0.25, 0.5, 1.6], grid: [480, 480], lineWidth: 0.012, pulse: 1.2 * (1 - smooth(7, 9.5, c)), pulseCount: 3, pulsePhase: c * 0.5, pulseDir: [0, 0, 0.0002] } },
        { mesh: cube, instances: blocks, material: { metal: 0.95, iridescence: 0.25, specular: 1.2, shininess: 160, rim: 0.12, lines: 2.0, lineColor: tunHue, grid: [1, 1], lineWidth: 0.06, pulse: 2.6, pulseCount: 4, pulsePhase: c * 2.2, pulseDir: [0, 0, 0.0005] } },
        { mesh: cube, instances: avenue, material: { metal: 0.95, iridescence: 0.4, specular: 1.6, shininess: 160, rim: 0.25, lines: 1.4, lineColor: [1.7, 0.35, 1.1], grid: [2, 7], lineWidth: 0.025, pulse: 2.4, pulsePhase: c * 1.5, pulseDir: [0, 0.0012, 0] } },
        c < HIT
          ? { mesh: monoMesh, instances: monoAt, material: { metal: 0.95, iridescence: 0.7, specular: 1.8, shininess: 180, rim: 0.4 } }
          : { mesh: shards.mesh, instances: (shards.update(c - HIT), shards.instances), cull: false, material: { metal: 0.95, iridescence: 0.6, specular: 1.8, shininess: 180, emissive: 0.35 * Math.exp(-(c - HIT) * 1.2) + 0.05, rim: 0.5 } },
        { mesh: sphere, instances: holeAt, material: { ambient: 0, diffuse: 0, specular: 0, rim: 0, metal: 0, iridescence: 0, fog: 0 } },
        { mesh: ring, instances: neonRings, blend: 'additive', material: { ambient: 0, diffuse: 0, specular: 0, rim: 0, emissive: 1.1, pulse: 4, pulseCount: 3, pulsePhase: c * 3, lineColor: [1.8, 0.4, 1.3] } },
        { points: stars, colors: starCol, size: 70, intensity: 1.6, streak: 0.6 },
        { points: dust, colors: dustCol, size: 1.6, intensity: 1.6, streak: 3, fog: [1800, 4500] },
        { points: disk, colors: diskOut, size: 11 + sn * 4, intensity: 0.24 * (1 - 0.25 * smooth(0, 0.6, sn)), streak: 1 },
        { points: halo, colors: haloCol, size: 10, intensity: 0.6 * (1 + 0.8 * smooth(SN + 0.3, SN + 1.5, c)) },
      ];
      if (sn > 0) {
        const R = RS * 1.15 + 9000 * (1 - Math.exp(-sn * 0.9)) + 1200 * sn;
        for (let i = 0; i < NSHELL; i++) {
          const k = R * shV[i];
          shell[i * 3] = BH[0] + shDir[i * 3] * k; shell[i * 3 + 1] = BH[1] + shDir[i * 3 + 1] * k; shell[i * 3 + 2] = BH[2] + shDir[i * 3 + 2] * k;
        }
        items.push({ points: shell, colors: shellCol, size: 15, intensity: 0.7 * smooth(0, 0.6, sn) * Math.exp(-Math.max(0, sn - 0.6) * 0.22), streak: 1 });
      }
      // Point lights travel with the camera through the tunnel, then sit by the monoliths.
      const lights = [
        { position: [e.x, e.y + 60, e.z - 700], color: [0.4, 0.9, 1.6], radius: 1600, intensity: 1.3 },
        { position: [0, FLOOR + 200, -21000], color: [1.6, 0.4, 1.1], radius: 4500, intensity: 1.2 + 4 * Math.exp(-(((c - HIT) / 0.35) ** 2)) },
      ];
      const px = r3.render(cam, items, { sort: 'near',
        background: '#020207', fog: [14000, 60000], fogColor: [0.01, 0.008, 0.025],
        sky: [0.02, 0.02, 0.06], horizon: [0.3, 0.12, 0.35], ground: [0.01, 0.01, 0.02], strips: [1.6 * stripK, 1.6 * stripK, 2.0 * stripK],
        key: [0.2, 0.5, 1], keyColor: [0.5, 0.45, 0.7], fill: [0, -1, 0], fillColor: [0.05, 0.05, 0.12], rimColor: [0.6, 0.9, 1.6], lights,
        mirror: { y: FLOOR, strength: 0.3 }, motionBlur: { shutter: c < 4.6 ? 0.2 : 0.45, max: c < 4.6 ? 24 : 80 }, prevCamera,
      });
      ctx.putImageData(new globalThis.ImageData(px, W, H), 0, 0);
    },
  });
  // Punches on story beats; timeAt() maps the slowed scene clock to film time.
  film.impact(0, { flash: '#cfe8ff', flashOpacity: 0.85, decay: 5, shockwave: 20, origin: [0.5, 0.5] });
  film.impact(film.timeAt('dive', 4.5), { shake: 10, flash: '#9fd0ff', flashOpacity: 0.25, aberration: 6, decay: 8 });
  film.impact(film.timeAt('dive', HIT), { shake: 26, flash: '#e6c8ff', flashOpacity: 0.55, aberration: 9, shockwave: 34, origin: [0.5, 0.48] });
  pose(SN);
  film.impact(film.timeAt('dive', SN), { shake: 30, flash: '#fff2dc', flashOpacity: 0.5, aberration: 10, decay: 4, shockwave: 46, origin: screenOf(BH) });
  return film;
}
