// The world of the film: one valley flown through in three looks. Terrain, the
// textures baked for each look (sunlit photographic, neon contour lines, comic
// cel), forests, a small city and the skies. Everything is procedural and
// deterministic; the expensive bakes are cached in film/.cache.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const CACHE = path.join(HERE, '.cache');
const VERSION = 'w23';
const WALL_NY = 0.6;                                // cells steeper than this (normal y) are cliffs
const WT = 4096, WY0 = -12, WY1 = 470;               // the wall atlas: 4 strips (one per facing), y from WY1 down to WY0

export const clamp = (v, a = 0, b = 1) => (v < a ? a : v > b ? b : v);
export const mix = (a, b, t) => a + (b - a) * t;
export const smooth = (a, b, x) => { const t = clamp((x - a) / (b - a)); return t * t * (3 - 2 * t); };
const norm = (v) => { const l = Math.hypot(...v); return v.map((x) => x / l); };

// ---- Noise (gradient noise, deterministic) -----------------------------------------------
function makeNoise(seed) {
  const perm = new Uint8Array(512), p = [...Array(256).keys()];
  let s = seed >>> 0;
  const rnd = () => ((s = (s * 1664525 + 1013904223) >>> 0) / 4294967296);
  for (let i = 255; i > 0; i--) { const j = Math.floor(rnd() * (i + 1)); [p[i], p[j]] = [p[j], p[i]]; }
  for (let i = 0; i < 512; i++) perm[i] = p[i & 255];
  const GX = [1, -1, 1, -1, 1.414, -1.414, 0, 0], GY = [1, 1, -1, -1, 0, 0, 1.414, -1.414];
  return (x, y) => {
    const xi = Math.floor(x), yi = Math.floor(y), xf = x - xi, yf = y - yi;
    const X = xi & 255, Y = yi & 255;
    const u = xf * xf * xf * (xf * (xf * 6 - 15) + 10), v = yf * yf * yf * (yf * (yf * 6 - 15) + 10);
    const a = perm[perm[X] + Y] & 7, b = perm[perm[X + 1] + Y] & 7, c = perm[perm[X] + Y + 1] & 7, d = perm[perm[X + 1] + Y + 1] & 7;
    const n00 = GX[a] * xf + GY[a] * yf, n10 = GX[b] * (xf - 1) + GY[b] * yf;
    const n01 = GX[c] * xf + GY[c] * (yf - 1), n11 = GX[d] * (xf - 1) + GY[d] * (yf - 1);
    const top = n00 + (n10 - n00) * u, bot = n01 + (n11 - n01) * u;
    return (top + (bot - top) * v) * 0.9;
  };
}
const N1 = makeNoise(11), N2 = makeNoise(23), N3 = makeNoise(37), N4 = makeNoise(51);
export const noise = N1;
export function fbm(x, y, oct = 4, n = N2) {
  let s = 0, a = 0.5, f = 1;
  for (let i = 0; i < oct; i++) { s += a * n(x * f, y * f); f *= 2.03; a *= 0.5; }
  return s;
}
function ridged(x, y, oct = 5) {
  let s = 0, a = 0.5, f = 1, w = 1;
  for (let i = 0; i < oct; i++) {
    let r = 1 - Math.abs(N1(x * f + i * 17.3, y * f - i * 9.1));
    r *= r;
    s += r * a * w;
    w = clamp(r * 1.6);
    f *= 2.1; a *= 0.5;
  }
  return s; // ~0 … 1
}
export function hash(i) { i = Math.imul(i ^ 0x9e3779b9, 0x85ebca6b); i ^= i >>> 13; i = Math.imul(i, 0xc2b2ae35); i ^= i >>> 16; return (i >>> 0) / 4294967296; }

// ---- Light ---------------------------------------------------------------------------------
export const SUN = norm([-0.86, 0.095, -0.44]);      // low golden-hour sun, off the left of the flight (side light)
export const WATER = 0;
export const EXT = 1300;                             // main terrain: x, z in [−EXT, EXT]

// ---- The route -------------------------------------------------------------------------------
// One continuous flight, z decreasing: a dive onto a mirror lake, a skim, a climb over a saddle,
// down through a small city, a narrow river canyon, a climb over the last ridge into the sun.
export const WAYPOINTS = [
  [40, 150, 1290], [36, 72, 1180], [28, 10, 1060],                 // the dive onto the lake
  [6, 5.5, 930], [16, 5.5, 800], [10, 14, 690],                  // skim
  [0, 50, 615], [-12, 66, 545],                                   // over the saddle
  [-36, 36, 470], [-12, 30, 370], [30, 31, 270], [18, 27, 175],  // the city
  [-8, 17, 85], [-44, 15, -35], [4, 15, -155], [48, 17, -275], [20, 18, -395], // the canyon
  [-14, 52, -495], [-6, 108, -585], [8, 162, -690],                // climb
  [18, 168, -800], [8, 142, -930], [0, 136, -1070], [-6, 133, -1200], // the vista (last point aims)
];

// Valley profiles along z (the route runs along −z): floor height, half-width, slope width, minimum walls.
const TABLE = [
  //   z     floor  half  slope  wall  floorAmp
  [1400, -14, 230, 380, 50, 0],
  [665, -14, 220, 360, 50, 0],
  [615, 4, 200, 320, 50, 2],
  [560, 30, 180, 300, 60, 3],
  [490, 8, 220, 220, 70, 0.6],
  [200, 8, 210, 200, 70, 0.6],
  [130, 0, 80, 120, 100, 0],
  [80, -10, 30, 72, 112, 0],
  [-420, -10, 30, 72, 112, 0],
  [-530, 40, 90, 160, 70, 3],
  [-700, 104, 150, 220, 50, 5],
  [-830, 22, 380, 340, 40, 4],
  [-930, -30, 600, 420, 40, 0],
  [-1400, -30, 700, 450, 40, 0],
];
function profile(z, k) {
  if (z >= TABLE[0][0]) return TABLE[0][k];
  for (let i = 0; i < TABLE.length - 1; i++) {
    const a = TABLE[i], b = TABLE[i + 1];
    if (z >= b[0]) { const t = (a[0] - z) / (a[0] - b[0]); return mix(a[k], b[k], t * t * (3 - 2 * t)); }
  }
  return TABLE[TABLE.length - 1][k];
}

