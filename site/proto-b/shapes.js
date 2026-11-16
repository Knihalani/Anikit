// Target shapes for the sculpture. Each returns a Float32Array [x, y, z] * n in
// model space (y up, about ±550 wide). Deterministic, allocation once.
import { textPoints, staggerValues } from '../../packages/anikit/dist/anikit.lean.js';

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
    const u = r() * 2 - 1, a = r() * TAU, rad = 700 + r() * 500;
    const s = Math.sqrt(1 - u * u);
    o[i * 3] = Math.cos(a) * s * rad * 1.4;
    o[i * 3 + 1] = u * rad * 0.7;
    o[i * 3 + 2] = Math.sin(a) * s * rad - 300;
  }
  return o;
}

export function lettering(n, text, font, weight, width) {
  return textPoints(text, n, { font, weight, width, depth: 70 });
}

/** A flower with four rings of curled petals, a domed centre and a stem. */
export function flower(n) {
  const o = new Float32Array(n * 3), r = rng(11);
  const tilt = -0.42, ct = Math.cos(tilt), st = Math.sin(tilt);
  const lift = 60;
  for (let i = 0; i < n; i++) {
    let x, y, z;
    const k = r();
    if (k < 0.1) {
      // centre dome
      const a = r() * TAU, rr = Math.sqrt(r()) * 62;
      x = Math.cos(a) * rr; y = Math.sin(a) * rr; z = 30 + Math.sqrt(Math.max(0, 62 * 62 - rr * rr)) * 0.55 + gauss(r) * 3;
    } else if (k < 0.15) {
      // stem: a gentle curve down to the floor (applied after tilt below)
      const t = r();
      const sx = Math.sin(t * 2.2) * 40 + gauss(r) * 5, sz = gauss(r) * 5;
      const sy = -t * 420;
      o[i * 3] = sx; o[i * 3 + 1] = sy + lift - 30; o[i * 3 + 2] = sz - 40;
      continue;
    } else {
      const L = Math.min(3, Math.floor(r() * 4.6));
      const np = 6 + L * 2;
      const j = Math.floor(r() * np);
      const th = (j / np) * TAU + L * 0.45;
      const R = 330 - L * 62;
      const u = Math.pow(r(), 0.8), v = r() - 0.5;
      const elev = 0.18 + L * 0.32 + u * u * (0.5 + L * 0.15);
      const w = Math.pow(Math.sin(Math.PI * Math.min(1, u * 1.05)), 0.7) * v * R * 0.62;
      const rr = u * R + 20;
      const radial = rr * Math.cos(elev);
      const axial = rr * Math.sin(elev) - (v * v) * 30 * u; // petals cup slightly
      const c = Math.cos(th), s = Math.sin(th);
      x = c * radial - s * w;
      y = s * radial + c * w;
      z = axial + gauss(r) * 2.5;
    }
    // face the camera, tipped up
    o[i * 3] = x;
    o[i * 3 + 1] = y * ct - z * st + lift;
    o[i * 3 + 2] = y * st + z * ct;
  }
  return o;
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
export function galaxy(n) {
  const o = new Float32Array(n * 3), r = rng(41);
  const arms = 3, R = 560;
  for (let i = 0; i < n; i++) {
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
