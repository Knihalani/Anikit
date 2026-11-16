// ANIKIT — ONE WORLD, THREE LOOKS. A single continuous flight through one valley at golden
// hour that keeps changing how it is drawn: photographic, neon, comic. Every frame is
// rendered by anikit (WebAssembly rasteriser, no WebGL); the looks are materials, lights and
// a per-pixel ink pass over the same geometry, and the transitions are driven by the depth
// buffer, so they sweep through the world rather than across the screen.
//
//   anikit-render film.mjs --still 6 --out still.jpg          (the first run bakes the world: ~1 min)
//   anikit-render film.mjs --review review/
//   anikit-render film.mjs --out anikit-film.mp4 --workers 3 --fade-out 0.5
import { buildWorld, boulder, SUN, WATER, HAZE_DISPLAY, clamp, mix, smooth, hash, noise } from './world.mjs';

/** Filled in by film(): the camera pose function and duration, for scripts that check the flight. */
export const debug = {};

export default async function film({ anikit: A }) {
  const { composition, scene3d, renderer3d, mesh, instances } = A;
  const W = 1920, H = 1080, FPS = 60;
  const world = await buildWorld(A);
  const { path, groundAt, centerX, trees, towers, textures: TX } = world;

  // ---- Speed along the route (m/s by z), integrated into a time → path parameter map --------
  const VZ = [[1300, 64], [1200, 104], [1060, 112], [700, 122], [620, 94], [520, 90], [470, 100], [180, 102], [120, 116], [-400, 122], [-480, 104], [-700, 94], [-830, 84], [-1100, 66]];
  const speedAt = (z) => {
    if (z >= VZ[0][0]) return VZ[0][1];
    for (let i = 0; i < VZ.length - 1; i++) if (z >= VZ[i + 1][0]) { const t = (VZ[i][0] - z) / (VZ[i][0] - VZ[i + 1][0]); return mix(VZ[i][1], VZ[i + 1][1], t * t * (3 - 2 * t)); }
    return VZ[VZ.length - 1][1];
  };
  const NS = 6000, P = path.points(NS + 1), TT = new Float64Array(NS + 1);
  for (let i = 1; i <= NS; i++) {
    const a = P[i - 1], b = P[i];
    TT[i] = TT[i - 1] + Math.hypot(b[0] - a[0], b[1] - a[1], b[2] - a[2]) / (1.15 * speedAt((a[2] + b[2]) / 2));
  }
  const sOf = (t) => {
    if (t <= 0) return 0;
    let lo = 0, hi = NS;
    while (hi - lo > 1) { const m = (lo + hi) >> 1; if (TT[m] < t) lo = m; else hi = m; }
    return ((lo + clamp((t - TT[lo]) / (TT[hi] - TT[lo] || 1))) / NS) * path.length;
  };
  const tAtZ = (z) => { for (let i = 1; i <= NS; i++) if (P[i][2] <= z) return TT[i]; return TT[NS]; };
  const END = tAtZ(-1065);
  if (process.env.FILM_DEBUG) console.error('times', JSON.stringify({ END, saddle: tAtZ(575), cityEnd: tAtZ(185), c1: tAtZ(-60), c2: tAtZ(-330), climb: tAtZ(-560), ridge: tAtZ(-770) }));
  const DUR = Math.round(END * FPS) / FPS;

  const cam = scene3d({ fov: 72, width: W, height: H, near: 1.5, far: 60000 });
  // A steady drone: a faint drift only. On the steep climb out of the canyon the aim is held lower
  // (the horizon and the forest stay in frame rather than an empty sky).
  const pose = (t) => {
    const p = path.apply(cam, sOf(t), 0, { shake: 0.22, time: t, rate: 0.6 });
    const z = cam.camera.z, dip = 34 * smooth(-460, -545, z) * (1 - smooth(-640, -760, z));
    cam.target.y -= dip;
    return p;
  };
  Object.assign(debug, { pose, cam, DUR, FPS });

  // ---- Looks and transitions: a schedule anchored to places on the route --------------------
  // R = photographic, N = neon, C = comic.
  const tr = (c, dur, from, to, type) => ({ t0: c - dur / 2, t1: c + dur / 2, from, to, type });
  const SCHEDULE = [
    tr(3.8, 0.8, 'R', 'C', 'front'),     // over the lake: the comic world sweeps in from the horizon
    tr(6.6, 0.6, 'C', 'N', 'glitch'),    // over the saddle, scan lines into the neon city
    tr(10.0, 0.7, 'N', 'C', 'ink'),      // into the canyon: an ink bloom
    tr(12.8, 0.7, 'C', 'N', 'dots'),     // halftone dots dissolve into a neon river
    tr(15.4, 0.6, 'N', 'R', 'slash'),    // out of the canyon: daylight wipes across
    tr(18.25, 1.0, 'R', 'C', 'reveal'),   // over the last ridge a comic world grows out from the lens
  ];
  const TEASE = [0.25, 1.9]; // the hook: during the dive the frame splits into all three looks
  const OUTRO = DUR - 1.35;
  const ease = (p) => p * p * (3 - 2 * p);
  const state = (t) => {
    if (t > TEASE[0] && t < TEASE[1]) { const p = (t - TEASE[0]) / (TEASE[1] - TEASE[0]); return { tease: p, inn: ease(clamp(p / 0.2)), out: ease(clamp((p - 0.8) / 0.2)) }; }
    if (t > OUTRO) return { tease: 0.5, inn: ease(clamp((t - OUTRO) / 0.5)), out: 0, mid: 'C', right: 'R' }; // the bookend: all three again
    let look = 'R';
    for (const s of SCHEDULE) {
      if (t < s.t0) return { look };
      if (t < s.t1) return { from: s.from, to: s.to, type: s.type, p: (t - s.t0) / (s.t1 - s.t0) };
      look = s.to;
    }
    return { look };
  };
  // Weights of each look on screen (for the grade).
  const weights = (t) => {
    const s = state(t), w = { R: 0, N: 0, C: 0 };
    if (s.tease !== undefined) { const k = s.inn * (1 - s.out), mid = s.mid ?? 'R'; w.R = w.N = w.C = 0.33 * k; w[mid] = 1 - 0.66 * k; }
    else if (s.look) w[s.look] = 1;
    else { const e = ease(s.p); w[s.from] += 1 - e; w[s.to] += e; }
    return w;
  };

  // ---- Geometry --------------------------------------------------------------------------------
  const one = () => instances(1, () => ({}));
  const terrainAt = one(), farAt = one();
  const water = mesh.plane(1, 1), waterAt = instances(1, () => ({ sx: 40000, sz: 40000, y: WATER }));
  // The sky: an upper hemisphere (equirectangular UVs) resting on the water level, so the
  // reflection pass shows through below the horizon.
  const dome = (() => {
    const R = 40, S = 128, Pp = [], Nn = [], Uu = [], Ii = [];
    for (let i = 0; i <= R; i++) {
      const th = (i / R) * Math.PI / 2;
      for (let j = 0; j <= S; j++) {
        const ph = (j / S) * Math.PI * 2, d = [Math.sin(th) * Math.cos(ph), Math.cos(th), Math.sin(th) * Math.sin(ph)];
        Pp.push(...d); Nn.push(-d[0], -d[1], -d[2]); Uu.push(j / S, (i / R) * 0.5);
      }
    }
    for (let i = 0; i < R; i++) for (let j = 0; j < S; j++) { const a = i * (S + 1) + j, b = a + S + 1; Ii.push(a, b, b + 1, a, b + 1, a + 1); }
    return mesh.custom(new Float32Array(Pp), new Float32Array(Nn), new Uint32Array(Ii), new Float32Array(Uu));
  })();
  const domeAt = one();
  const sunBall = mesh.sphere(12, 24), sunAt = instances(1, () => ({ color: [1, 0.86, 0.62] }));
  const cube = mesh.cube();
  // Trees in three silhouettes (fir, spruce, broadleaf), each with its own colour range.
  const TM = world.treeMeshes;
  // Chunked along z (the flight runs towards −z), so chunks wholly behind the camera are skipped.
  const CHUNK = 160, chunks = [];
  for (const k of [0, 1, 2]) {
    const by = new Map();
    for (const t of trees) if (t.kind === k) { const b = Math.floor(t.z / CHUNK); if (!by.has(b)) by.set(b, []); by.get(b).push(t); }
    for (const list of by.values()) chunks.push({ kind: k, list, zmin: Math.min(...list.map((t) => t.z)), zmax: Math.max(...list.map((t) => t.z)) });
  }
  const treeSet = (col) => chunks.map(({ list }) => instances(list.length, (i) => {
    const t = list[i], c = col(t);
    return { x: t.x, y: t.y, z: t.z, sx: t.h * t.w, sy: t.h, sz: t.h * t.w, ry: t.ry, color: c };
  }));
  const TREE_R = [[0.035, 0.1, 0.045], [0.03, 0.085, 0.07], [0.08, 0.15, 0.045]];
  const treesR = treeSet((t) => {
    const v = 0.45 + 0.55 * world.visAt(t.x, t.z), k = 0.75 + 0.5 * t.tint, base = TREE_R[t.kind];
    const autumn = t.kind === 2 && t.tint > 0.88 ? [2.2, 1.0, 0.5] : [1, 1, 1];
    return [base[0] * v * k * autumn[0], base[1] * v * (0.85 + 0.3 * t.tint) * autumn[1], base[2] * v * autumn[2]];
  });
  const treesN = treeSet((t) => (t.tint > 0.5 ? [0.0, 0.035, 0.05] : [0.04, 0.0, 0.045]));
  const TREE_C = [[[0.12, 0.5, 0.3], [0.1, 0.42, 0.26]], [[0.1, 0.44, 0.36], [0.08, 0.36, 0.32]], [[0.5, 0.72, 0.22], [0.92, 0.6, 0.2]]];
  const treesC = treeSet((t) => TREE_C[t.kind][t.tint > 0.6 ? 1 : 0]);
  // Boulders along the shores and the lower slopes near the flight line.
  const rockMesh = boulder(mesh, 3.1);
  const rocks = [];
  for (let k = 0; k < 20000 && rocks.length < 1400; k++) {
    const z = mix(1250, -1150, hash(k * 3 + 11)), side = hash(k * 3 + 12) < 0.5 ? -1 : 1;
    const x = centerX(z) + side * (12 + 230 * hash(k * 3 + 13) ** 1.6);
    const h = groundAt(x, z);
    if (h < -0.8 || h > 220) continue;
    const sl = Math.hypot(groundAt(x + 3, z) - groundAt(x - 3, z), groundAt(x, z + 3) - groundAt(x, z - 3)) / 6;
    if (sl > 0.9) continue;
    const sz = (1.2 + 5.5 * hash(k * 7 + 1) ** 2.2) * (h < 1.5 ? 1.4 : 1);
    rocks.push({ x, z, y: h - sz * 0.25, s: sz, r: hash(k * 7 + 2) * 360, v: hash(k * 7 + 3) });
  }
  const rockSet = (col) => instances(rocks.length, (i) => { const r = rocks[i]; return { x: r.x, y: r.y, z: r.z, sx: r.s * (0.8 + 0.4 * r.v), sy: r.s, sz: r.s, ry: r.r, color: col(r) }; });
  const rocksR = rockSet((r) => { const v = 0.5 + 0.5 * world.visAt(r.x, r.z); return [0.3 * v * (0.85 + 0.3 * r.v), 0.28 * v, 0.25 * v]; });
  const rocksN = rockSet(() => [0.015, 0.012, 0.025]);
  const rocksC = rockSet((r) => (r.v > 0.5 ? [0.66, 0.6, 0.55] : [0.56, 0.5, 0.48]));
  // Buildings in three height classes (so the facade texture keeps its window size).
  const CLS = [[0.82, 1.45], [0.82, 0.8], [0.82, 0.32]];
  const PAL = [[0.95, 0.36, 0.3], [0.98, 0.8, 0.3], [0.32, 0.7, 0.86], [0.92, 0.92, 0.88], [0.55, 0.45, 0.85]];
  // Setbacks: taller towers step in once or twice towards the top; every roof gets plant rooms.
  const blocks = [], roofs = [];
  towers.forEach((b, i) => {
    let { x, z, w, d, h } = b; const fl = b.y - h / 2;
    const tiers = h > 55 && hash(i * 5 + 1) < 0.7 ? 2 + (h > 85 && hash(i * 5 + 2) < 0.5 ? 1 : 0) : 1;
    let y0 = fl;
    for (let k = 0; k < tiers; k++) {
      const th = tiers === 1 ? h : k === 0 ? h * 0.6 : h * (0.4 / (tiers - 1)) * (k === tiers - 1 ? 1.15 : 0.85);
      blocks.push({ x, z, w, d, h: th, y: y0 + th / 2, cls: th > 60 ? 0 : th > 30 ? 1 : 2, id: i });
      y0 += th;
      if (k < tiers - 1) { const sh = 0.72 + 0.12 * hash(i * 9 + k); w *= sh; d *= sh; x += (hash(i * 3 + k) - 0.5) * w * 0.2; }
    }
    const nr = 1 + Math.floor(hash(i * 7 + 4) * 3);
    for (let k = 0; k < nr; k++) {
      const rw = w * (0.18 + 0.25 * hash(i * 11 + k)), rd = d * (0.18 + 0.25 * hash(i * 13 + k)), rh = 1.5 + 3.5 * hash(i * 17 + k);
      roofs.push({ x: x + (hash(i * 19 + k) - 0.5) * (w - rw) * 0.8, z: z + (hash(i * 23 + k) - 0.5) * (d - rd) * 0.8, w: rw, d: rd, h: rh, y: y0 + rh / 2 - 0.2, id: i });
    }
  });
  const FACADE = [[0.86, 0.83, 0.77], [0.92, 0.77, 0.6], [0.58, 0.7, 0.82], [0.68, 0.52, 0.45], [0.7, 0.72, 0.72]];
  const roofSet = (col) => instances(roofs.length, (i) => { const r = roofs[i]; return { x: r.x, y: r.y, z: r.z, sx: r.w, sy: r.h, sz: r.d, color: col(r) }; });
  const roofsR = roofSet((r) => { const v = 0.55 + 0.25 * hash(r.id * 29); return [v, v * 0.98, v * 0.95]; });
  const roofsN = roofSet(() => [0.02, 0.02, 0.035]);
  const roofsC = roofSet(() => [0.85, 0.85, 0.82]);
  const towerSet = (cls, col) => {
    const list = blocks.filter((b) => b.cls === cls);
    return instances(list.length, (i) => { const b = list[i]; return { x: b.x, y: b.y, z: b.z, sx: b.w, sy: b.h, sz: b.d, color: col(b, i) }; });
  };
  const towersR = [0, 1, 2].map((c) => towerSet(c, (b) => { const f = FACADE[Math.floor(hash(b.id * 31 + 7) * FACADE.length)], v = 0.85 + 0.25 * hash(b.id * 7); return f.map((q) => q * v); }));
  const towersN = [0, 1, 2].map((c) => towerSet(c, () => [0.025, 0.025, 0.04]));
  const towersC = [0, 1, 2].map((c) => towerSet(c, (b) => PAL[Math.floor(hash(b.id * 13 + 5) * PAL.length)]));

  // Neon light trails along the flight line (a glowing river under the drone, lanes either side).
  const lanes = [0, -1, 1].map((side, k) => {
    const pts = [];
    for (let z = 545; z >= -790; z -= 5) {
      const w = world.profile(z, 2), off = side * (w < 60 ? 11 : 24);
      const x = centerX(z) + off;
      pts.push([x, Math.max(groundAt(x, z), WATER) + (k === 0 ? 1.2 : 1.8), z]);
    }
    return mesh.tube(pts, { radius: k === 0 ? 0.55 : 0.4, sides: 6 });
  });
  const laneCol = [[0.35, 1.0, 1.8], [1.8, 0.3, 1.2], [1.8, 0.75, 0.25]];
  // Points: sunlit pollen in the air (photographic), floating motes (neon), stars (neon).
  const motes = new Float32Array(9000 * 3);
  for (let i = 0; i < 9000; i++) {
    const z = mix(1150, -1100, hash(i * 3 + 1)), x = centerX(z) + (hash(i * 3 + 2) * 2 - 1) * 70;
    motes.set([x, Math.max(groundAt(x, z), 0) + 2 + hash(i * 3 + 3) * 45, z], i * 3);
  }
  const moteCol = new Float32Array(9000 * 3), moteColN = new Float32Array(9000 * 3);
  for (let i = 0; i < 9000; i++) {
    moteCol.set([1.0, 0.82, 0.55], i * 3);
    moteColN.set(hash(i) < 0.5 ? [0.4, 1.0, 1.6] : [1.6, 0.4, 1.2], i * 3);
  }
  const stars = new Float32Array(2500 * 3), starCol = new Float32Array(2500 * 3);
  for (let i = 0; i < 2500; i++) {
    const a = hash(i * 5) * Math.PI * 2, e = Math.asin(0.04 + 0.96 * hash(i * 5 + 1));
    stars.set([Math.cos(a) * Math.cos(e) * 15000, Math.sin(e) * 15000, Math.sin(a) * Math.cos(e) * 15000], i * 3);
    const b = 0.4 + 1.2 * hash(i * 5 + 2) ** 3;
    starCol.set([b, b, b * 1.15], i * 3);
  }

  // ---- The three looks ---------------------------------------------------------------------------
  const ENV_R = { sky: [0.32, 0.45, 0.7], horizon: [1.0, 0.8, 0.6], ground: [0.1, 0.09, 0.07], strips: [0, 0, 0] };
  const baked = { ambient: 1, diffuse: 0, specular: 0, rim: 0 };
  const items = {
    R: [
      { mesh: world.terrainMesh, instances: terrainAt, material: { ...baked, texture: TX.real, textureFilter: 'aniso' } },
      { mesh: world.wallMesh, instances: terrainAt, material: { ...baked, texture: TX.wallReal, textureFilter: 'aniso' } },
      { mesh: rockMesh, instances: rocksR, material: { ambient: 0.32, diffuse: 1.05, specular: 0.12, shininess: 10, rim: 0.15 } },
      ...chunks.map((ch, k) => ({ mesh: TM[ch.kind], instances: treesR[k], zmin: ch.zmin, zmax: ch.zmax, material: { ambient: 0.5, diffuse: 1.0, specular: 0.06, shininess: 6, rim: 0.34 } })),
      ...towersR.map((n, c) => ({ mesh: cube, instances: n, material: { ambient: 0.85, diffuse: 0.9, specular: 0.5, shininess: 60, rim: 0.2, texture: TX.windows, textureRepeat: CLS[c] } })),
      { mesh: cube, instances: roofsR, material: { ambient: 0.7, diffuse: 0.9, specular: 0.1, shininess: 10, rim: 0.15 } },
      { mesh: world.farMesh, instances: farAt, material: { ...baked, texture: TX.farReal } },
      { mesh: dome, instances: domeAt, cull: false, reflect: true, material: { ambient: 0, diffuse: 0, specular: 0, rim: 0, texture: TX.sky, textureMode: 'emissive', fog: 0 } },
      { mesh: water, instances: waterAt, mirror: true, material: { ...baked, ambient: 0.25 } },
      { mesh: sunBall, instances: sunAt, blend: 'additive', material: { ambient: 0, diffuse: 0, specular: 0, rim: 0, emissive: 2.6, fog: 0 } },
      { points: motes, colors: moteCol, size: 0.05, intensity: 0.9, streak: 1, fog: [6, 90] },
    ],
    N: [
      { mesh: world.terrainMesh, instances: terrainAt, material: { ambient: 1, diffuse: 0.9, specular: 0, rim: 0, texture: TX.neon, textureMode: 'emissive', textureGain: 1.25, textureFilter: 'aniso' } },
      { mesh: world.wallMesh, instances: terrainAt, material: { ambient: 1, diffuse: 0.9, specular: 0, rim: 0, texture: TX.wallNeon, textureMode: 'emissive', textureGain: 1.25, textureFilter: 'aniso' } },
      { mesh: rockMesh, instances: rocksN, material: { ambient: 1, diffuse: 0.4, specular: 0.6, shininess: 30, rim: 0.9 } },
      ...towersN.map((n, c) => ({ mesh: cube, instances: n, material: { ambient: 1, diffuse: 0.6, specular: 0.9, shininess: 80, rim: 0.3, texture: TX.windowsNeon, textureMode: 'emissive', textureGain: 1.5, textureRepeat: CLS[c], lines: 1.1, lineColor: [1.5, 0.35, 1.1], grid: [1, 1], lineWidth: 0.012 } })),
      { mesh: cube, instances: roofsN, material: { ambient: 1, diffuse: 0.6, specular: 0.9, shininess: 80, rim: 0.3, lines: 1.1, lineColor: [0.35, 1.0, 1.6], grid: [1, 1], lineWidth: 0.04 } },
      { mesh: world.farMesh, instances: farAt, material: { ambient: 1, diffuse: 0, specular: 0, rim: 0, texture: TX.farNeon, textureMode: 'emissive', textureGain: 1.3 } },
      { mesh: dome, instances: domeAt, cull: false, material: { ambient: 0, diffuse: 0, specular: 0, rim: 0, texture: TX.skyNeon, textureMode: 'emissive', fog: 0 } },
      { mesh: water, instances: waterAt, mirror: true, material: { ambient: 1, diffuse: 0, specular: 0, rim: 0 } },
      ...lanes.map((m, k) => ({ mesh: m, blend: 'additive', material: { ambient: 0, diffuse: 0, specular: 0, rim: 0, emissive: 0.12, pulse: 2.1, pulseCount: k === 0 ? 34 : 48, pulsePhase: 0, lineColor: laneCol[k] } })),
      ...chunks.map((ch, k) => ({ mesh: TM[ch.kind], instances: treesN[k], zmin: ch.zmin, zmax: ch.zmax, material: { ambient: 1, diffuse: 0, specular: 0, rim: 0.85 } })),
      { points: motes, colors: moteColN, size: 0.09, intensity: 1.8, streak: 1, fog: [6, 120] },
    ],
    C: [
      { mesh: world.terrainMesh, instances: terrainAt, material: { ...baked, texture: TX.comic, textureFilter: 'aniso' } },
      { mesh: world.wallMesh, instances: terrainAt, material: { ...baked, texture: TX.wallComic, textureFilter: 'aniso' } },
      { mesh: rockMesh, instances: rocksC, material: { ambient: 0.6, diffuse: 0.7, specular: 0, rim: 0 } },
      ...chunks.map((ch, k) => ({ mesh: TM[ch.kind], instances: treesC[k], zmin: ch.zmin, zmax: ch.zmax, material: { ambient: 0.6, diffuse: 0.7, specular: 0, rim: 0 } })),
      ...towersC.map((n, c) => ({ mesh: cube, instances: n, material: { ambient: 0.62, diffuse: 0.6, specular: 0, rim: 0, texture: TX.windowsComic, textureRepeat: CLS[c] } })),
      { mesh: cube, instances: roofsC, material: { ambient: 0.62, diffuse: 0.6, specular: 0, rim: 0 } },
      { mesh: world.farMesh, instances: farAt, material: { ...baked, texture: TX.farComic } },
      { mesh: dome, instances: domeAt, cull: false, material: { ambient: 0, diffuse: 0, specular: 0, rim: 0, texture: TX.skyComic, textureMode: 'emissive', fog: 0 } },
      { mesh: water, instances: waterAt, mirror: true, material: { ...baked, ambient: 1 } },
    ],
  };
  // Neon lane pulses move forward with the flight.
  const laneItems = items.N.filter((it) => it.mesh && lanes.includes(it.mesh));
  const waterR = items.R.find((it) => it.mirror), waterN = items.N.find((it) => it.mirror), waterC = items.C.find((it) => it.mirror);
  waterAt.set(0, { sx: 40000, sz: 40000, y: WATER, color: [0.035, 0.07, 0.085] });
  const waterNAt = instances(1, () => ({ sx: 40000, sz: 40000, y: WATER, color: [0.004, 0.003, 0.01] }));
  const waterCAt = instances(1, () => ({ sx: 40000, sz: 40000, y: WATER, color: [0.2, 0.5, 0.95] }));
  waterN.instances = waterNAt; waterC.instances = waterCAt;
  void waterR;
  const farNAt = instances(1, () => ({ color: [0.02, 0.012, 0.035] }));
  items.N.find((it) => it.mesh === world.farMesh).instances = farNAt;
  const terrNAt = instances(1, () => ({ color: [0.02, 0.018, 0.035] }));
  items.N.find((it) => it.mesh === world.terrainMesh).instances = terrNAt;
  items.N.find((it) => it.mesh === world.wallMesh).instances = terrNAt;

  const opts = {
    R: {
      background: '#c9a888', fog: [60, 15000], fogColor: HAZE_DISPLAY, groundFog: { bottom: 2, top: 70, range: 2600 },
      key: SUN, keyColor: [1.35, 1.0, 0.7], fill: [0, 1, 0], fillColor: [0.22, 0.28, 0.4], rimColor: [1.1, 0.72, 0.45], ...ENV_R,
      mirror: { y: WATER, strength: 0.78 },
      motionBlur: { shutter: 0.36, max: 70 },
    },
    N: {
      background: '#040208', fog: [200, 16000], fogColor: [0.09, 0.02, 0.1],
      key: [0.3, 0.6, 0.5], keyColor: [0.08, 0.1, 0.22], fill: [0, 1, 0], fillColor: [0.02, 0.02, 0.06], rimColor: [0.35, 0.95, 1.4],
      sky: [0.012, 0.008, 0.035], horizon: [0.5, 0.09, 0.4], ground: [0.01, 0.005, 0.02], strips: [0.6, 0.5, 1.2],
      mirror: { y: WATER, strength: 0.8 },
      motionBlur: { shutter: 0.36, max: 70 },
    },
    C: {
      background: '#9fd4ff', fog: [500, 26000], fogColor: [0.68, 0.86, 1.0],
      key: SUN, keyColor: [1.05, 1.0, 0.92], fill: [0, 1, 0], fillColor: [0.35, 0.38, 0.5], rimColor: [1, 1, 1], sky: [0.3, 0.6, 1], horizon: [0.7, 0.88, 1], ground: [0.3, 0.3, 0.3], strips: [0, 0, 0],
      // No mirror pass: printed water is flat ink with wave strokes (and it saves the reflection render).
    },
  };

  const rA = renderer3d({ width: W, height: H }), rB = renderer3d({ width: W, height: H }), rC = renderer3d({ width: W, height: H });
  const N_PIX = W * H;
  const bufA = new Uint8ClampedArray(N_PIX * 4), bufB = new Uint8ClampedArray(N_PIX * 4), frame = new Uint8ClampedArray(N_PIX * 4);
  const comicOut = [new Uint8ClampedArray(N_PIX * 4), new Uint8ClampedArray(N_PIX * 4), new Uint8ClampedArray(N_PIX * 4)];
  const distCopy = [new Float32Array(N_PIX), new Float32Array(N_PIX), new Float32Array(N_PIX)];

  // Render one look through the current camera into `slot` (0, 1, 2): pixels + distances.
  let prevVP = null, curT = 0;
  const renderLook = (look, slot) => {
    const r = [rA, rB, rC][slot];
    const o = opts[look];
    const zc = cam.camera.z + 120;
    const its = items[look].filter((it) => it.zmin === undefined || it.zmin < zc);
    for (const it of laneItems) it.material.pulsePhase = curT * 4.2;
    const px = r.render(cam, its, { ...o, sort: 'near', prevCamera: prevVP, motionBlur: look === 'C' ? undefined : o.motionBlur });
    const dist = r.depth(cam);
    const dst = slot === 0 ? bufA : slot === 1 ? bufB : frame;
    if (look === 'C') { comic(px, dist, comicOut[slot], curT); distCopy[slot].set(dist); return { px: comicOut[slot], dist: distCopy[slot] }; }
    dst.set(px);
    distCopy[slot].set(dist);
    if (look === 'R') realWater(px, dst, distCopy[slot], curT);
    return { px: dst, dist: distCopy[slot] };
  };

  // ---- The comic pass: ink outlines from depth and colour edges, flat colour bands, halftone ----
  const ANG = new Uint16Array(N_PIX), RAD = new Float32Array(N_PIX);
  for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
    const dx = x - W / 2, dy = y - H * 0.47;
    ANG[y * W + x] = Math.floor(((Math.atan2(dy, dx) / (Math.PI * 2)) + 0.5) * 720) % 720;
    RAD[y * W + x] = Math.hypot(dx, dy) / (H * 0.5);
  }
  const lum = new Float32Array(N_PIX);
  // Printed-comic look: flat inks on off-white paper, Ben-Day dots in the mid tones, black
  // cross-hatching in the shadows, heavy ink outlines from depth, slight colour misregistration,
  // manga speed lines and a panel border.
  const PAPER = [246, 238, 214];
  // The 45° halftone screen and the hatching, precomputed once per pixel.
  // Hatching is engraved: line width grows with the shade (distance to the nearest line, in px,
  // of a slightly wavering 6 px screen in two directions); the paper has a faint fibre grain.
  const DOT = new Float32Array(N_PIX), HA = new Float32Array(N_PIX), HB = new Float32Array(N_PIX), GRAIN = new Float32Array(N_PIX);
  for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
    const u45 = (x + y) * 0.7071, v45 = (x - y) * 0.7071;
    const fu = u45 / 7 - Math.floor(u45 / 7) - 0.5, fv = v45 / 7 - Math.floor(v45 / 7) - 0.5;
    const i = y * W + x;
    DOT[i] = Math.hypot(fu, fv);
    const wa = 0.9 * Math.sin(y * 0.031 + x * 0.007) + 0.5 * noise(x / 40, y / 40);
    const a = (x * 0.94 + y * 0.34 + wa) / 6, b = (x * 0.34 - y * 0.94 + wa * 0.7 + 3000) / 6.5;
    HA[i] = Math.abs(a - Math.round(a)) * 6; HB[i] = Math.abs(b - Math.round(b)) * 6.5;
    GRAIN[i] = 1 - 0.035 * (0.5 + 0.5 * noise(x / 1.7 + 50, y / 9)) - 0.03 * (0.5 + 0.5 * noise(x / 60, y / 45));
  }
  // Water strokes: each pixel's ray is a linear function of its screen position (from the inverse
  // view-projection); where the ray meets the lake at the depth the buffer holds, the pixel is water,
  // and short inked wave dashes and white glints are drawn there, fixed in the world.
  const inv4 = (m) => {
    const a = m, o = new Array(16);
    const b00 = a[0] * a[5] - a[1] * a[4], b01 = a[0] * a[6] - a[2] * a[4], b02 = a[0] * a[7] - a[3] * a[4], b03 = a[1] * a[6] - a[2] * a[5], b04 = a[1] * a[7] - a[3] * a[5], b05 = a[2] * a[7] - a[3] * a[6];
    const b06 = a[8] * a[13] - a[9] * a[12], b07 = a[8] * a[14] - a[10] * a[12], b08 = a[8] * a[15] - a[11] * a[12], b09 = a[9] * a[14] - a[10] * a[13], b10 = a[9] * a[15] - a[11] * a[13], b11 = a[10] * a[15] - a[11] * a[14];
    const det = 1 / (b00 * b11 - b01 * b10 + b02 * b09 + b03 * b08 - b04 * b07 + b05 * b06);
    o[0] = (a[5] * b11 - a[6] * b10 + a[7] * b09) * det; o[1] = (a[2] * b10 - a[1] * b11 - a[3] * b09) * det; o[2] = (a[13] * b05 - a[14] * b04 + a[15] * b03) * det; o[3] = (a[10] * b04 - a[9] * b05 - a[11] * b03) * det;
    o[4] = (a[6] * b08 - a[4] * b11 - a[7] * b07) * det; o[5] = (a[0] * b11 - a[2] * b08 + a[3] * b07) * det; o[6] = (a[14] * b02 - a[12] * b05 - a[15] * b01) * det; o[7] = (a[8] * b05 - a[10] * b02 + a[11] * b01) * det;
    o[8] = (a[4] * b10 - a[5] * b08 + a[7] * b06) * det; o[9] = (a[1] * b08 - a[0] * b10 - a[3] * b06) * det; o[10] = (a[12] * b04 - a[13] * b02 + a[15] * b00) * det; o[11] = (a[9] * b02 - a[8] * b04 - a[11] * b00) * det;
    o[12] = (a[5] * b07 - a[4] * b09 - a[6] * b06) * det; o[13] = (a[0] * b09 - a[1] * b07 + a[2] * b06) * det; o[14] = (a[13] * b01 - a[12] * b03 - a[14] * b00) * det; o[15] = (a[8] * b03 - a[9] * b01 + a[10] * b00) * det;
    return o;
  };
  const WAVE = new Uint8Array(N_PIX); // 0 none, 1 ink dash, 2 glint
  const WSW = new Float32Array(N_PIX), WVZ = new Float32Array(N_PIX), WX = new Float32Array(N_PIX), WZ = new Float32Array(N_PIX), WDN = new Float32Array(N_PIX); // water: view depth (0 = not water), world x/z, how steeply we look down
  function waterStrokes(dist, mode = 'comic', t = 0) {
    WAVE.fill(0); WVZ.fill(0);
    const c = cam.camera, ex = c.x, ey = c.y - WATER, ez = c.z;
    if (ey < 0.2) return;
    const iv = inv4(cam.viewProjection().matrix);
    const tg = cam.target, fl = Math.hypot(tg.x - ex, tg.y - c.y, tg.z - ez), fx = (tg.x - ex) / fl, fy = (tg.y - c.y) / fl, fz = (tg.z - ez) / fl;
    // Homogeneous far-plane point p(nx, ny) = iv · [nx, ny, 1, 1]; the ray is p.xyz − eye · p.w (linear in nx, ny).
    const P = (nx, ny) => { const r = [0, 1, 2, 3].map((q) => iv[q] * nx + iv[4 + q] * ny + iv[8 + q] + iv[12 + q]); return [r[0] - ex * r[3], r[1] - c.y * r[3], r[2] - ez * r[3]]; };
    const o0 = P(0, 0), ax = P(1, 0).map((v, q) => v - o0[q]), ay = P(0, 1).map((v, q) => v - o0[q]);
    // Evaluated on 2×2 blocks (written to all four pixels): a quarter of the work.
    for (let y = 0; y < H; y += 2) {
      const ny = 1 - ((y + 1) / H) * 2;
      const ry = o0[1] + ay[1] * ny;
      if (ry - Math.abs(ax[1]) >= 0) continue; // the whole row looks above the horizon
      for (let x = 0; x < W; x += 2) {
        const nx = ((x + 1) / W) * 2 - 1;
        const dy = ry + ax[1] * nx;
        if (dy >= 0) continue;
        const dx = o0[0] + ax[0] * nx + ay[0] * ny, dz = o0[2] + ax[2] * nx + ay[2] * ny;
        const tt = -ey / dy, vz = tt * (dx * fx + dy * fy + dz * fz);
        const i = y * W + x, d = dist[i];
        if (vz > (mode === 'comic' ? 700 : 30000) || Math.abs(d - vz) > 0.03 * vz + 0.4) continue;
        const wx = ex + dx * tt, wz = ez + dz * tt, dn = -dy / Math.sqrt(dx * dx + dy * dy + dz * dz);
        let wv = 0;
        if (mode !== 'comic') wv = Math.sin(wz * 0.55 + wx * 0.12 + t * 1.6) * 0.6 + Math.sin(wz * 1.7 - wx * 0.4 + t * 2.3) * 0.3 + Math.sin(wz * 0.31 - wx * 0.27 - t * 1.1) * 0.4;
        if (mode === 'comic') {
          const u = wx / 7, v = wz / 2.4, cu = Math.floor(u), cv = Math.floor(v), hh = hash(cu * 7919 + cv * 104729 + 13);
          if (hh <= 0.42) {
            const fu = u - cu, fv = v - cv - 0.5 - 0.12 * Math.sin(fu * Math.PI);
            const len = 0.55 + 0.4 * hash(cu * 31 + cv * 17), du = (fu - 0.5) / (len * 0.5);
            if (Math.abs(du) <= 1) {
              const th = (hh < 0.3 ? 0.09 : 0.06) * (1 - du * du) * Math.min(1, 40 / vz + 0.35);
              if (Math.abs(fv) < th) wv = hh < 0.3 ? 1 : 2;
            }
          }
        }
        const wc = mode === 'comic' ? wv : 0;
        for (let q = 0; q < 4; q++) { const k = i + (q & 1) + (q >> 1) * W; if (k >= N_PIX) continue; WVZ[k] = vz; WX[k] = wx; WZ[k] = wz; WDN[k] = dn; WAVE[k] = wc; WSW[k] = wv; }
      }
    }
  }
  // Photographic water: a gentle swell bends the reflection (more up close), and looking down
  // the lake shows its own dark body instead of the mirror (Fresnel).
  function realWater(src, dst, dist, t) {
    waterStrokes(dist, 'real', t);
    const DEEP = [16, 30, 34];
    for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
      const i = y * W + x, vz = WVZ[i];
      if (vz === 0) continue;
      const sw = WSW[i];
      const amp = Math.min(9, 260 / (vz + 20));
      const sy = Math.min(H - 1, Math.max(0, Math.round(y + sw * amp))), sx = Math.min(W - 1, Math.max(0, Math.round(x + sw * amp * 0.25)));
      const so = (sy * W + sx) * 4, o = i * 4;
      const dn = WDN[i], fr = (0.12 + 0.85 * dn * Math.sqrt(dn)) * 0.75;
      dst[o] = mix(src[so], DEEP[0], fr); dst[o + 1] = mix(src[so + 1], DEEP[1], fr); dst[o + 2] = mix(src[so + 2], DEEP[2], fr);
    }
  }
  function comic(src, dist, out, t) {
    waterStrokes(dist);
    for (let i = 0; i < N_PIX; i++) { const o = i * 4; lum[i] = (src[o] * 0.299 + src[o + 1] * 0.587 + src[o + 2] * 0.114) / 255; }
    const B = 16;
    for (let y = 0; y < H; y++) {
      const yy = Math.min(H - 4, Math.max(3, y));
      for (let x = 0; x < W; x++) {
        const i = y * W + x, o = i * 4;
        const xx = Math.min(W - 4, Math.max(3, x)), c = yy * W + xx;
        const d = dist[c], finite = d < 1e30, sky = !finite || d > 9500; // the dome is 20 km away but its z-depth falls off-axis
        // Ink: silhouettes and creases from depth (thicker up close), strong colour edges.
        let ink = 0;
        const step = d < 150 ? 3 : d < 700 ? 2 : 1;
        const l = dist[c - step], r = dist[c + step], u = dist[c - step * W], dn = dist[c + step * W];
        const jump = (n) => (finite !== (n < 1e30) ? 1 : finite ? Math.abs(d - n) / Math.min(d, n) : 0);
        const jm = Math.max(jump(l), jump(r), jump(u), jump(dn));
        if (jm > 0.05 && d < 15000) ink = 1;
        // Heavier contour on silhouettes (big depth jumps): look one step further out.
        else if (d < 2500 && Math.max(jump(dist[c - 2 * step]), jump(dist[c + 2 * step]), jump(dist[c - 2 * step * W]), jump(dist[c + 2 * step * W])) > 0.35) ink = 1;
        if (finite && d < 3000 && (Math.abs(l + r - 2 * d) + Math.abs(u + dn - 2 * d)) / d > 0.07 / step) ink = 1;
        const g = Math.abs(lum[c + 2] - lum[c - 2]) + Math.abs(lum[c + 2 * W] - lum[c - 2 * W]);
        if (g > 0.34) ink = 1;
        // Flat inks: saturate, then three bands.
        const L = lum[i];
        let R = src[o], Gc = src[o + 1], Bc = src[o + 2];
        const m = (R + Gc + Bc) / 3;
        R = m + (R - m) * 1.7; Gc = m + (Gc - m) * 1.7; Bc = m + (Bc - m) * 1.7;
        const k = 1 / (L + 0.05);
        R *= k * 0.62; Gc *= k * 0.62; Bc *= k * 0.62; // normalise to a flat ink of the hue
        const dot = DOT[i];
        let cr, cg, cb;
        if (sky) {
          // Sky: Ben-Day dots, bigger towards the zenith; clouds stay paper.
          const rad = 0.1 + 0.36 * clamp((0.97 - L) * 1.5) * (0.55 + 0.45 * (1 - y / H));
          const on = clamp((rad - dot) * 14 + 0.5);
          cr = mix(PAPER[0], R * 0.8, on); cg = mix(PAPER[1], Gc * 0.85, on); cb = mix(PAPER[2], Math.min(255, Bc * 1.05), on);
          if (L > 0.93) { cr = PAPER[0]; cg = PAPER[1]; cb = PAPER[2]; }
        } else if (L > 0.58) {
          cr = mix(PAPER[0], R, 0.75); cg = mix(PAPER[1], Gc, 0.75); cb = mix(PAPER[2], Bc, 0.75);
        } else if (L > 0.26) {
          const on = dot < 0.36;
          cr = on ? R : mix(PAPER[0], R, 0.55); cg = on ? Gc : mix(PAPER[1], Gc, 0.55); cb = on ? Bc : mix(PAPER[2], Bc, 0.55);
        } else if (L > 0.07) {
          cr = R * 0.66; cg = Gc * 0.66; cb = Bc * 0.74;
          // Engraved hatching: lines thicken into the shadow; a second direction crosses the darkest.
          const wA = 0.35 + 2.4 * clamp((0.27 - L) / 0.17), wB = 2.2 * clamp((0.17 - L) / 0.1);
          const hk = Math.max(clamp(wA - HA[i] + 0.5), clamp(wB - HB[i] + 0.5));
          cr = mix(cr, 22, hk); cg = mix(cg, 18, hk); cb = mix(cb, 30, hk);
        } else {
          cr = 20; cg = 17; cb = 27; // spotted blacks
        }
        // Misregistration: the red plate sits 2 px to the right.
        const or = (y * W + Math.max(B, x - 2)) * 4;
        if (!sky && L > 0.26) cr = mix(cr, src[or] * k * 0.62, 0.25);
        // Speed lines towards the edges (black, manga).
        const rad = RAD[i];
        if (rad > 0.66) {
          const a = ANG[i], hsh = hash(a * 97 + Math.floor(t * 9 + hash(a * 13 + 1) * 9) * 7919);
          if (hsh > 0.955 && hash(a * 31 + 3) * 0.4 + 0.8 < rad) { const w = smooth(0.8, 1.15, rad) * 0.85; cr = mix(cr, 16, w); cg = mix(cg, 14, w); cb = mix(cb, 22, w); }
        }
        const wv = WAVE[i];
        if (wv === 1) { cr = 14; cg = 26; cb = 70; } else if (wv === 2) { cr = PAPER[0]; cg = PAPER[1]; cb = PAPER[2]; }
        if (ink) { cr = 16; cg = 13; cb = 20; }
        const gr = GRAIN[i]; cr *= gr; cg *= gr; cb *= gr;
        out[o] = cr; out[o + 1] = cg; out[o + 2] = cb; out[o + 3] = 255;
      }
    }
  }

  // The comic panel border (paper gutter, then a black rule), faded in with the comic look.
  function panelBorder(buf, a) {
    if (a <= 0.002) return;
    const G = 9, Bw = 16;
    const paint = (x0, y0, x1, y1, c) => {
      for (let y = y0; y < y1; y++) for (let x = x0; x < x1; x++) {
        const o = (y * W + x) * 4;
        buf[o] += (c[0] - buf[o]) * a; buf[o + 1] += (c[1] - buf[o + 1]) * a; buf[o + 2] += (c[2] - buf[o + 2]) * a;
      }
    };
    const INK = [14, 12, 18];
    paint(0, 0, W, G, PAPER); paint(0, H - G, W, H, PAPER); paint(0, G, G, H - G, PAPER); paint(W - G, G, W, H - G, PAPER);
    paint(G, G, W - G, Bw, INK); paint(G, H - Bw, W - G, H - G, INK); paint(G, Bw, Bw, H - Bw, INK); paint(W - Bw, Bw, W - G, H - Bw, INK);
  }

  // ---- Transitions ------------------------------------------------------------------------------
  const lnF = Math.log(9000), lnN = Math.log(1.5);
  function composite(a, b, type, p, t) {
    const pa = a.px, pb = b.px, da = a.dist;
    const e = ease(clamp(p));
    if (type === 'front' || type === 'reveal') {
      // A front at distance D sweeps through the world (far → near, or near → far).
      // High above water nothing is nearer than the camera's height: start the front there.
      const lnNear = Math.max(lnN, Math.log(Math.max(1.5, (cam.camera.y - WATER) * 0.8)));
      const D = Math.exp(type === 'front' ? mix(lnF, lnNear, e) : mix(lnNear, lnF, e));
      // The sky (and anything beyond 8 km) cross-fades instead: first for a front, last for a reveal.
      const kSky = type === 'front' ? smooth(0.0, 0.35, e) : smooth(0.6, 0.95, e);
      const soft = D * (type === 'reveal' ? 0.45 : 0.12) + 1.5, band = D * 0.12 + 2;
      const sc = type === 'front' ? [0.5, 1.0, 1.0] : [1.0, 0.75, 0.4];
      for (let i = 0; i < N_PIX; i++) {
        const d = da[i], o = i * 4, sky = d > 8000;
        const k = sky ? kSky : type === 'front' ? smooth(D - soft, D + soft, d) : 1 - smooth(D - soft, D + soft, d);
        const g = sky ? 0 : Math.exp(-(((d - D) / band) ** 2)) * (1 - smooth(0.7, 1, e)) * smooth(0, 0.15, e) * 255 * 0.16;
        frame[o] = pa[o] + (pb[o] - pa[o]) * k + g * sc[0];
        frame[o + 1] = pa[o + 1] + (pb[o + 1] - pa[o + 1]) * k + g * sc[1];
        frame[o + 2] = pa[o + 2] + (pb[o + 2] - pa[o + 2]) * k + g * sc[2];
        frame[o + 3] = 255;
      }
    } else if (type === 'dots') {
      const cell = 26;
      for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
        const cx = (Math.floor(x / cell) + 0.5) * cell, cy = (Math.floor(y / cell) + 0.5) * cell;
        const rc = Math.hypot(cx - W / 2, cy - H / 2) / (W * 0.55);
        const r = cell * 0.74 * clamp(e * 1.9 - rc * 0.9);
        const dd = Math.hypot(x - cx, y - cy);
        const i = y * W + x, o = i * 4;
        const k = clamp(r - dd + 0.5), ring = r > 1 ? clamp(1 - Math.abs(dd - r) / 1.6) * (1 - e) * 0.5 : 0;
        for (let q = 0; q < 3; q++) frame[o + q] = mix(mix(pa[o + q], pb[o + q], k), 16, ring);
        frame[o + 3] = 255;
      }
    } else if (type === 'slash') {
      const nx = 0.82, ny = 0.57, span = W * nx + H * ny;
      const edge = mix(-0.08, 1.08, e) * span;
      for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
        const u = x * nx + y * ny, s = (edge - u);
        const i = y * W + x, o = i * 4;
        // A clean, soft edge with a thin warm line of daylight along it.
        const k = clamp(s / 90 + 0.5);
        const hot = Math.exp(-((s / 6) ** 2)) * 255 * 0.22 * (1 - smooth(0.8, 1, e));
        const ob = o;
        frame[o] = mix(pa[o], pb[ob], k) + hot;
        frame[o + 1] = mix(pa[o + 1], pb[ob + 1], k) + hot * 0.92;
        frame[o + 2] = mix(pa[o + 2], pb[ob + 2], k) + hot * 0.8;
        frame[o + 3] = 255;
      }
    } else if (type === 'ink') {
      const R = mix(-0.15, 1.35, e);
      for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
        const dx = (x - W / 2) / (H * 0.5), dy = (y - H * 0.5) / (H * 0.5);
        const ang = Math.atan2(dy, dx);
        const r = Math.hypot(dx, dy) * 0.62 + 0.16 * noise(Math.cos(ang) * 2.2 + 7, Math.sin(ang) * 2.2) + 0.06 * noise(x / 38, y / 38 + 3);
        const blot = noise(x / 22 + 11, y / 22) > 0.42 - 0.5 * (R - r) ? 1 : 0; // droplets ahead of the front
        const i = y * W + x, o = i * 4;
        const k = r < R ? 1 : blot && r < R + 0.12 ? 1 : 0;
        const rim = clamp(1.6 - Math.abs(r - R) / 0.03) * (R > 0 && R < 1.25 ? 1 : 0);
        for (let q = 0; q < 3; q++) frame[o + q] = mix(k ? pb[o + q] : pa[o + q], 14, rim);
        frame[o + 3] = 255;
      }
    } else if (type === 'glitch') {
      let y = 0, band = 0;
      while (y < H) {
        const hgt = 6 + Math.floor(hash(band * 17 + 1) * 54), u = hash(band * 29 + 5) * 0.86 + 0.07;
        const on = e > u, near = Math.abs(e - u) < 0.09;
        const shift = 0;
        for (let yy = y; yy < Math.min(H, y + hgt); yy++) for (let x = 0; x < W; x++) {
          const o = (yy * W + x) * 4, src = on ? pb : pa;
          const xs = Math.min(W - 1, Math.max(0, x + shift)), os = (yy * W + xs) * 4;
          const sp = 0;
          frame[o] = src[(yy * W + Math.min(W - 1, xs + sp)) * 4];
          frame[o + 1] = src[os + 1];
          frame[o + 2] = src[(yy * W + Math.max(0, xs - sp)) * 4 + 2];
          
          frame[o + 3] = 255;
        }
        y += hgt; band++;
      }
    }
  }
  // The hook: three panels (neon | photographic | comic) slide in during the dive and out again.
  function triptych(nP, rP, cP, inn, out) {
    const x1 = W * mix(0, 1 / 3, inn) - W * 0.34 * out, x2 = W * mix(1, 2 / 3, inn) + W * 0.34 * out;
    for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
      const o = (y * W + x) * 4;
      // Slightly slanted seams.
      const s1 = x - (x1 + (y - H / 2) * 0.08), s2 = x - (x2 + (y - H / 2) * 0.08);
      const src = s1 < 0 ? nP : s2 > 0 ? cP : rP;
      const seam = Math.max(Math.exp(-((s1 / 2.2) ** 2)), Math.exp(-((s2 / 2.2) ** 2))) * (x1 > 2 || x2 < W - 2 ? 255 : 0);
      frame[o] = src[o] + seam; frame[o + 1] = src[o + 1] + seam; frame[o + 2] = src[o + 2] + seam; frame[o + 3] = 255;
    }
  }

  // ---- Where the sun is on screen (for rays and flares) -----------------------------------------
  const sunScreen = (t) => {
    pose(t);
    const m = cam.viewProjection().matrix, p = [cam.camera.x + SUN[0] * 1e4, cam.camera.y + SUN[1] * 1e4, cam.camera.z + SUN[2] * 1e4];
    const w = m[3] * p[0] + m[7] * p[1] + m[11] * p[2] + m[15];
    if (w <= 0) return { x: 0.5, y: -1, vis: 0 };
    const x = (m[0] * p[0] + m[4] * p[1] + m[8] * p[2] + m[12]) / w, y = (m[1] * p[0] + m[5] * p[1] + m[9] * p[2] + m[13]) / w;
    const sx = 0.5 + x * 0.5, sy = 0.5 - y * 0.5;
    const inside = (1 - smooth(0.42, 0.62, Math.abs(sx - 0.5))) * (1 - smooth(0.42, 0.62, Math.abs(sy - 0.5)));
    // Hidden behind terrain? Compare the ground along the sun ray a little ahead.
    let occl = 0;
    for (let d = 40; d < 2200; d *= 1.35) { const gx = cam.camera.x + SUN[0] * d, gz = cam.camera.z + SUN[2] * d; if (groundAt(gx, gz) + 12 > cam.camera.y + SUN[1] * d) { occl = 1; break; } }
    return { x: sx, y: sy, vis: inside * (1 - occl) };
  };
  const sunMemo = new Map();
  const sunAtT = (t) => { const k = Math.round(t * FPS); if (!sunMemo.has(k)) sunMemo.set(k, sunScreen(k / FPS)); return sunMemo.get(k); };

  // ---- The grade (blended by how much of each look is on screen) ---------------------------------
  const W_ = (t, r, n, c) => { const w = weights(t); return w.R * r + w.N * n + w.C * c; };
  const filmC = composition({
    width: W, height: H, fps: FPS, duration: DUR, motionBlur: 1,
    post: {
      aa: 1,
      bloom: (t) => W_(t, 0.28, 0.78, 0.0),
      bloomThreshold: (t) => W_(t, 0.82, 0.6, 1),
      bloomRadius: 42,
      streaks: (t) => W_(t, 0, 0.3, 0),
      streakLength: 260,
      rays: (t) => W_(t, 0.5 * sunAtT(t).vis, 0, 0),
      raysCenter: (t) => { const s = sunAtT(t); return [s.x, s.y]; },
      raysLength: 0.55,
      flares: (t) => W_(t, 0.55 * sunAtT(t).vis, 0.12, 0),
      aberration: (t) => W_(t, 0.25, 0.7, 0),
      vignette: (t) => W_(t, 0.34, 0.44, 0.12),
      grain: (t) => W_(t, 2.8, 2.6, 0),
      contrast: (t) => W_(t, 1.1, 1.08, 1.0),
      exposure: (t) => W_(t, 1.12, 1, 1),
      saturation: (t) => W_(t, 1.14, 1.1, 1.0),
      shadows: [0.0, 0.006, 0.018], highlights: [0.025, 0.01, -0.012],
    },
  });

  filmC.scene('flight', 0, DUR, {
    draw({ t, pixels }) {
      curT = t;
      pose(Math.max(0, t - 1 / FPS));
      prevVP = cam.viewProjection().matrix;
      pose(t);
      const c = cam.camera;
      domeAt.set(0, { x: c.x, y: WATER, z: c.z, scale: 20000 });
      sunAt.set(0, { x: c.x + SUN[0] * 17000, y: c.y + SUN[1] * 17000, z: c.z + SUN[2] * 17000, scale: 190, color: [1, 0.86, 0.62] });
      const s = state(t);
      let out;
      if (s.tease !== undefined) {
        const mid = s.mid ?? 'R', right = s.right ?? 'C';
        const n = renderLook('N', 0), rp = renderLook(right, 1), m = renderLook(mid, 2);
        triptych(n.px, m.px, rp.px, s.inn, s.out);
        out = frame;
      } else if (s.look) {
        out = renderLook(s.look, 2).px;
      } else {
        const a = renderLook(s.from, 0), b = renderLook(s.to, 1);
        composite(a, b, s.type, s.p, t);
        out = frame;
      }
      const aC = s.look ? (s.look === 'C' ? 1 : 0) : s.tease !== undefined ? 0 : s.to === 'C' ? ease(clamp((s.p - 0.6) / 0.4)) : s.from === 'C' ? 1 - ease(clamp(s.p / 0.4)) : 0;
      panelBorder(out, aC);
      pixels(out); // straight to the grade and the encoder (no canvas round trip)
    },
  });
  return filmC;
}