export async function buildWorld(A) {
  const { mesh, instances, texture, flightPath } = A;
  const t0 = performance.now();
  const log = (m) => process.env.ANIKIT_WORLD_LOG && console.error(`world: ${m} (${((performance.now() - t0) / 1000).toFixed(1)}s)`);
  const path3 = flightPath(WAYPOINTS, { lookAhead: 90, bank: 0.07, maxBank: 8, bankWindow: 300 });
  // Centre line x(z) from the flown path (the valley follows the camera exactly).
  const pts = path3.points(3000);
  const CZ0 = 1500, CZ1 = -1500, CN = 3001;
  const cx = new Float32Array(CN), cy = new Float32Array(CN);
  {
    let j = 0;
    for (let i = 0; i < CN; i++) {
      const z = CZ0 + (CZ1 - CZ0) * (i / (CN - 1));
      while (j < pts.length - 2 && pts[j + 1][2] > z) j++;
      const a = pts[j], b = pts[j + 1];
      if (z > pts[0][2]) { cx[i] = pts[0][0]; cy[i] = pts[0][1]; }
      else if (z < pts[pts.length - 1][2]) { cx[i] = pts[pts.length - 1][0]; cy[i] = pts[pts.length - 1][1]; }
      else { const t = (a[2] - z) / (a[2] - b[2] || 1); cx[i] = mix(a[0], b[0], clamp(t)); cy[i] = mix(a[1], b[1], clamp(t)); }
    }
  }
  const camY = (z) => { const f = clamp((z - CZ0) / (CZ1 - CZ0)) * (CN - 1), i = Math.min(CN - 2, Math.floor(f)); return mix(cy[i], cy[i + 1], f - i); };
  const centerX = (z) => { const f = clamp((z - CZ0) / (CZ1 - CZ0)) * (CN - 1), i = Math.min(CN - 2, Math.floor(f)); return mix(cx[i], cx[i + 1], f - i); };

  // ---- Height ------------------------------------------------------------------------------
  const heightAt = (x, z) => {
    const f = profile(z, 1), w0 = profile(z, 2), sw = profile(z, 3), wm = profile(z, 4), fa = profile(z, 5);
    const canyon = w0 < 60;
    const d = Math.abs(x - centerX(z)) + (canyon ? 7 : 40) * N3(x / (canyon ? 55 : 260), z / (canyon ? 55 : 260));
    const w = w0 * (1 + (canyon ? 0.12 : 0.28) * N4(x / 400 + 3, z / 400));
    let base = 60 + 300 * ridged(x / 720 + 4.1, z / 720 - 1.7) + 0.12 * Math.max(0, d - w);
    base = Math.max(base, f + wm * (0.8 + 0.4 * N2(x / 300, z / 300 + 9)));
    const k = smooth(w, w + sw, d);
    let h = f + (base - f) * k;
    if (canyon) h += k * (1 - k) * 4 * 9 * N1(x / 30, z / 30 + 5) + k * 3 * Math.sin(h * 0.6 + N2(x / 50, z / 50) * 2); // ledges and strata
    h += (1 - k) * fa * fbm(x / 70, z / 70, 3, N3);
    // A shallow, flat lake bed just under the water (it never reflects as a "ceiling").
    if (h < -1.5) h = -1.5 - 0.5 * (1 - Math.exp((h + 1.5) * 0.3));
    return h;
  };

  // ---- Coarse grid (the mesh) ------------------------------------------------------------
  const GN = 401, GS = (2 * EXT) / (GN - 1);
  const gh = new Float32Array(GN * GN);
  for (let j = 0; j < GN; j++) for (let i = 0; i < GN; i++) gh[j * GN + i] = heightAt(-EXT + i * GS, -EXT + j * GS);
  const groundAt = (x, z) => {
    const fx = clamp((x + EXT) / GS, 0, GN - 1.001), fz = clamp((z + EXT) / GS, 0, GN - 1.001);
    const i = Math.floor(fx), j = Math.floor(fz), tx = fx - i, tz = fz - j, a = j * GN + i;
    return mix(mix(gh[a], gh[a + 1], tx), mix(gh[a + GN], gh[a + GN + 1], tx), tz);
  };
  // Split at the waterline: cells wholly under water are drawn but never reflected (mirrored, they
  // would float above the water as a ceiling).
  const under = (i, j) => gh[j * GN + i] < -0.3 && gh[j * GN + i + 1] < -0.3 && gh[(j + 1) * GN + i] < -0.3 && gh[(j + 1) * GN + i + 1] < -0.3;
  const gpos = (i, j) => [-EXT + i * GS, gh[j * GN + i], -EXT + j * GS], guv = (i, j) => [i / (GN - 1), j / (GN - 1)];
  // Steep cells (cliffs) get their own mesh with wall-space UVs, so their texture isn't
  // stretched down the face: grouped by the way they face, mapped (z, y) or (x, y).
  const wallGroup = new Int8Array((GN - 1) * (GN - 1)).fill(-1);
  for (let j = 0; j < GN - 1; j++) for (let i = 0; i < GN - 1; i++) {
    const a = gh[j * GN + i], b = gh[j * GN + i + 1], c = gh[(j + 1) * GN + i], d = gh[(j + 1) * GN + i + 1];
    if (Math.min(a, b, c, d) < -0.3) continue;
    const dx = (b + d - a - c) / (2 * GS), dz = (c + d - a - b) / (2 * GS);
    if (1 / Math.hypot(dx, 1, dz) > WALL_NY) continue;
    wallGroup[j * (GN - 1) + i] = Math.abs(dx) > Math.abs(dz) ? (dx < 0 ? 0 : 1) : (dz < 0 ? 2 : 3);
  }
  const isWall = (i, j) => wallGroup[j * (GN - 1) + i] >= 0;
  const terrainMesh = gridMesh(mesh, GN, GN, gpos, guv, (i, j) => under(i, j) || isWall(i, j));
  const terrainBelow = gridMesh(mesh, GN, GN, gpos, guv, (i, j) => !under(i, j));
  const walls = wallMesh(mesh, GN, GS, gh, wallGroup);
  log('terrain mesh');

  // ---- Far mountains (a ring around the valley, 40 km across) -------------------------------
  const FN = 321, FE = 20000, FS = (2 * FE) / (FN - 1);
  const farH = (x, z) => {
    const r = Math.hypot(x, z);
    let h = 120 + 1500 * ridged(x / 5200 + 1.3, z / 5200 + 7.7, 6) * smooth(1500, 9000, r) + 260 * ridged(x / 1500, z / 1500, 4)
      + 240 * ridged(x / 650 + 3.3, z / 650 - 2.1, 3) * smooth(1800, 5000, r);
    // Meet the valley terrain at its edge (no shards where the two meshes join).
    const k = smooth(EXT - 200, EXT + 2600, Math.max(Math.abs(x), Math.abs(z)));
    if (k < 1) h = mix(heightAt(clamp(x, -EXT, EXT), clamp(z, -EXT, EXT)) - 4, h, k);
    // The valley beyond the last ridge opens onto a long lake (straight on from the valley's).
    const vm = Math.exp(-(((x + 300 * Math.sin(z / 2500)) / 1500) ** 2)) * smooth(-1000, -1350, z) * (1 - smooth(-11000, -16000, z));
    h = mix(h, -22 + 6 * N2(x / 900, z / 900), smooth(0.3, 0.7, vm));
    return h;
  };
  const fh = new Float32Array(FN * FN);
  for (let j = 0; j < FN; j++) for (let i = 0; i < FN; i++) fh[j * FN + i] = farH(-FE + i * FS, -FE + j * FS);
  // Cells inside the main terrain are cut out (they would reflect in the lake as a ceiling).
  const farMesh = gridMesh(mesh, FN, FN, (i, j) => [-FE + i * FS, fh[j * FN + i], -FE + j * FS], (i, j) => [i / (FN - 1), j / (FN - 1)],
    (i, j) => Math.max(Math.abs(-FE + (i + 0.5) * FS), Math.abs(-FE + (j + 0.5) * FS)) < EXT + FS * 0.25
      || (fh[j * FN + i] < -1 && fh[j * FN + i + 1] < -1 && fh[(j + 1) * FN + i] < -1 && fh[(j + 1) * FN + i + 1] < -1));
  log('far mesh');

  // ---- Trees -------------------------------------------------------------------------------
  const trees = [];
  {
    let s = 7;
    const rnd = () => ((s = (s * 1664525 + 1013904223) >>> 0) / 4294967296);
    for (let k = 0; k < 600000 && trees.length < 15000; k++) {
      const z = mix(1350, -1250, rnd());
      const x = centerX(z) + (rnd() * 2 - 1) * 900;
      if (Math.abs(x) > EXT - 10) continue;
      const h = groundAt(x, z);
      if (h < 2.2 || h > 300) continue;
      const sl = Math.hypot(groundAt(x + 4, z) - groundAt(x - 4, z), groundAt(x, z + 4) - groundAt(x, z - 4)) / 8;
      if (sl > 0.75) continue;
      const inCity = z < 495 && z > 165 && Math.abs(x - centerX(z)) < profile(z, 2) - 10;
      if (inCity) continue;
      if (Math.abs(x - centerX(z)) < 18 && h + 26 > camY(z) - 5) continue; // keep the flight line clear
      const dens = 0.62 + 0.9 * fbm(x / 240, z / 240, 3, N4) - sl * 0.4 - (h > 220 ? (h - 220) / 90 : 0);
      if (rnd() > dens) continue;
      const ht = 11 + 13 * rnd();
      const tr = { x, y: h - 1, z, h: ht, w: 0.75 + 0.35 * rnd(), ry: rnd() * 360, tint: rnd() };
      // Three silhouettes: fir, slender spruce (higher up), broadleaf (low, by the water).
      const kh = hash(trees.length * 7 + 3);
      tr.kind = h < 45 && kh < 0.3 + 0.2 * smooth(30, 5, h) ? 2 : kh > (h > 120 ? 0.45 : 0.72) ? 1 : 0;
      if (tr.kind === 2) { tr.h *= 0.7; tr.w *= 1.1; }
      if (tr.kind === 1) { tr.h *= 1.12; tr.w *= 0.8; }
      trees.push(tr);
    }
  }
  log(`${trees.length} trees`);

  // ---- The city ----------------------------------------------------------------------------
  const towers = []; // { x, y, z, w, d, h, cls }
  {
    let s = 99;
    const rnd = () => ((s = (s * 1664525 + 1013904223) >>> 0) / 4294967296);
    for (let gz = 485; gz > 160; gz -= 38) {
      for (let gx = -6; gx <= 6; gx++) {
        const z = gz + (rnd() - 0.5) * 10, c = centerX(z);
        const x = c + gx * 36 + (rnd() - 0.5) * 8;
        const off = Math.abs(x - c);
        if (off < 30 || off > profile(z, 2) - 12) continue;
        const near = off < 75;
        const h = near ? 45 + 70 * rnd() : off < 140 ? 22 + 55 * rnd() : 9 + 22 * rnd();
        const w = 14 + 14 * rnd(), d = 14 + 14 * rnd();
        const fl = groundAt(x, z);
        towers.push({ x, z, w, d, h, y: fl - 2 + h / 2, cls: h > 60 ? 0 : h > 30 ? 1 : 2 });
      }
    }
  }
  log(`${towers.length} buildings`);

  // ---- Bakes ---------------------------------------------------------------------------------
  const T = 4096;
  const tex = await cached('terrain2', () => bakeTerrain(T, heightAt, groundAt, gh, GN, GS, trees, centerX, log));
  const wallTex = await cached('walls4', () => bakeWalls(heightAt, groundAt, walls.tris, log));
  const farTex = await cached('far2', () => bakeFar(4096, farH, FE));
  const sky = await cached('sky', () => bakeSky(4096, 2048, false));
  const skyComic = await cached('skycomic', () => bakeSky(2048, 1024, true));
  const skyNeon = await cached('skyneon', () => bakeNeonSky(4096, 2048));
  log('bakes');
  const mk = (img) => texture(img);
  const textures = {
    real: mk(tex.real), comic: mk(tex.comic), neon: mk(tex.neon),
    wallReal: mk(wallTex.real), wallComic: mk(wallTex.comic), wallNeon: mk(wallTex.neon),
    farReal: mk(farTex.real), farComic: mk(farTex.comic), farNeon: mk(farTex.neon), skyNeon: mk(skyNeon.real),
    sky: mk(sky.real), skyComic: mk(skyComic.real),
    windows: mk(windows(256, 512, 'real')), windowsNeon: mk(windows(256, 512, 'neon')), windowsComic: mk(windows(256, 512, 'comic')),
  };
  log('textures');

  // Tree tint follows the baked sun visibility at its foot (trees in a ridge's shadow are darker).
  const sunVis = tex.vis; // Float32Array visGrid (VN × VN)
  const VN = tex.visN;
  const visAt = (x, z) => {
    const i = clamp(Math.round((x + EXT) / (2 * EXT) * (VN - 1)), 0, VN - 1), j = clamp(Math.round((z + EXT) / (2 * EXT) * (VN - 1)), 0, VN - 1);
    return sunVis[j * VN + i];
  };

  return { path: path3, heightAt, groundAt, centerX, profile, terrainMesh, wallMesh: walls.mesh, terrainBelow, farMesh, trees, towers, textures, visAt, treeMesh: conifer(mesh), treeMeshes: [conifer(mesh), conifer(mesh, true), broadleaf(mesh)], log };
}

