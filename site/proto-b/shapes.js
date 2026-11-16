// Target shapes for the sculpture. Each returns a Float32Array [x, y, z] * n in
// model space (y up, about ±550 wide). Deterministic, allocation once.
import { staggerValues } from '../../packages/anikit/dist/anikit.js';
import { splats } from '../../packages/anikit/dist/splats.js';

const TAU = Math.PI * 2;
function rng(seed) {
  let s = seed >>> 0 || 1;
  return () => ((s = (s * 1664525 + 1013904223) >>> 0) / 4294967296);
}
function gauss(r) {
  return Math.sqrt(-2 * Math.log(r() + 1e-9)) * Math.cos(TAU * r());
}

/** Soft fog around the stage: where splats wait before the first form. */
export function dust(n) {
  const o = new Float32Array(n * 3), r = rng(7);
  for (let i = 0; i < n; i++) {
    const u = r() * 2 - 1, a = r() * TAU, rad = 520 + r() * 420;
    const s = Math.sqrt(1 - u * u);
    o[i * 3] = Math.cos(a) * s * rad * 1.4;
    o[i * 3 + 1] = u * rad * 0.7;
    o[i * 3 + 2] = Math.sin(a) * s * rad * 0.6 - 500;
  }
  return o;
}

// Positions of a splat set (stride 14), resampled to n with a scattering
// permutation so any prefix of the result covers the whole form.
const STRIDE = 14;
function positionsOf(set, n, flipY = false) {
  const o = new Float32Array(n * 3), c = Math.max(1, set.count), d = set.data;
  for (let i = 0; i < n; i++) {
    const j = ((i * 7919) % c) * STRIDE;
    o[i * 3] = d[j]; o[i * 3 + 1] = flipY ? -d[j + 1] : d[j + 1]; o[i * 3 + 2] = d[j + 2];
  }
  return o;
}

/** Text as splats (splats.fromText), in world units. */
export function lettering(n, text, font, weight, width) {
  return positionsOf(splats.fromText(text, { count: n, width, depth: 70, font, weight }), n);
}

/**
 * A flower: petals as a triangle mesh sampled by splats.fromMesh, a domed
 * centre from splats.fromSDF, a stem from splats.fromPoints; merged.
 */
export function flowerSteps(n, done) {
  const tilt = -0.42, ct = Math.cos(tilt), st = Math.sin(tilt), lift = 60;
  const pos = [], idx = [];
  const put = (x, y, z) => { pos.push(x, y * ct - z * st + lift, y * st + z * ct); return pos.length / 3 - 1; };
  const U = 12, V = 6;
  for (let L = 0; L < 4; L++) {
    const np = 6 + L * 2, R = 330 - L * 62;
    for (let j = 0; j < np; j++) {
      const th = (j / np) * TAU + L * 0.45, c = Math.cos(th), sn = Math.sin(th);
      const base = pos.length / 3;
      for (let a = 0; a <= U; a++) for (let b = 0; b <= V; b++) {
        const u = a / U, v = b / V - 0.5;
        const elev = 0.18 + L * 0.32 + u * u * (0.5 + L * 0.15);
        const w = Math.pow(Math.sin(Math.PI * Math.min(1, u * 1.02 + 0.001)), 0.7) * v * R * 0.62;
        const rr = u * R + 20;
        const radial = rr * Math.cos(elev), axial = rr * Math.sin(elev) - v * v * 30 * u;
        put(c * radial - sn * w, sn * radial + c * w, axial);
      }
      for (let a = 0; a < U; a++) for (let b = 0; b < V; b++) {
        const i0 = base + a * (V + 1) + b;
        idx.push(i0, i0 + V + 1, i0 + 1, i0 + 1, i0 + V + 1, i0 + V + 2);
      }
    }
  }
  const parts = [];
  const half = Math.round(n * 0.42);
  return [
    () => parts.push(splats.fromMesh(pos, idx, { count: half, seed: 3 })),
    () => parts.push(splats.fromMesh(pos, idx, { count: half, seed: 4 })),
    () => {
  // centre: a squashed sphere, then tipped like the petals
  const centre = splats.fromSDF((x, y, z) => (Math.hypot(x / 62, y / 62, (z - 26) / 36) - 1) * 36, { count: Math.round(n * 0.11), bounds: 100, seed: 5 });
  const cd = centre.data;
  for (let k = 0; k < cd.length; k += STRIDE) {
    const y = cd[k + 1], z = cd[k + 2];
    cd[k + 1] = y * ct - z * st + lift; cd[k + 2] = y * st + z * ct;
  }
  const r = rng(13), stemN = n - parts[0].count - parts[1].count - centre.count, stem = new Float32Array(Math.max(0, stemN) * 3);
  for (let i = 0; i < stemN; i++) {
    const t = r();
    stem[i * 3] = Math.sin(t * 2.2) * 40 + gauss(r) * 5;
    stem[i * 3 + 1] = -t * 420 + lift - 30;
    stem[i * 3 + 2] = gauss(r) * 5 - 40;
  }
  parts.push(centre, splats.fromPoints(stem));
    },
    () => done(positionsOf(splats.merge(...parts), n)),
  ];
}

