// Procedural headphones built with anikit's modelling tools: lathe() for the
// turned parts, capsule() for the sliders, tube() for the band, and an SDF
// (smoothUnion of a half torus, a stem and two pivots) meshed by sdfMesh()
// for each yoke. Every part is returned centred on its own centroid so it can
// spin about itself when the headphones come apart.
import { lathe, capsule, tube, sdfMesh, sdTorus, sdSphere, smoothUnion, smoothNormals } from '../../packages/anikit/dist/anikit.lean.js';

export const CUP_Y = -4.2;
const CUP_X = 6.6, ARM_X = 7.55, YOKE_R = 4.75;

const arc = (n, f) => Array.from({ length: n + 1 }, (_, k) => f(k / n));
const quarter = (n, f) => arc(n, (t) => f((t * Math.PI) / 2));

/** Map a lathe's Y axis onto ±X (ear cups face sideways), then move it to the cup. */
function toSide(g, s) {
  const P = g.positions, N = g.normals;
  for (let k = 0; k < P.length; k += 3) {
    const x = P[k], y = P[k + 1];
    P[k] = y * s + s * CUP_X; P[k + 1] = -x * s + CUP_Y;
    const nx = N[k], ny = N[k + 1];
    N[k] = ny * s; N[k + 1] = -nx * s;
  }
  if (s < 0) flip(g);
  return g;
}
function flip(g) {
  // Rotation keeps winding; nothing to do. Kept for clarity when mirroring.
  return g;
}
/** Squash a tube's section across Z (a flat band from a round tube), fixing normals. */
function squashZ(g, k) {
  const P = g.positions, N = g.normals;
  for (let i = 0; i < P.length; i += 3) {
    P[i + 2] *= k;
    N[i + 2] /= k;
    const l = Math.hypot(N[i], N[i + 1], N[i + 2]) || 1;
    N[i] /= l; N[i + 1] /= l; N[i + 2] /= l;
  }
  return g;
}
function translate(g, x, y, z) {
  const P = g.positions;
  for (let i = 0; i < P.length; i += 3) { P[i] += x; P[i + 1] += y; P[i + 2] += z; }
  return g;
}
function centroid(g) {
  const P = g.positions, n = P.length / 3;
  let x = 0, y = 0, z = 0;
  for (let i = 0; i < P.length; i += 3) { x += P[i]; y += P[i + 1]; z += P[i + 2]; }
  return [x / n, y / n, z / n];
}

/** Profiles (r, h), counter-clockwise, h along the cup axis (outwards). */
const shellProfile = (q) => [[0, 0], [3.0, 0], ...quarter(q, (a) => [3.9 + 0.35 * Math.sin(a), 0.02 + 0.48 * (1 - Math.cos(a))]),
  [4.28, 1.5], ...quarter(q, (a) => [4.28 - 0.75 * (1 - Math.cos(a)), 1.5 + 0.7 * Math.sin(a)]), [3.35, 2.25], [3.3, 2.15], [0, 2.15]];
const glassProfile = (q) => [[0, 2.1], [3.24, 2.1], ...quarter(q, (a) => [3.22 * Math.cos(a), 2.3 + 0.32 * Math.sin(a)])];
const cushionProfile = (q) => arc(q, (t) => {
  const a = t * Math.PI * 2, sn = Math.sin(a);
  return [3.05 + 1.05 * Math.cos(a), -0.62 + 0.72 * sn * (sn > 0 ? 0.85 : 1)];
});

/** The yoke as a signed distance field: a half ring around the cup, a stem, two pivot bosses. */
function yokeSdf(s) {
  const cy = CUP_Y;
  return (x, y, z) => {
    // Local frame: the ring lies in the YZ plane at x = s * ARM_X.
    const lx = x - s * ARM_X, ly = y - cy;
    let ring = sdTorus(ly, lx, z, YOKE_R, 0.3);
    ring = Math.max(ring, -ly - 0.05); // keep the upper half
    const stemD = Math.hypot(lx, z) - 0.27, stemCap = Math.max(stemD, Math.abs(ly - (YOKE_R + 0.9)) - 0.95);
    const pivots = Math.min(sdSphere(lx, ly, z - YOKE_R, 0.5), sdSphere(lx, ly, z + YOKE_R, 0.5));
    return smoothUnion(smoothUnion(ring, stemCap, 0.35), pivots, 0.3);
  };
}

/**
 * Build the parts. `detail` 1 for the hero, lower for the 300 copies.
 * Each part: { name, group, geometry, centre, explode, spin: [axis 0|1|2, degrees], mat }.
 */
export function buildHeadphones(detail = 1) {
  const seg = Math.max(18, Math.round(72 * detail));
  const q = Math.max(2, Math.round(7 * detail));
  const parts = [];
  const add = (p) => {
    const c = centroid(p.geometry);
    translate(p.geometry, -c[0], -c[1], -c[2]);
    p.centre = c;
    parts.push(p);
  };

  const band = tube((t) => { const a = Math.PI * (0.02 + 0.96 * t); return [-ARM_X * Math.cos(a), 4.6 + 6.6 * Math.sin(a), 0]; }, 0.32, Math.round(120 * detail), Math.max(10, Math.round(28 * detail)));
  add({ name: 'Headband', group: 'band', geometry: squashZ(band, 3.2), explode: [0, 5.5, 0], spin: [0, -18], mat: 'chrome' });

  const pad = tube((t) => { const a = Math.PI * (0.22 + 0.56 * t); return [-7.0 * Math.cos(a), 4.6 + 5.85 * Math.sin(a), 0]; }, 0.42, Math.round(80 * detail), Math.max(10, Math.round(24 * detail)));
  add({ name: 'Band cushion', group: 'bandpad', geometry: squashZ(pad, 2.2), explode: [0, 1.2, 6.5], spin: [0, 24], mat: 'rubberSand' });

  for (const s of [-1, 1]) {
    add({ name: 'Slider', group: 'slider', geometry: translate(capsule(0.42, 4.6, Math.max(12, Math.round(28 * detail))), s * ARM_X, 3.5, 0), explode: [s * 3.2, 2.6, -1.5], spin: [2, s * 14], mat: 'brushed' });
    const res = Math.max(20, Math.round(104 * detail));
    const yoke = sdfMesh((x, y, z) => yokeSdf(s)(x + s * ARM_X, y + CUP_Y + 2.3, z), { resolution: res, bounds: 5.7 });
    add({ name: 'Yoke', group: 'yoke', geometry: translate(yoke, s * ARM_X, CUP_Y + 2.3, 0), explode: [s * 5.2, 0.6, 0], spin: [1, s * 20], mat: 'chrome' });
    add({ name: 'Housing', group: 'cup', geometry: toSide(lathe(shellProfile(q), seg), s), explode: [s * 9.5, -0.6, 0.6], spin: [2, s * -8], mat: 'brushedMist' });
    add({ name: 'Glass cap', group: 'glass', geometry: toSide(lathe(glassProfile(q + 3), seg), s), explode: [s * 14.5, 0.3, 1.0], spin: [1, s * 26], mat: 'glass' });
    add({ name: 'Ear cushion', group: 'pad', geometry: toSide(smoothNormals(lathe(cushionProfile(Math.max(12, Math.round(28 * detail))), seg)), s), explode: [s * 1.0, -3.4, 6.5], spin: [1, s * -38], mat: 'rubberSand' });
  }
  return parts;
}