// ---- Geometry helpers ------------------------------------------------------------------------
function gridMesh(mesh, nx, nz, pos, uv, skipCell) {
  const P = new Float32Array(nx * nz * 3), NR = new Float32Array(nx * nz * 3), U = new Float32Array(nx * nz * 2);
  for (let j = 0; j < nz; j++) for (let i = 0; i < nx; i++) {
    const k = j * nx + i, p = pos(i, j), t = uv(i, j);
    P.set(p, k * 3); U[k * 2] = t[0]; U[k * 2 + 1] = t[1];
  }
  const y = (i, j) => P[(clamp(j, 0, nz - 1) * nx + clamp(i, 0, nx - 1)) * 3 + 1];
  const sx = P[3] - P[0], sz = P[nx * 3 + 2] - P[2];
  for (let j = 0; j < nz; j++) for (let i = 0; i < nx; i++) {
    const dx = (y(i + 1, j) - y(i - 1, j)) / (2 * sx), dz = (y(i, j + 1) - y(i, j - 1)) / (2 * sz);
    const l = Math.hypot(dx, 1, dz), k = (j * nx + i) * 3;
    NR[k] = -dx / l; NR[k + 1] = 1 / l; NR[k + 2] = -dz / l;
  }
  const idx = [];
  for (let j = 0; j < nz - 1; j++) for (let i = 0; i < nx - 1; i++) {
    if (skipCell && skipCell(i, j)) continue;
    const a = j * nx + i, b = a + 1, c = a + nx + 1, d = a + nx;
    idx.push(a, c, b, a, d, c);
  }
  return mesh.custom(P, NR, new Uint32Array(idx), U);
}

// A conifer: a trunk and six ragged, drooping tiers of branches, unit height (y 0 … 1).
function conifer(mesh, slender = false) {
  const P = [], NR = [], U = [], I = [];
  const S = 9;
  const tier = (y0, y1, r0, seed) => {
    const base = P.length / 3;
    for (let k = 0; k <= S; k++) {
      const a = (k / S) * Math.PI * 2 + seed, c = Math.cos(a), s = Math.sin(a);
      const j = 0.72 + 0.5 * hash((k % S) * 7 + seed * 131), r = r0 * j;
      const droop = (y1 - y0) * 0.22 * j; // branch tips hang below the tier's base
      const n = norm([c, (r / (y1 - y0)) * 0.9, s]);
      P.push(c * r, y0 - droop, s * r, c * r * 0.08, y1, s * r * 0.08);
      NR.push(...n, n[0] * 0.4, 1, n[2] * 0.4);
      U.push(k / S, 0, k / S, 1);
    }
    for (let k = 0; k < S; k++) { const a = base + k * 2; I.push(a, a + 1, a + 3, a, a + 3, a + 2); }
  };
  // Trunk.
  {
    const base = P.length / 3;
    for (let k = 0; k <= 6; k++) {
      const a = (k / 6) * Math.PI * 2, c = Math.cos(a), s = Math.sin(a);
      P.push(c * 0.03, 0, s * 0.03, c * 0.02, 0.3, s * 0.02); NR.push(c, 0, s, c, 0, s); U.push(k / 6, 0, k / 6, 1);
    }
    for (let k = 0; k < 6; k++) { const a = base + k * 2; I.push(a, a + 1, a + 3, a, a + 3, a + 2); }
  }
  const T = slender
    ? [[0.1, 0.3, 0.2], [0.2, 0.4, 0.185], [0.3, 0.5, 0.165], [0.4, 0.6, 0.145], [0.5, 0.7, 0.12], [0.6, 0.8, 0.095], [0.7, 0.9, 0.07], [0.8, 1.0, 0.045]]
    : [[0.14, 0.4, 0.26], [0.27, 0.52, 0.23], [0.4, 0.64, 0.19], [0.52, 0.76, 0.15], [0.64, 0.88, 0.11], [0.76, 1.0, 0.07]];
  T.forEach(([y0, y1, r], i) => tier(y0, y1, r, i * 1.7 + 0.3));
  return mesh.custom(new Float32Array(P), new Float32Array(NR), new Uint32Array(I), new Float32Array(U));
}

// A broadleaf tree: a short trunk and a crown of lumpy overlapping blobs, unit height.
function broadleaf(mesh) {
  const P = [], NR = [], U = [], I = [];
  const blob = (cx, cy, cz, r, seed) => {
    const R = 8, S = 12, base = P.length / 3;
    for (let i = 0; i <= R; i++) {
      const th = (i / R) * Math.PI;
      for (let j = 0; j <= S; j++) {
        const ph = (j / S) * Math.PI * 2, d = [Math.sin(th) * Math.cos(ph), Math.cos(th), Math.sin(th) * Math.sin(ph)];
        const k = 1 + 0.22 * N3(d[0] * 2.1 + seed, d[2] * 2.1 + d[1] * 1.7) + 0.1 * N4(d[0] * 5 + seed, d[1] * 5 - d[2] * 3);
        P.push(cx + d[0] * r * k, cy + d[1] * r * k * 0.85, cz + d[2] * r * k);
        NR.push(...norm([d[0], d[1] * 1.2 + 0.25, d[2]]));
        U.push(j / S, i / R);
      }
    }
    for (let i = 0; i < R; i++) for (let j = 0; j < S; j++) { const a = base + i * (S + 1) + j, b = a + S + 1; I.push(a, b, b + 1, a, b + 1, a + 1); }
  };
  {
    const base = P.length / 3;
    for (let k = 0; k <= 6; k++) {
      const a = (k / 6) * Math.PI * 2, c = Math.cos(a), s = Math.sin(a);
      P.push(c * 0.05, 0, s * 0.05, c * 0.03, 0.5, s * 0.03); NR.push(c, 0, s, c, 0, s); U.push(k / 6, 0, k / 6, 1);
    }
    for (let k = 0; k < 6; k++) { const a = base + k * 2; I.push(a, a + 1, a + 3, a, a + 3, a + 2); }
  }
  blob(0, 0.62, 0, 0.3, 1.3); blob(0.17, 0.5, 0.08, 0.22, 4.1); blob(-0.15, 0.52, -0.1, 0.23, 7.7); blob(0.02, 0.8, -0.04, 0.2, 9.2); blob(-0.05, 0.5, 0.18, 0.2, 2.6);
  return mesh.custom(new Float32Array(P), new Float32Array(NR), new Uint32Array(I), new Float32Array(U));
}

