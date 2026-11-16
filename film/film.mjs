// ANIKIT — ONE WORLD, THREE LOOKS. A single continuous flight through one valley at golden
// hour that keeps changing how it is drawn: photographic, neon, comic. Every frame is
// rendered by anikit (WebAssembly rasteriser, no WebGL); the looks are materials, lights and
// a per-pixel ink pass over the same geometry, and the transitions are driven by the depth
// buffer, so they sweep through the world rather than across the screen.
//
//   anikit-render film.mjs --still 6 --out still.jpg          (the first run bakes the world: ~1 min)
//   anikit-render film.mjs --review review/
//   anikit-render film.mjs --out anikit-film.mp4 --workers 3 --fade-out 0.5
import { buildWorld, SUN, WATER, HAZE_DISPLAY, clamp, mix, smooth, hash, noise } from './world.mjs';

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
  const pose = (t) => path.apply(cam, sOf(t), 0, { shake: 2.2 + 3 * smooth(0, 0.3, t) * (1 - smooth(1.0, 1.6, t)), time: t, rate: 2.4 });
  Object.assign(debug, { pose, cam, DUR, FPS });

  // ---- Looks and transitions: a schedule anchored to places on the route --------------------
  // R = photographic, N = neon, C = comic.
  const tr = (c, dur, from, to, type) => ({ t0: c - dur / 2, t1: c + dur / 2, from, to, type });
  const SCHEDULE = [
    tr(3.8, 0.8, 'R', 'C', 'front'),     // over the lake: the comic world sweeps in from the horizon
    tr(6.6, 0.5, 'C', 'N', 'glitch'),    // over the saddle, a scan glitch into the neon city
    tr(10.0, 0.6, 'N', 'C', 'ink'),      // into the canyon: an ink burst
    tr(12.8, 0.6, 'C', 'N', 'dots'),     // halftone dots dissolve into a neon river
    tr(15.4, 0.45, 'N', 'R', 'slash'),   // out of the canyon: a slash of daylight
    tr(18.2, 0.9, 'R', 'C', 'reveal'),   // over the last ridge a comic world grows out from the lens
  ];
  const TEASE = [0.34, 1.12]; // the hook: during the dive the frame splits into all three looks
  const OUTRO = DUR - 1.35;
  const ease = (p) => p * p * (3 - 2 * p);
  const state = (t) => {
    if (t > TEASE[0] && t < TEASE[1]) { const p = (t - TEASE[0]) / (TEASE[1] - TEASE[0]); return { tease: p, inn: ease(clamp(p / 0.28)), out: ease(clamp((p - 0.7) / 0.3)) }; }
    if (t > OUTRO) return { tease: 0.5, inn: ease(clamp((t - OUTRO) / 0.5)), out: 0 }; // the bookend: all three again
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
    if (s.tease !== undefined) { const k = s.inn * (1 - s.out); w.R = 1 - 0.66 * k; w.N = w.C = 0.33 * k; }
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
  const tm = world.treeMesh;
  const treeSet = (col) => instances(trees.length, (i) => {
    const t = trees[i], c = col(t);
    return { x: t.x, y: t.y, z: t.z, sx: t.h * t.w, sy: t.h, sz: t.h * t.w, ry: t.ry, color: c };
  });
  const treesR = treeSet((t) => { const v = 0.5 + 0.5 * world.visAt(t.x, t.z), k = 0.8 + 0.45 * t.tint; return [0.11 * v * k, 0.16 * v * (0.9 + 0.2 * t.tint), 0.075 * v]; });
  const treesN = treeSet((t) => (t.tint > 0.5 ? [0.0, 0.035, 0.05] : [0.04, 0.0, 0.045]));
  const treesC = treeSet((t) => (t.tint > 0.75 ? [0.2, 0.62, 0.28] : t.tint > 0.35 ? [0.12, 0.5, 0.3] : [0.1, 0.42, 0.26]));
  // Buildings in three height classes (so the facade texture keeps its window size).
  const CLS = [[0.82, 1.45], [0.82, 0.8], [0.82, 0.32]];
  const PAL = [[0.95, 0.36, 0.3], [0.98, 0.8, 0.3], [0.32, 0.7, 0.86], [0.92, 0.92, 0.88], [0.55, 0.45, 0.85]];
  const towerSet = (cls, col) => {
    const list = towers.filter((b) => b.cls === cls);
    return instances(list.length, (i) => { const b = list[i]; return { x: b.x, y: b.y, z: b.z, sx: b.w, sy: b.h, sz: b.d, color: col(b, i) }; });
  };
  const towersR = [0, 1, 2].map((c) => towerSet(c, (b, i) => { const v = 0.82 + 0.3 * hash(i * 7 + c); return [v, v * 0.97, v * 0.92]; }));
  const towersN = [0, 1, 2].map((c) => towerSet(c, () => [0.025, 0.025, 0.04]));
  const towersC = [0, 1, 2].map((c) => towerSet(c, (b, i) => PAL[Math.floor(hash(i * 13 + c * 5) * PAL.length)]));

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
      { mesh: dome, instances: domeAt, cull: false, reflect: true, material: { ambient: 0, diffuse: 0, specular: 0, rim: 0, texture: TX.sky, textureMode: 'emissive', fog: 0 } },
      { mesh: world.farMesh, instances: farAt, material: { ...baked, texture: TX.farReal } },
      { mesh: world.terrainMesh, instances: terrainAt, material: { ...baked, texture: TX.real, textureFilter: 'aniso' } },
      { mesh: tm, instances: treesR, material: { ambient: 0.3, diffuse: 1.0, specular: 0.08, shininess: 6, rim: 0.45 } },
      ...towersR.map((n, c) => ({ mesh: cube, instances: n, material: { ambient: 0.42, diffuse: 0.9, specular: 0.7, shininess: 60, rim: 0.12, texture: TX.windows, textureRepeat: CLS[c] } })),
      { mesh: water, instances: waterAt, mirror: true, material: { ...baked, ambient: 0.25 } },
      { mesh: sunBall, instances: sunAt, blend: 'additive', material: { ambient: 0, diffuse: 0, specular: 0, rim: 0, emissive: 2.6, fog: 0 } },
      { points: motes, colors: moteCol, size: 0.05, intensity: 0.9, streak: 1, fog: [6, 90] },
    ],
    N: [
      { mesh: world.farMesh, instances: farAt, material: { ambient: 1, diffuse: 0, specular: 0, rim: 0, texture: TX.farNeon, textureMode: 'emissive', textureGain: 1.3 } },
      { mesh: world.terrainMesh, instances: terrainAt, material: { ambient: 1, diffuse: 0.9, specular: 0, rim: 0, texture: TX.neon, textureMode: 'emissive', textureGain: 1.25, textureFilter: 'aniso' } },
      { mesh: dome, instances: domeAt, cull: false, material: { ambient: 0, diffuse: 0, specular: 0, rim: 0, texture: TX.skyNeon, textureMode: 'emissive', fog: 0 } },
      ...towersN.map((n, c) => ({ mesh: cube, instances: n, material: { ambient: 1, diffuse: 0.6, specular: 0.9, shininess: 80, rim: 0.3, texture: TX.windowsNeon, textureMode: 'emissive', textureGain: 1.5, textureRepeat: CLS[c], lines: 1.1, lineColor: [1.5, 0.35, 1.1], grid: [1, 1], lineWidth: 0.012 } })),
      { mesh: water, instances: waterAt, mirror: true, material: { ambient: 1, diffuse: 0, specular: 0, rim: 0 } },
      ...lanes.map((m, k) => ({ mesh: m, blend: 'additive', material: { ambient: 0, diffuse: 0, specular: 0, rim: 0, emissive: 0.12, pulse: 3.2, pulseCount: k === 0 ? 34 : 48, pulsePhase: 0, lineColor: laneCol[k] } })),
      { mesh: tm, instances: treesN, blend: 'additive', material: { ambient: 1, diffuse: 0, specular: 0, rim: 1.1 } },
      { points: motes, colors: moteColN, size: 0.09, intensity: 1.8, streak: 1, fog: [6, 120] },
    ],
    C: [
      { mesh: dome, instances: domeAt, cull: false, material: { ambient: 0, diffuse: 0, specular: 0, rim: 0, texture: TX.skyComic, textureMode: 'emissive', fog: 0 } },
      { mesh: world.farMesh, instances: farAt, material: { ...baked, texture: TX.farComic } },
      { mesh: world.terrainMesh, instances: terrainAt, material: { ...baked, texture: TX.comic, textureFilter: 'aniso' } },
      { mesh: tm, instances: treesC, material: { ambient: 0.6, diffuse: 0.7, specular: 0, rim: 0 } },
      ...towersC.map((n, c) => ({ mesh: cube, instances: n, material: { ambient: 0.62, diffuse: 0.6, specular: 0, rim: 0, texture: TX.windowsComic, textureRepeat: CLS[c] } })),
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
  items.N[0].instances = farNAt;
  const terrNAt = instances(1, () => ({ color: [0.02, 0.018, 0.035] }));
  items.N[1].instances = terrNAt;

  const opts = {
    R: {
      background: '#c9a888', fog: [120, 21000], fogColor: HAZE_DISPLAY,
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
      mirror: { y: WATER, strength: 0.42 },
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
    const its = items[look];
    for (const it of laneItems) it.material.pulsePhase = curT * 4.2;
    const px = r.render(cam, its, { ...o, prevCamera: prevVP, motionBlur: look === 'C' ? undefined : o.motionBlur });
    const dist = r.depth(cam);
    const dst = slot === 0 ? bufA : slot === 1 ? bufB : frame;
    if (look === 'C') { comic(px, dist, comicOut[slot], curT); distCopy[slot].set(dist); return { px: comicOut[slot], dist: distCopy[slot] }; }
    dst.set(px);
    distCopy[slot].set(dist);
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
  function comic(src, dist, out, t) {
    const seed = Math.floor(t * 14);
    for (let i = 0; i < N_PIX; i++) { const o = i * 4; lum[i] = (src[o] * 0.299 + src[o + 1] * 0.587 + src[o + 2] * 0.114) / 255; }
    const speed = 0.55 + 0.45 * smooth(60, 120, 100);
    for (let y = 0; y < H; y++) {
      const yy = Math.min(H - 3, Math.max(2, y));
      for (let x = 0; x < W; x++) {
        const i = y * W + x, o = i * 4;
        const xx = Math.min(W - 3, Math.max(2, x)), c = yy * W + xx;
        const d = dist[c];
        // Ink: silhouettes (depth jumps) and strong colour edges.
        let ink = 0;
        const step = d < 400 ? 2 : 1;
        const l = dist[c - step], r = dist[c + step], u = dist[c - step * W], dn = dist[c + step * W];
        const finite = d < 1e30;
        const jump = (n) => (finite !== (n < 1e30) ? 1 : finite ? Math.abs(d - n) / Math.min(d, n) : 0);
        const j = Math.max(jump(l), jump(r), jump(u), jump(dn));
        if (j > 0.07 && d < 15000) ink = 1;
        if (finite && d < 2500) {
          const crease = Math.abs(l + r - 2 * d) / d + Math.abs(u + dn - 2 * d) / d;
          if (crease > 0.09 / step) ink = Math.max(ink, 0.8);
        }
        const g = Math.abs(lum[c + 1] - lum[c - 1]) + Math.abs(lum[c + W] - lum[c - W]);
        if (g > 0.42) ink = 1;
        // Flat bands: quantise luminance, keep (and push) the hue.
        const L = lum[i] + 1e-4;
        let q = L < 0.16 ? 0.1 : L < 0.4 ? 0.3 : L < 0.68 ? 0.6 : 0.9;
        let k = q / L;
        k = k > 3 ? 3 : k;
        let R = src[o] * k, G = src[o + 1] * k, B = src[o + 2] * k;
        const m = (R + G + B) / 3;
        R = m + (R - m) * 1.35; G = m + (G - m) * 1.35; B = m + (B - m) * 1.35;
        // Halftone: dots in the shade band (45° screen), lighter dots in the sky.
        const u45 = (x + y) * 0.7071, v45 = (x - y) * 0.7071, cell = 7;
        const fu = (u45 / cell) % 1, fv = (v45 / cell) % 1;
        const dd = Math.hypot((fu < 0 ? fu + 1 : fu) - 0.5, (fv < 0 ? fv + 1 : fv) - 0.5);
        if (q <= 0.3 && dd < 0.3 + (0.3 - q) * 0.5) { R *= 0.45; G *= 0.42; B *= 0.55; }
        else if (!finite || d > 15000) { const sky = 0.16 + 0.22 * (1 - y / H); if (dd < sky) { R = R * 0.86 + 6; G = G * 0.9 + 10; B = B * 0.96 + 14; } }
        // Speed lines towards the edges.
        const rad = RAD[i];
        if (rad > 0.62) {
          const a = ANG[i], hsh = hash(a * 97 + seed * 7919);
          if (hsh > 0.93 && hash(a * 31 + 3) * 0.5 + 0.7 < rad) { const w = smooth(0.7, 1.15, rad) * speed * 0.8; R = mix(R, 255, w); G = mix(G, 255, w); B = mix(B, 255, w); }
        }
        if (ink > 0) { R = mix(R, 18, ink); G = mix(G, 14, ink); B = mix(B, 26, ink); }
        out[o] = R; out[o + 1] = G; out[o + 2] = B; out[o + 3] = 255;
      }
    }
  }

  // ---- Transitions ------------------------------------------------------------------------------
  const lnF = Math.log(40000), lnN = Math.log(1.5);
  function composite(a, b, type, p, t) {
    const pa = a.px, pb = b.px, da = a.dist;
    const e = ease(clamp(p));
    if (type === 'front' || type === 'reveal') {
      // A front at distance D sweeps through the world (far → near, or near → far).
      const D = Math.exp(type === 'front' ? mix(lnF, lnN, e) : mix(lnN, lnF, e));
      const soft = D * 0.035 + 0.6, band = D * 0.09 + 1.5;
      const sc = type === 'front' ? [0.5, 1.0, 1.0] : [1.0, 0.75, 0.4];
      for (let i = 0; i < N_PIX; i++) {
        const d = da[i] > 1e30 ? 1e9 : da[i], o = i * 4;
        const k = type === 'front' ? smooth(D - soft, D + soft, d) : 1 - smooth(D - soft, D + soft, d);
        const g = Math.exp(-(((d - D) / band) ** 2)) * (1 - smooth(0.85, 1, e)) * 255 * 0.9;
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
        const k = clamp(r - dd + 0.5), ring = r > 1 ? clamp(1 - Math.abs(dd - r) / 1.6) * (1 - e) : 0;
        for (let q = 0; q < 3; q++) frame[o + q] = mix(mix(pa[o + q], pb[o + q], k), 16, ring);
        frame[o + 3] = 255;
      }
    } else if (type === 'slash') {
      const nx = 0.82, ny = 0.57, span = W * nx + H * ny;
      const edge = mix(-0.08, 1.08, e) * span;
      for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
        const u = x * nx + y * ny, s = (edge - u);
        const i = y * W + x, o = i * 4;
        const k = clamp(s / 3 + 0.5);
        const hot = Math.exp(-((s / 14) ** 2)) * 255 * 1.1 + Math.exp(-((s / 90) ** 2)) * 80;
        // The image just behind the edge is dragged along it (a smear).
        const sm = s > 0 && s < 160 ? Math.round((1 - s / 160) * 60) : 0;
        const ob = (y * W + Math.max(0, x - sm)) * 4;
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
        const rim = clamp(1 - Math.abs(r - R) / 0.022) * (R > 0 && R < 1.25 ? 1 : 0);
        for (let q = 0; q < 3; q++) frame[o + q] = mix(k ? pb[o + q] : pa[o + q], 14, rim);
        frame[o + 3] = 255;
      }
    } else if (type === 'glitch') {
      let y = 0, band = 0;
      while (y < H) {
        const hgt = 6 + Math.floor(hash(band * 17 + 1) * 54), u = hash(band * 29 + 5) * 0.86 + 0.07;
        const on = e > u, near = Math.abs(e - u) < 0.09;
        const shift = near ? Math.round((hash(band * 3 + Math.floor(t * 30)) - 0.5) * 70) : 0;
        for (let yy = y; yy < Math.min(H, y + hgt); yy++) for (let x = 0; x < W; x++) {
          const o = (yy * W + x) * 4, src = on ? pb : pa;
          const xs = Math.min(W - 1, Math.max(0, x + shift)), os = (yy * W + xs) * 4;
          const sp = near ? 10 : 0;
          frame[o] = src[(yy * W + Math.min(W - 1, xs + sp)) * 4];
          frame[o + 1] = src[os + 1];
          frame[o + 2] = src[(yy * W + Math.max(0, xs - sp)) * 4 + 2];
          if (near && (yy & 1)) { frame[o] += 30; frame[o + 1] += 30; frame[o + 2] += 40; }
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
      bloom: (t) => W_(t, 0.28, 0.95, 0.0),
      bloomThreshold: (t) => W_(t, 0.82, 0.42, 1),
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
    draw({ ctx, t }) {
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
        const n = renderLook('N', 0), cc = renderLook('C', 1), r = renderLook('R', 2);
        triptych(n.px, r.px, cc.px, s.inn, s.out);
        out = frame;
      } else if (s.look) {
        out = renderLook(s.look, 2).px;
      } else {
        const a = renderLook(s.from, 0), b = renderLook(s.to, 1);
        composite(a, b, s.type, s.p, t);
        out = frame;
      }
      ctx.putImageData(new globalThis.ImageData(out, W, H), 0, 0);
    },
  });
  // A soft punch on the hard cuts of look (the slash and the glitch).
  filmC.impact(SCHEDULE[2].t0 + 0.2, { shake: 6, flash: '#fff4e0', flashOpacity: 0.18, decay: 9 });
  filmC.impact(SCHEDULE[4].t0 + 0.25, { shake: 4, aberration: 5, decay: 10 });
  return filmC;
}