/** A flock: birds along a ribbon, wing phases spread with staggerValues(). */
export function flock(n) {
  const birds = 72, o = new Float32Array(n * 3), r = rng(23);
  const phase = staggerValues(birds, 1 / birds, { grid: [12, 6], from: 'center' });
  const cx = new Float32Array(birds), cy = new Float32Array(birds), cz = new Float32Array(birds), sc = new Float32Array(birds);
  for (let b = 0; b < birds; b++) {
    const t = b / (birds - 1) + (r() - 0.5) * 0.04;
    cx[b] = (t - 0.5) * 1150 + gauss(r) * 30;
    cy[b] = Math.sin(t * 5.2 + 0.6) * 150 + 30 + gauss(r) * 40;
    cz[b] = Math.cos(t * 3.4) * 260 - 60 + gauss(r) * 50;
    sc[b] = 0.75 + r() * 0.5;
  }
  for (let i = 0; i < n; i++) {
    const b = i % birds;
    const side = r() < 0.5 ? -1 : 1;
    const u = Math.pow(r(), 0.85); // 0 body → 1 wingtip
    const flap = Math.sin(phase[b] * TAU * 2.2) * 0.9;
    const span = 46 * sc[b];
    const body = r() < 0.16;
    let x, y, z;
    if (body) {
      x = gauss(r) * 3; y = gauss(r) * 3; z = (r() - 0.6) * 26 * sc[b];
    } else {
      const sweep = u * u * 14 * sc[b];
      x = side * u * span;
      y = Math.sin(flap) * u * span * 0.7 + gauss(r) * 1.5;
      z = -sweep + (r() - 0.5) * 12 * (1 - u * 0.7) * sc[b];
    }
    // birds head along +x of the ribbon: rotate bird so wings span z/y
    o[i * 3] = cx[b] + z;
    o[i * 3 + 1] = cy[b] + y;
    o[i * 3 + 2] = cz[b] + x;
  }
  return o;
}

/** A sheet of water: the timeline drives its swell in the shader. */
export function wave(n) {
  const o = new Float32Array(n * 3), r = rng(31);
  for (let i = 0; i < n; i++) {
    const x = (r() - 0.5) * 1300, z = (r() - 0.5) * 800;
    const fall = 1 - Math.min(1, Math.hypot(x / 650, z / 400));
    o[i * 3] = x;
    o[i * 3 + 1] = Math.sin(x * 0.009 + z * 0.004) * 40 * fall + gauss(r) * 2 - 40;
    o[i * 3 + 2] = z;
  }
  return o;
}

/** A barred spiral galaxy in the model's xz plane. */
export function galaxy(n, o = new Float32Array(n * 3), from = 0, to = n) {
  const r = rng(41 + from);
  const arms = 3, R = 560;
  for (let i = from; i < to; i++) {
    let x, y, z;
    if (r() < 0.16) {
      const a = r() * TAU, rr = Math.abs(gauss(r)) * 55;
      x = Math.cos(a) * rr * 1.25; z = Math.sin(a) * rr; y = gauss(r) * 22 * Math.exp(-rr / 80);
    } else {
      const t = Math.pow(r(), 0.7);
      const rr = 50 + t * R;
      const arm = Math.floor(r() * arms);
      const a = (arm / arms) * TAU + Math.log(rr / 50) * 2.1 + gauss(r) * (0.16 + 0.12 * t);
      x = Math.cos(a) * rr + gauss(r) * 9;
      z = Math.sin(a) * rr + gauss(r) * 9;
      y = gauss(r) * 9 * (1 - t * 0.7);
    }
    o[i * 3] = x; o[i * 3 + 1] = y; o[i * 3 + 2] = z;
  }
  return o;
}