// A boulder: a lumpy, flattened sphere, unit radius.
export function boulder(mesh, seed) {
  const R = 9, S = 14, P = [], NR = [], U = [], I = [];
  const f = (x, y, z) => 1 + 0.28 * N3(x * 1.3 + seed, z * 1.3 + y) + 0.12 * N4(x * 3 + y, z * 3 - seed);
  for (let i = 0; i <= R; i++) {
    const th = (i / R) * Math.PI;
    for (let j = 0; j <= S; j++) {
      const ph = (j / S) * Math.PI * 2;
      const d = [Math.sin(th) * Math.cos(ph), Math.cos(th), Math.sin(th) * Math.sin(ph)];
      const k = f(...d);
      P.push(d[0] * k, d[1] * k * 0.62, d[2] * k);
      NR.push(...norm([d[0], d[1] * 1.4, d[2]]));
      U.push(j / S, i / R);
    }
  }
  for (let i = 0; i < R; i++) for (let j = 0; j < S; j++) { const a = i * (S + 1) + j, b = a + S + 1; I.push(a, b, b + 1, a, b + 1, a + 1); }
  return mesh.custom(new Float32Array(P), new Float32Array(NR), new Uint32Array(I), new Float32Array(U));
}

// ---- Cache -------------------------------------------------------------------------------------
async function cached(name, make) {
  const meta = path.join(CACHE, `${VERSION}-${name}.json`);
  if (fs.existsSync(meta)) {
    const m = JSON.parse(fs.readFileSync(meta, 'utf8'));
    const out = {};
    for (const [k, v] of Object.entries(m)) {
      if (v.kind === 'value') { out[k] = v.value; continue; }
      const buf = fs.readFileSync(path.join(CACHE, `${VERSION}-${name}-${k}.bin`));
      const ab = buf.buffer.slice(buf.byteOffset, buf.byteOffset + buf.byteLength);
      if (v.kind === 'img') out[k] = { width: v.width, height: v.height, data: new Uint8ClampedArray(ab) };
      else if (v.kind === 'f32') out[k] = new Float32Array(ab);
      else out[k] = v.value;
    }
    return out;
  }
  const out = await make();
  fs.mkdirSync(CACHE, { recursive: true });
  const m = {};
  for (const [k, v] of Object.entries(out)) {
    const tmp = path.join(CACHE, `${VERSION}-${name}-${k}.bin.${process.pid}`);
    if (v && v.data) { fs.writeFileSync(tmp, v.data); m[k] = { kind: 'img', width: v.width, height: v.height }; }
    else if (v instanceof Float32Array) { fs.writeFileSync(tmp, new Uint8Array(v.buffer, v.byteOffset, v.byteLength)); m[k] = { kind: 'f32' }; }
    else { m[k] = { kind: 'value', value: v }; continue; }
    fs.renameSync(tmp, path.join(CACHE, `${VERSION}-${name}-${k}.bin`));
  }
  const tmp = `${meta}.${process.pid}`;
  fs.writeFileSync(tmp, JSON.stringify(m));
  fs.renameSync(tmp, meta);
  return out;
}

// ---- Colour helpers ------------------------------------------------------------------------------
const toByte = (v) => (v <= 0 ? 0 : v >= 1 ? 255 : Math.round(255 * Math.pow(v, 1 / 2.2)));
// A gentle filmic shoulder (linear in, linear out ≤ 1).
const tone = (v) => { const x = Math.max(0, v); return x / (1 + 0.45 * x) * 1.45; };

// ---- Terrain bake ----------------------------------------------------------------------------------
function bakeTerrain(T, heightAt, groundAt, gh, GN, GS, trees, centerX, log) {
  // Fine heights at texel centres (with micro relief), the source of shading normals.
  const fine = new Float32Array(T * T);
  const ts = (2 * EXT) / T;
  for (let j = 0; j < T; j++) {
    const z = -EXT + (j + 0.5) * ts;
    for (let i = 0; i < T; i++) {
      const x = -EXT + (i + 0.5) * ts;
      const g = groundAt(x, z);
      const rough = g > 1 ? 1 : 0.2;
      const base = heightAt(x, z);
      const gm = Math.hypot(groundAt(x + 5, z) - groundAt(x - 5, z), groundAt(x, z + 5) - groundAt(x, z - 5)) / 10;
      const steep = smooth(0.45, 1.3, gm);
      const relief = steep * (5 * (1 - Math.abs(N1(x / 34, z / 34))) ** 2 + 2.2 * N2(x / 13, z / 13) + 0.8 * N4(x / 4.5, z / 4.5));
      fine[j * T + i] = base + rough * (0.9 * N3(x / 9, z / 9) + 0.35 * N4(x / 3.1, z / 3.1)) + relief;
    }
  }
  log && log('fine heights');
  // Sun visibility on a mid grid (soft heightfield shadows), marched towards the sun.
  const VN = 1024, vs = (2 * EXT) / (VN - 1);
  const vis = new Float32Array(VN * VN);
  const hz = Math.hypot(SUN[0], SUN[2]), dx = SUN[0] / hz, dz = SUN[2] / hz, rise = SUN[1] / hz;
  const hAt = (x, z) => {
    const fx = (x + EXT) / ts - 0.5, fz = (z + EXT) / ts - 0.5;
    if (fx < 0 || fz < 0 || fx >= T - 1 || fz >= T - 1) return -1e9;
    const i = fx | 0, j = fz | 0, tx = fx - i, tz = fz - j, a = j * T + i;
    return (fine[a] * (1 - tx) + fine[a + 1] * tx) * (1 - tz) + (fine[a + T] * (1 - tx) + fine[a + T + 1] * tx) * tz;
  };
  for (let j = 0; j < VN; j++) for (let i = 0; i < VN; i++) {
    const x0 = -EXT + i * vs, z0 = -EXT + j * vs, y0 = hAt(x0, z0) + 0.5;
    let v = 1;
    for (let t = 3; t < 1800; t += t < 60 ? 2.5 : t < 400 ? 6 : 14) {
      const hh = hAt(x0 + dx * t, z0 + dz * t);
      if (hh < -1e8) break;
      const ex = hh - (y0 + rise * t);
      if (ex > 0) { v = 0; break; }
      v = Math.min(v, -ex / (0.025 * t + 0.6));
    }
    vis[j * VN + i] = clamp(v);
  }
  log && log('sun visibility');
  const visAt = (x, z) => {
    const fx = clamp((x + EXT) / vs, 0, VN - 1.001), fz = clamp((z + EXT) / vs, 0, VN - 1.001);
    const i = fx | 0, j = fz | 0, tx = fx - i, tz = fz - j, a = j * VN + i;
    return (vis[a] * (1 - tx) + vis[a + 1] * tx) * (1 - tz) + (vis[a + VN] * (1 - tx) + vis[a + VN + 1] * tx) * tz;
  };
  // Sky visibility / ambient occlusion from blurred heights (two radii).
  const blur = (src, n, r) => {
    const tmp = new Float32Array(n * n), out = new Float32Array(n * n);
    for (let j = 0; j < n; j++) { let s = 0; const row = j * n; for (let i = -r; i <= r; i++) s += src[row + clamp(i, 0, n - 1)]; for (let i = 0; i < n; i++) { tmp[row + i] = s / (2 * r + 1); s += src[row + clamp(i + r + 1, 0, n - 1)] - src[row + clamp(i - r, 0, n - 1)]; } }
    for (let i = 0; i < n; i++) { let s = 0; for (let j = -r; j <= r; j++) s += tmp[clamp(j, 0, n - 1) * n + i]; for (let j = 0; j < n; j++) { out[j * n + i] = s / (2 * r + 1); s += tmp[clamp(j + r + 1, 0, n - 1) * n + i] - tmp[clamp(j - r, 0, n - 1) * n + i]; } }
    return out;
  };
  const mid = new Float32Array(VN * VN);
  for (let j = 0; j < VN; j++) for (let i = 0; i < VN; i++) mid[j * VN + i] = hAt(-EXT + i * vs, -EXT + j * vs);
  const b1 = blur(mid, VN, 6), b2 = blur(mid, VN, 24);
  const midAt = (arr, x, z) => {
    const fx = clamp((x + EXT) / vs, 0, VN - 1.001), fz = clamp((z + EXT) / vs, 0, VN - 1.001);
    const i = fx | 0, j = fz | 0, tx = fx - i, tz = fz - j, a = j * VN + i;
    return (arr[a] * (1 - tx) + arr[a + 1] * tx) * (1 - tz) + (arr[a + VN] * (1 - tx) + arr[a + VN + 1] * tx) * tz;
  };
  log && log('occlusion');
  // Tree shadows and canopy: long golden-hour shadows stamped away from the sun.
  const shade = new Float32Array(T * T);
  const canopy = new Float32Array(T * T);
  const tan = SUN[1] / hz;
  for (const tr of trees) {
    const L = Math.min(170, tr.h / tan * 0.85), wd = tr.h * 0.2 * tr.w;
    const steps = Math.ceil(L / (ts * 0.7));
    for (let k = 0; k <= steps; k++) {
      const f = k / steps, cxp = tr.x - dx * L * f, czp = tr.z - dz * L * f;
      const half = wd * (f < 0.15 ? 0.6 + f * 2.6 : 1 - (f - 0.15) / 0.85 * 0.95);
      const n = Math.ceil(half / ts);
      const ci = Math.round((cxp + EXT) / ts - 0.5), cj = Math.round((czp + EXT) / ts - 0.5);
      for (let a = -n; a <= n; a++) for (let b = -n; b <= n; b++) {
        const ii = ci + a, jj = cj + b;
        if (ii < 0 || jj < 0 || ii >= T || jj >= T) continue;
        const r = Math.hypot(a, b) * ts / half;
        if (r > 1) continue;
        const v = (1 - r * r) * 0.85;
        if (v > shade[jj * T + ii]) shade[jj * T + ii] = v;
      }
    }
    // The canopy footprint itself (seen from above between the cones: dark needles).
    const cr = tr.h * 0.22 * tr.w, n = Math.ceil(cr / ts);
    const ci = Math.round((tr.x + EXT) / ts - 0.5), cj = Math.round((tr.z + EXT) / ts - 0.5);
    for (let a = -n; a <= n; a++) for (let b = -n; b <= n; b++) {
      const ii = ci + a, jj = cj + b;
      if (ii < 0 || jj < 0 || ii >= T || jj >= T) continue;
      const r = Math.hypot(a, b) * ts / cr;
      if (r < 1) canopy[jj * T + ii] = Math.max(canopy[jj * T + ii], 1 - r * r);
    }
  }
  log && log('tree shadows');

  const real = new Uint8ClampedArray(T * T * 4), comic = new Uint8ClampedArray(T * T * 4), neon = new Uint8ClampedArray(T * T * 4);
  const SUNC = [1.75, 1.05, 0.55], SKYC = [0.24, 0.28, 0.46];
  for (let j = 0; j < T; j++) {
    const z = -EXT + (j + 0.5) * ts;
    for (let i = 0; i < T; i++) {
      const x = -EXT + (i + 0.5) * ts, k = j * T + i;
      const h = fine[k];
      const hl = fine[j * T + Math.max(0, i - 1)], hr = fine[j * T + Math.min(T - 1, i + 1)];
      const hd = fine[Math.max(0, j - 1) * T + i], hu = fine[Math.min(T - 1, j + 1) * T + i];
      const gx = (hr - hl) / (2 * ts), gz = (hu - hd) / (2 * ts);
      const il = 1 / Math.hypot(gx, 1, gz), nx = -gx * il, ny = il, nz = -gz * il;
      const slope = 1 - ny;
      const ndl = Math.max(0, nx * SUN[0] + ny * SUN[1] + nz * SUN[2]);
      const sv = visAt(x, z) * (1 - shade[k]);
      const cav = clamp(1 + (midAt(b1, x, z) - h) * -0.05) * clamp(1 + (midAt(b2, x, z) - h) * -0.012, 0.35, 1);
      // Materials.
      const n1 = fbm(x / 26, z / 26, 3, N1), n2 = N2(x / 5.5, z / 5.5), n3 = fbm(x / 140, z / 140, 2, N4);
      const gmc = Math.hypot(groundAt(x + 3.3, z) - groundAt(x - 3.3, z), groundAt(x, z + 3.3) - groundAt(x, z - 3.3)) / 6.6;
      const rock = Math.max(smooth(0.38, 0.62, slope + 0.12 * n1), smooth(0.95, 1.2, gmc + 0.15 * n1)); // rock wherever the mesh nears a cliff (meets the wall atlas)
      const sand = (1 - smooth(0.4, 1.6, h + n2 * 0.6)) * (1 - rock);
      const forest = clamp(canopy[k] * 1.4 + 0.5 * smooth(0.1, 0.6, fbm(x / 240, z / 240, 3, N4) + 0.3)) * (1 - rock) * (1 - sand);
      const dry = smooth(-0.05, 0.35, n3) * (1 - forest) * (1 - rock) * (1 - sand);
      const grass = Math.max(0, 1 - rock - sand - forest - dry);
      const strata = 0.5 + 0.22 * Math.sin(h * 0.55 + n1 * 6 + 3 * N4(x / 60, z / 60)) + 0.28 * n2;
      const alb = [0, 0, 0], cpal = [0, 0, 0];
      const add = (w, c, cc) => { for (let q = 0; q < 3; q++) { alb[q] += w * c[q]; cpal[q] += w * cc[q]; } };
      add(grass, [0.085 + 0.03 * n1, 0.13 + 0.03 * n1, 0.04], [0.36, 0.66, 0.24]);
      add(dry, [0.25, 0.20, 0.09], [0.86, 0.72, 0.32]);
      add(forest, [0.035, 0.06, 0.035], [0.12, 0.44, 0.26]);
      add(rock, [0.24 + 0.09 * strata, 0.2 + 0.07 * strata, 0.16 + 0.05 * strata], [0.82 - 0.12 * strata, 0.6 - 0.1 * strata, 0.42 - 0.08 * strata]);
      add(sand, [0.2, 0.17, 0.12], [0.96, 0.84, 0.58]);
      const v = 0.82 + 0.3 * (0.5 * n2 + 0.5 * n1);
      for (let q = 0; q < 3; q++) alb[q] *= v;
      // Lighting: sun × visibility + sky × occlusion + a warm bounce.
      const sky = (0.55 + 0.45 * ny) * cav;
      for (let q = 0; q < 3; q++) {
        const lin = alb[q] * (SUNC[q] * ndl * sv * 1.9 + SKYC[q] * sky * 1.25 + 0.05 * SUNC[q] * cav);
        real[k * 4 + q] = toByte(tone(lin));
      }
      real[k * 4 + 3] = 255;
      // Comic: flat palette, two light bands (sun / shade tinted blue-violet).
      const lit = smooth(0.1, 0.16, ndl * sv);
      const shadeT = [0.56, 0.54, 0.74];
      const crack = rock > 0.45 && (Math.abs(N1(x / 16, z / 16)) < 0.035 || Math.abs(Math.sin(h * 0.7 + n1 * 3)) < 0.06) ? 0.12 : 1;
      for (let q = 0; q < 3; q++) comic[k * 4 + q] = Math.round(255 * clamp(cpal[q] * mix(shadeT[q], 1.02, lit) * (0.8 + 0.2 * cav) * crack));
      comic[k * 4 + 3] = 255;
      // Neon: contour lines every 8 m (a brighter one every 40 m), a shoreline, a faint 50 m grid.
      const grad = Math.hypot(gx, gz) * ts;
      const line = (period, width) => { const f = h / period, d = Math.abs(f - Math.round(f)) * period; return clamp(1 - d / Math.max(0.05, width * (grad + 0.06))); };
      const minor = line(8, 1.4), major = line(40, 2.2);
      const gl = (q) => { const f = q / 50, d = Math.abs(f - Math.round(f)) * 50; return clamp(1 - d / 0.9); };
      const grid = Math.max(gl(x), gl(z)) * 0.18;
      const hn = clamp((h + 5) / 260);
      const col = [mix(1.0, 0.15, hn), mix(0.18, 0.8, hn), mix(0.75, 1.0, hn)];
      const shore = clamp(1 - Math.abs(h - 0.2) / 0.9) * (h > -1.4 ? 1 : 0);
      for (let q = 0; q < 3; q++) {
        const e = col[q] * (minor * 0.45 + major * 1.0) + [0.2, 0.9, 1.0][q] * shore * 1.2 + [0.25, 0.3, 0.9][q] * grid;
        neon[k * 4 + q] = Math.round(255 * clamp(e));
      }
      neon[k * 4 + 3] = 255;
    }
  }
  log && log('terrain textures');
  const visOut = new Float32Array(VN * VN);
  for (let j = 0; j < VN; j++) for (let i = 0; i < VN; i++) visOut[j * VN + i] = vis[j * VN + i];
  return { real: { width: T, height: T, data: real }, comic: { width: T, height: T, data: comic }, neon: { width: T, height: T, data: neon }, vis: visOut, visN: VN };
}

function bakeFar(T, farH, FE) {
  const real = new Uint8ClampedArray(T * T * 4), comic = new Uint8ClampedArray(T * T * 4), neon = new Uint8ClampedArray(T * T * 4);
  const ts = (2 * FE) / T;
  const SUNC = [1.5, 1.05, 0.68], SKYC = [0.3, 0.38, 0.52];
  for (let j = 0; j < T; j++) for (let i = 0; i < T; i++) {
    const x = -FE + (i + 0.5) * ts, z = -FE + (j + 0.5) * ts, k = j * T + i;
    const h = farH(x, z), e = ts;
    let gx = (farH(x + e, z) - farH(x - e, z)) / (2 * e), gz = (farH(x, z + e) - farH(x, z - e)) / (2 * e);
    // Fine relief the coarse mesh can't hold: gullies and ridges for the shading.
    gx += 0.35 * N1(x / 160, z / 160) + 0.18 * N3(x / 60, z / 60); gz += 0.35 * N2(x / 160, z / 160) + 0.18 * N4(x / 60, z / 60);
    // Erosion gullies running down the fall line: ridged noise stretched along the slope.
    {
      const gl = Math.hypot(gx, gz) + 1e-6, ax = gx / gl, az = gz / gl;
      const along = x * ax + z * az, across = -x * az + z * ax;
      const gul = Math.abs(N3(across / 90, along / 420)), steepK = smooth(0.15, 0.6, gl);
      const d = (Math.exp(-((gul / 0.12) ** 2)) - 0.3) * 0.9 * steepK;
      gx += -az * d * Math.sign(N3(across / 90 + 0.05, along / 420) - N3(across / 90 - 0.05, along / 420)); gz += ax * d * Math.sign(N3(across / 90 + 0.05, along / 420) - N3(across / 90 - 0.05, along / 420));
    }
    const il = 1 / Math.hypot(gx, 1, gz), nx = -gx * il, ny = il, nz = -gz * il;
    const ndl = Math.max(0, nx * SUN[0] + ny * SUN[1] + nz * SUN[2]);
    const slope = 1 - ny, n1 = fbm(x / 300, z / 300, 3, N3);
    const n2 = N2(x / 45, z / 45), n4 = fbm(x / 120, z / 120, 3, N1);
    const snow = smooth(820, 1000, h + 160 * n1 + 60 * n2) * (1 - smooth(0.5, 0.68, slope + 0.1 * n2));
    const rock = smooth(0.18, 0.36, slope + 0.12 * n1 + 0.08 * n2 + smooth(500, 900, h) * 0.12) * (1 - snow);
    const scree = rock * smooth(0.1, 0.4, n4) * (1 - smooth(0.4, 0.6, slope));
    const forest = (1 - snow - rock) * (1 - smooth(450, 750, h + 80 * n1)) * smooth(-0.2, 0.1, fbm(x / 700, z / 700, 3, N4) + 0.15 + 0.25 * n4);
    const grass = Math.max(0, 1 - snow - rock - forest);
    const alb = [0, 0, 0], cpal = [0, 0, 0];
    const add = (w, c, cc) => { for (let q = 0; q < 3; q++) { alb[q] += w * c[q]; cpal[q] += w * cc[q]; } };
    const rv = 0.85 + 0.3 * n2;
    add(snow, [0.78, 0.79, 0.84], [0.97, 0.97, 1]);
    add(rock - scree, [0.19 * rv, 0.165 * rv, 0.145 * rv], [0.62, 0.52, 0.5]);
    add(scree, [0.27, 0.25, 0.22], [0.7, 0.62, 0.56]);
    add(forest, [0.03 * (0.8 + 0.4 * n4), 0.05, 0.032], [0.15, 0.45, 0.3]);
    add(grass, [0.11 + 0.04 * n4, 0.12 + 0.02 * n4, 0.055], [0.45, 0.66, 0.3]);
    const water = h < 0 ? 1 : 0;
    for (let q = 0; q < 3; q++) {
      const lin = alb[q] * (SUNC[q] * ndl * 1.7 + SKYC[q] * (0.5 + 0.5 * ny) * 1.2);
      real[k * 4 + q] = toByte(tone(lin * (water ? 0.75 : 1)));
      comic[k * 4 + q] = Math.round(255 * clamp(cpal[q] * (ndl > 0.12 ? 1 : 0.6)));
    }
    // Neon: contour lines every 60 m (brighter every 300 m), fading with distance from the valley.
    const grad = Math.hypot(gx, gz) * ts;
    const ln = (period, w) => { const f = h / period, d = Math.abs(f - Math.round(f)) * period; return clamp(1 - d / Math.max(0.5, w * (grad + 0.05))); };
    const fade = 1 - smooth(4000, 16000, Math.hypot(x, z));
    const hn = clamp(h / 1500);
    const ncol = [mix(1.0, 0.25, hn), mix(0.2, 0.85, hn), mix(0.8, 1.0, hn)];
    const glowN = (ln(60, 1.2) * 0.4 + ln(300, 1.8)) * fade * (water ? 0 : 1);
    for (let q = 0; q < 3; q++) neon[k * 4 + q] = Math.round(255 * clamp(ncol[q] * glowN));
    real[k * 4 + 3] = comic[k * 4 + 3] = neon[k * 4 + 3] = 255;
  }
  return { real: { width: T, height: T, data: real }, comic: { width: T, height: T, data: comic }, neon: { width: T, height: T, data: neon } };
}

// ---- Skies (equirectangular, for a sphere's UVs: u around from +x towards +z, v from the zenith) ----
function bakeSky(TW, TH, comicStyle) {
  const data = new Uint8ClampedArray(TW * TH * 4);
  const ZEN = comicStyle ? [0.06, 0.32, 0.86] : [0.13, 0.19, 0.36];
  const HOR_SUN = comicStyle ? [1.0, 0.86, 0.55] : [1.45, 0.82, 0.42];
  const HOR_AWAY = comicStyle ? [0.62, 0.86, 1.0] : [0.85, 0.66, 0.62];
  const HAZE = HAZE_RGB;
  for (let j = 0; j < TH; j++) {
    const th = ((j + 0.5) / TH) * Math.PI;
    const dy = Math.cos(th), sth = Math.sin(th);
    for (let i = 0; i < TW; i++) {
      const ph = ((i + 0.5) / TW) * Math.PI * 2;
      const d = [sth * Math.cos(ph), dy, sth * Math.sin(ph)];
      const sd = d[0] * SUN[0] + d[1] * SUN[1] + d[2] * SUN[2];
      const sh = clamp((d[0] * SUN[0] + d[2] * SUN[2]) / (Math.hypot(SUN[0], SUN[2]) * Math.max(1e-4, Math.hypot(d[0], d[2]))) * 0.5 + 0.5);
      const hor = [0, 1, 2].map((q) => mix(HOR_AWAY[q], HOR_SUN[q], sh ** 2));
      const el = Math.max(0, dy);
      const g = comicStyle ? clamp(el * 2.2) ** 0.8 : 1 - Math.exp(-el * 2.6);
      let c = [0, 1, 2].map((q) => mix(hor[q], ZEN[q], g));
      // Haze band at (and below) the horizon, matching the distance fog.
      const band = 1 - smooth(0, comicStyle ? 0.0 : 0.05, dy);
      if (!comicStyle) c = c.map((v, q) => mix(v, HAZE[q], band));
      else if (dy < 0) c = [0.62, 0.86, 1.0];
      // Sun glow (Mie) and disc.
      const glow = comicStyle ? 0 : 0.55 * Math.pow(Math.max(0, sd), 90) + 0.22 * Math.pow(Math.max(0, sd), 10);
      c = c.map((v, q) => v + glow * HOR_SUN[q]);
      if (sd > 0.99993) c = comicStyle ? [1, 0.95, 0.55] : [1.6, 1.5, 1.3];
      else if (comicStyle && sd > 0.9994) c = [1, 0.95, 0.55];
      // Clouds on a plane above: scattered cumulus/altocumulus, lit from the low sun.
      if (dy > 0.012) {
        const k = 1 / dy, px = d[0] * k * 2.2, pz = d[2] * k * 2.2;
        const base = fbm(px * 0.55 + 3, pz * 0.55, 5, N2) + 0.35 * fbm(px * 2.2, pz * 2.2, 3, N4);
        const cov = comicStyle ? 0.18 : 0.12;
        const dens = clamp((base - cov) * (comicStyle ? 9 : 3.2));
        const fadeH = smooth(0.012, 0.09, dy);
        let a = dens * fadeH * (comicStyle ? 1 : 0.92);
        if (comicStyle) a = a > 0.5 ? 1 : 0;
        if (a > 0) {
          // Light: the side towards the sun glows, the rest is lit pink-grey from below.
          const thick = clamp(base - cov) * 2;
          const fwd = Math.pow(Math.max(0, sd), 6);
          const lit = comicStyle ? (base > cov + 0.22 ? [0.72, 0.8, 0.95] : [1, 1, 1])
            : [0, 1, 2].map((q) => mix([1.05, 0.72, 0.6][q], [0.42, 0.40, 0.5][q], clamp(thick * 1.6)) + fwd * [1.4, 0.95, 0.55][q] * (1 - clamp(thick * 1.2)));
          c = c.map((v, q) => mix(v, lit[q], a));
        }
      }
      const o = (j * TW + i) * 4;
      for (let q = 0; q < 3; q++) data[o + q] = comicStyle ? Math.round(255 * clamp(c[q])) : toByte(clamp(c[q]));
      data[o + 3] = 255;
    }
  }
  return { real: { width: TW, height: TH, data } };
}
// The neon sky: a violet dusk, stars, and a big banded sun low over the mountains.
function bakeNeonSky(TW, TH) {
  const data = new Uint8ClampedArray(TW * TH * 4);
  const sunAz = Math.atan2(SUN[2], SUN[0]);
  for (let j = 0; j < TH; j++) {
    const th = ((j + 0.5) / TH) * Math.PI, dy = Math.cos(th), sth = Math.sin(th);
    for (let i = 0; i < TW; i++) {
      const ph = ((i + 0.5) / TW) * Math.PI * 2;
      const el = Math.max(0, dy);
      // Gradient: hot magenta at the horizon, violet, then near-black.
      let c = [mix(0.85, 0.02, clamp(el * 4) ** 0.6), mix(0.12, 0.005, clamp(el * 3) ** 0.7), mix(0.55, 0.06, clamp(el * 2.5) ** 0.8)];
      // Stars.
      const sx = Math.floor(ph * 900), sy = Math.floor(th * 900), hs = hash(sx * 7919 + sy * 104729);
      if (hs > 0.9965 && el > 0.05) { const b = 0.35 + 0.65 * hash(sx + sy * 31); c = c.map((v) => v + b * smooth(0.05, 0.3, el)); }
      // The sun: 9° across, yellow at the top to magenta below, cut by horizontal bands near its base.
      let da = ph - sunAz; da = Math.atan2(Math.sin(da), Math.cos(da));
      const ex = da * sth, ey = (Math.asin(clamp(dy, -1, 1)) - 0.075);
      const r = Math.hypot(ex, ey) / 0.08;
      if (r < 1 && dy > -0.01) {
        const v = clamp(0.5 - ey / 0.16);
        const band = ey < 0.02 ? (Math.sin((ey + 0.1) * 260 - (0.02 - ey) * 40) > -0.2 + 1.6 * (0.02 - ey) / 0.1 ? 1 : 0) : 1;
        const sc = [1.0, mix(0.95, 0.15, v), mix(0.35, 0.65, v)];
        if (band) c = sc;
      }
      const glow = Math.exp(-Math.max(0, r - 1) * 1.6) * 0.5 * (dy > -0.01 ? 1 : 0);
      c = [c[0] + glow * 1.0, c[1] + glow * 0.25, c[2] + glow * 0.6];
      const o = (j * TW + i) * 4;
      for (let q = 0; q < 3; q++) data[o + q] = Math.round(255 * clamp(c[q]));
      data[o + 3] = 255;
    }
  }
  return { real: { width: TW, height: TH, data } };
}

// The haze colour at the horizon (linear); the distance fog uses its display value.
export const HAZE_RGB = [0.86, 0.6, 0.42];
export const HAZE_DISPLAY = HAZE_RGB.map((v) => Math.pow(clamp(v), 1 / 2.2));

// ---- Facade textures -------------------------------------------------------------------------------
function windows(W, H, style) {
  const data = new Uint8ClampedArray(W * H * 4);
  const cols = 8, rows = 16, cw = W / cols, rh = H / rows;
  for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
    const ci = Math.floor(x / cw), ri = Math.floor(y / rh), fx = (x % cw) / cw, fy = (y % rh) / rh;
    const pane = fx > 0.12 && fx < 0.92 && fy > 0.16 && fy < 0.86;
    const r = hash(ci * 131 + ri * 7919 + 17), r2 = hash(ci * 31 + ri * 977 + 5);
    let c;
    if (style === 'real') {
      c = pane ? [0.16 + 0.06 * r, 0.2 + 0.06 * r, 0.24 + 0.05 * r] : [0.58, 0.56, 0.52];
      if (pane && r2 > 0.86) c = [0.75, 0.6, 0.38];
      if (pane) { const g = (fx + fy) * 0.5; c = c.map((v) => v * (0.85 + 0.3 * g)); }
    } else if (style === 'neon') {
      const on = r2 > 0.45;
      const hue = r < 0.4 ? [1.0, 0.75, 0.45] : r < 0.7 ? [0.3, 0.85, 1.0] : [1.0, 0.3, 0.8];
      c = pane && on ? hue.map((v) => v * (0.55 + 0.45 * r2)) : [0.0, 0.0, 0.0];
    } else {
      c = pane ? (r2 > 0.8 ? [1, 0.92, 0.5] : [0.16, 0.22, 0.38]) : [1, 1, 1];
      if (!pane && (fx < 0.04 || fy < 0.05)) c = [0.05, 0.05, 0.08];
    }
    const o = (y * W + x) * 4;
    for (let q = 0; q < 3; q++) data[o + q] = Math.round(255 * clamp(c[q]));
    data[o + 3] = 255;
  }
  return { width: W, height: H, data };
}

// ---- Cliff walls: their own mesh and textures in wall space ---------------------------------------
// u runs along the wall (z for walls facing ±x, x for walls facing ±z), v down the height; one strip
// of the atlas per facing, so the two sides of the valley never share texels.
function wallMesh(mesh, GN, GS, gh, wallGroup) {
  const H = (i, j) => gh[clamp(j, 0, GN - 1) * GN + clamp(i, 0, GN - 1)];
  const nrm = (i, j) => { const dx = (H(i + 1, j) - H(i - 1, j)) / (2 * GS), dz = (H(i, j + 1) - H(i, j - 1)) / (2 * GS), l = Math.hypot(dx, 1, dz); return [-dx / l, 1 / l, -dz / l]; };
  const P = [], NR = [], U = [], I = [], tris = [];
  for (let j = 0; j < GN - 1; j++) for (let i = 0; i < GN - 1; i++) {
    const g = wallGroup[j * (GN - 1) + i];
    if (g < 0) continue;
    const base = P.length / 3, vs = [];
    for (const [a, b] of [[0, 0], [1, 0], [1, 1], [0, 1]]) {
      const x = -EXT + (i + a) * GS, z = -EXT + (j + b) * GS, y = H(i + a, j + b), n = nrm(i + a, j + b);
      const u = ((g < 2 ? z : x) + EXT) / (2 * EXT), v = (g + clamp((WY1 - y) / (WY1 - WY0))) / 4;
      P.push(x, y, z); NR.push(...n); U.push(u, v); vs.push([x, y, z, ...n, u, v, g]);
    }
    // Same diagonal as the terrain grid (a, c, b / a, d, c with a = (0,0), b = (1,0), c = (1,1), d = (0,1)).
    I.push(base, base + 2, base + 1, base, base + 3, base + 2);
    tris.push(...vs[0], ...vs[2], ...vs[1], ...vs[0], ...vs[3], ...vs[2]);
  }
  return { mesh: mesh.custom(new Float32Array(P), new Float32Array(NR), new Uint32Array(I), new Float32Array(U)), tris: new Float32Array(tris) };
}

function bakeWalls(heightAt, groundAt, tris, log) {
  const T = WT, n = T * T;
  const real = new Uint8ClampedArray(n * 4), comic = new Uint8ClampedArray(n * 4), neon = new Uint8ClampedArray(n * 4);
  const done = new Uint8Array(n);
  const SUNC = [1.75, 1.05, 0.55], SKYC = [0.24, 0.28, 0.46];
  const hz = Math.hypot(SUN[0], SUN[2]);
  const vy = (WY1 - WY0) / (T / 4);  // metres per texel row
  const shadeTexel = (k, x, y, z, nx, ny, nz, g) => {
    const u = g < 2 ? z : x;          // metres along the wall
    // Rock: sedimentary bands in y (wavy), blocky joints, dark vertical streaks (water stains).
    const w1 = N1(u / 38, y / 38) * 3 + N2(u / 9, y / 9) * 0.8;
    const band = Math.sin((y + w1) * 0.62) * 0.5 + 0.5, band2 = Math.sin((y + w1 * 0.6) * 2.3 + 1.7) * 0.5 + 0.5;
    const streak = clamp(0.5 + 1.3 * N3(u / 2.6, y / 34 + g * 7) + 0.5 * N4(u / 0.9, y / 12)) ** 2;
    const joint = 1 - Math.exp(-((Math.abs(N4(u / 7 + g * 3, y / 70)) / 0.025) ** 2)) * 0.55 * clamp(N1(u / 15, y / 25) + 0.6);
    const grit = 0.82 + 0.18 * N2(u / 0.45, y / 0.45) + 0.12 * N1(u / 1.6, y / 1.6);
    const warm = N3(u / 70 + 5, y / 50);
    // Bump from the same patterns: ledges at the bands, relief along the joints.
    const bh = (uu, yy) => 0.35 * Math.sin((yy + N1(uu / 38, yy / 38) * 3) * 0.62) + 0.55 * N2(uu / 4, yy / 4) + 0.3 * N4(uu / 1.3, yy / 1.3);
    const e = 0.35, du = (bh(u + e, y) - bh(u - e, y)) / (2 * e), dv = (bh(u, y + e) - bh(u, y - e)) / (2 * e);
    const tu = g < 2 ? [0, 0, 1] : [1, 0, 0];
    let mx = nx - 0.55 * du * tu[0], my = ny - 0.55 * dv, mz = nz - 0.55 * du * tu[2];
    const ml = Math.hypot(mx, my, mz); mx /= ml; my /= ml; mz /= ml;
    const ndl = Math.max(0, mx * SUN[0] + my * SUN[1] + mz * SUN[2]);
    // Sun visibility: march from the face towards the sun over the terrain mesh.
    let sv = ndl > 0 ? 1 : 0;
    if (sv > 0) {
      const ox = x + nx * 1.2, oy = y + ny * 1.2, oz = z + nz * 1.2;
      for (let t = 2; t < 1600; t *= 1.13) {
        const px = ox + SUN[0] * t, pz = oz + SUN[2] * t;
        if (Math.abs(px) > EXT || Math.abs(pz) > EXT) break;
        const ex = groundAt(px, pz) - (oy + SUN[1] * t);
        if (ex > 0) { sv = 0; break; }
        sv = Math.min(sv, -ex / (0.03 * t + 0.4));
      }
      sv = clamp(sv);
    }
    // Moss / grass on the ledges (faces turned up), lichen patches.
    const ledge = smooth(0.5, 0.72, my + 0.2 * N2(u / 6, y / 6)) * (1 - smooth(260, 330, y));
    const s = 0.3 * band + 0.15 * band2 + 0.55 * warm;
    let alb = [0.25 + 0.1 * s, 0.205 + 0.075 * s, 0.165 + 0.05 * s];
    alb = alb.map((v, q) => v * grit * mix(1, [0.55, 0.52, 0.5][q], streak * 0.75) * joint);
    alb = alb.map((v, q) => mix(v, [0.07, 0.1, 0.04][q] * (0.8 + 0.4 * grit), ledge));
    const cav = 0.55 + 0.45 * joint * clamp(0.75 + 0.5 * band2);
    const sky = (0.55 + 0.45 * my) * cav;
    for (let q = 0; q < 3; q++) real[k * 4 + q] = toByte(tone(alb[q] * (SUNC[q] * ndl * sv * 1.9 + SKYC[q] * sky * 1.25 + 0.05 * SUNC[q] * cav)));
    // Comic: flat rock in two light bands, inked strata and cracks with varying weight.
    const lit = smooth(0.08, 0.14, ndl * sv);
    const cr = [0.84 - 0.1 * band, 0.6 - 0.08 * band, 0.44 - 0.06 * band];
    const cg = ledge > 0.5 ? [0.3, 0.62, 0.3] : cr;
    const shadeT = [0.5, 0.48, 0.72];
    const strata = Math.abs(Math.sin((y + w1) * 0.62 * 0.5)) < 0.015 + 0.06 * clamp(N3(u / 45, y / 20) + 0.25) ** 1.5 ? 1 : 0;
    const crack = N1(u / 40, y / 40) > 0.05 && Math.abs(N4(u / 7 + g * 3, y / 70)) < 0.006 + 0.01 * clamp(N1(u / 9, y / 5) + 0.3) ? 1 : 0;
    const ink = strata || crack ? 0.1 : 1;
    for (let q = 0; q < 3; q++) comic[k * 4 + q] = Math.round(255 * clamp(cg[q] * mix(shadeT[q], 1.03, lit) * ink));
    // Neon: contour lines every 8 m (brighter every 40 m) run as crisp horizontal lines.
    const line = (period, width) => { const f = y / period, d = Math.abs(f - Math.round(f)) * period; return clamp(1 - d / width); };
    const hn = clamp((y + 5) / 260);
    const col = [mix(1.0, 0.15, hn), mix(0.18, 0.8, hn), mix(0.75, 1.0, hn)];
    const em = line(8, 0.32) * 0.45 + line(40, 0.55);
    for (let q = 0; q < 3; q++) neon[k * 4 + q] = Math.round(255 * clamp(col[q] * em));
    real[k * 4 + 3] = comic[k * 4 + 3] = neon[k * 4 + 3] = 255;
    done[k] = 1;
  };
  // Rasterise every wall triangle in atlas space; shade each texel at its world position.
  const S = 9;
  for (let t = 0; t < tris.length; t += S * 3) {
    const A = t, B = t + S, C = t + 2 * S;
    const ax = tris[A + 6] * T - 0.5, ay = tris[A + 7] * T - 0.5, bx = tris[B + 6] * T - 0.5, by = tris[B + 7] * T - 0.5, cx = tris[C + 6] * T - 0.5, cy = tris[C + 7] * T - 0.5;
    const den = (by - cy) * (ax - cx) + (cx - bx) * (ay - cy);
    if (Math.abs(den) < 1e-9) continue;
    const g = tris[A + 8];
    const x0 = Math.max(0, Math.floor(Math.min(ax, bx, cx)) - 1), x1 = Math.min(T - 1, Math.ceil(Math.max(ax, bx, cx)) + 1);
    const y0 = Math.max(0, Math.floor(Math.min(ay, by, cy)) - 1), y1 = Math.min(T - 1, Math.ceil(Math.max(ay, by, cy)) + 1);
    for (let py = y0; py <= y1; py++) for (let px = x0; px <= x1; px++) {
      let w0 = ((by - cy) * (px - cx) + (cx - bx) * (py - cy)) / den, w1 = ((cy - ay) * (px - cx) + (ax - cx) * (py - cy)) / den, w2 = 1 - w0 - w1;
      const eps = -0.02;
      if (w0 < eps || w1 < eps || w2 < eps) continue;
      const k = py * T + px;
      if (done[k]) continue;
      w0 = Math.max(0, w0); w1 = Math.max(0, w1); w2 = Math.max(0, w2); const ws = w0 + w1 + w2; w0 /= ws; w1 /= ws; w2 /= ws;
      const at = (o) => tris[A + o] * w0 + tris[B + o] * w1 + tris[C + o] * w2;
      let nx = at(3), ny = at(4), nz = at(5); const l = Math.hypot(nx, ny, nz); nx /= l; ny /= l; nz /= l;
      shadeTexel(k, at(0), at(1), at(2), nx, ny, nz, g);
    }
  }
  log && log('walls shaded');
  void vy; void heightAt; void hz;
  // Grow the painted texels outwards a few texels (filtering at triangle edges never reads black).
  for (let pass = 0; pass < 4; pass++) {
    const add = [];
    for (let py = 0; py < T; py++) for (let px = 0; px < T; px++) {
      const k = py * T + px;
      if (done[k]) continue;
      const nb = px > 0 && done[k - 1] ? k - 1 : px < T - 1 && done[k + 1] ? k + 1 : py > 0 && done[k - T] ? k - T : py < T - 1 && done[k + T] ? k + T : -1;
      if (nb >= 0) add.push(k, nb);
    }
    for (let a = 0; a < add.length; a += 2) {
      const k = add[a], s = add[a + 1];
      for (let q = 0; q < 4; q++) { real[k * 4 + q] = real[s * 4 + q]; comic[k * 4 + q] = comic[s * 4 + q]; neon[k * 4 + q] = neon[s * 4 + q]; }
      done[k] = 1;
    }
  }
  return { real: { width: T, height: T, data: real }, comic: { width: T, height: T, data: comic }, neon: { width: T, height: T, data: neon } };
}
