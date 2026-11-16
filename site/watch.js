// The watch: a mechanical wristwatch built procedurally (case, bezel, crystal, dial, hands,
// strap and a movement with real meshing teeth turning at true ratios from a 4 Hz
// escapement), rendered by anikit's CPU rasteriser. No WebGL. Ported from film/film2.mjs
// and simplified for real time. Nothing is allocated per frame.
//
//   const w = await createWatch({ width, height, canvas });
//   w.render({ t, clock, explode, dive, yaw, pitch, rest });   // see `render` below
import { ready, scene3d, renderer3d, mesh, instances, texture } from '../packages/anikit/dist/anikit.lean.js';

const PI = Math.PI, TAU = PI * 2, DEG = PI / 180;
const clamp = (x, a = 0, b = 1) => (x < a ? a : x > b ? b : x);
const smooth = (a, b, x) => { const t = clamp((x - a) / (b - a)); return t * t * (3 - 2 * t); };
const ease = (t) => (t < 0.5 ? 4 * t * t * t : 1 - (-2 * t + 2) ** 3 / 2);
const lerp = (a, b, k) => a + (b - a) * k;

// ---- Affine 3×4 matrices (column-major, like the instance buffer), from a per-frame pool ----------
const POOL = Array.from({ length: 2048 }, () => new Float64Array(12));
let pi = 0;
const nm = () => POOL[pi++ & 2047];
const M = (r00, r10, r20, r01, r11, r21, r02, r12, r22, x = 0, y = 0, z = 0) => { const o = nm(); o[0] = r00; o[1] = r10; o[2] = r20; o[3] = r01; o[4] = r11; o[5] = r21; o[6] = r02; o[7] = r12; o[8] = r22; o[9] = x; o[10] = y; o[11] = z; return o; };
const T = (x, y, z) => M(1, 0, 0, 0, 1, 0, 0, 0, 1, x, y, z);
const S = (x, y = x, z = x) => M(x, 0, 0, 0, y, 0, 0, 0, z);
const Rx = (a) => { const c = Math.cos(a), s = Math.sin(a); return M(1, 0, 0, 0, c, s, 0, -s, c); };
const spin = (th) => { const c = Math.cos(th), s = Math.sin(th); return M(c, 0, s, 0, 1, 0, -s, 0, c); };
const Rz = (a) => { const c = Math.cos(a), s = Math.sin(a); return M(c, s, 0, -s, c, 0, 0, 0, 1); };
const mul = (A, B) => {
  const o = nm();
  for (let c = 0; c < 3; c++) for (let r = 0; r < 3; r++) o[c * 3 + r] = A[r] * B[c * 3] + A[3 + r] * B[c * 3 + 1] + A[6 + r] * B[c * 3 + 2];
  for (let r = 0; r < 3; r++) o[9 + r] = A[r] * B[9] + A[3 + r] * B[10] + A[6 + r] * B[11] + A[9 + r];
  return o;
};
const ch = (a, b, c, d, e) => { let m = mul(a, b); if (c) m = mul(m, c); if (d) m = mul(m, d); if (e) m = mul(m, e); return m; };
const put = (inst, i, m, col) => { const d = inst.data, b = i * 15; for (let k = 0; k < 12; k++) d[b + k] = m[k]; d[b + 12] = col[0]; d[b + 13] = col[1]; d[b + 14] = col[2]; };
const xf = (m, x, y, z, out) => { out[0] = m[0] * x + m[3] * y + m[6] * z + m[9]; out[1] = m[1] * x + m[4] * y + m[7] * z + m[10]; out[2] = m[2] * x + m[5] * y + m[8] * z + m[11]; return out; };

// ---- Mesh builder ----------------------------------------------------------------------------------
class MB {
  constructor() { this.P = []; this.N = []; this.U = []; this.I = []; }
  v(x, y, z, nx, ny, nz, u = 0, w = 0) { this.P.push(x, y, z); const l = Math.hypot(nx, ny, nz) || 1; this.N.push(nx / l, ny / l, nz / l); this.U.push(u, w); return this.P.length / 3 - 1; }
  tri(a, b, c, n) {
    const P = this.P;
    const ux = P[b * 3] - P[a * 3], uy = P[b * 3 + 1] - P[a * 3 + 1], uz = P[b * 3 + 2] - P[a * 3 + 2];
    const vx = P[c * 3] - P[a * 3], vy = P[c * 3 + 1] - P[a * 3 + 1], vz = P[c * 3 + 2] - P[a * 3 + 2];
    const cx = uy * vz - uz * vy, cy = uz * vx - ux * vz, cz = ux * vy - uy * vx;
    const N = this.N, nn = n ?? [N[a * 3] + N[b * 3] + N[c * 3], N[a * 3 + 1] + N[b * 3 + 1] + N[c * 3 + 1], N[a * 3 + 2] + N[b * 3 + 2] + N[c * 3 + 2]];
    if (cx * nn[0] + cy * nn[1] + cz * nn[2] >= 0) this.I.push(a, b, c); else this.I.push(a, c, b);
  }
  quad(a, b, c, d, n) { this.tri(a, b, c, n); this.tri(a, c, d, n); }
  wall(x0, z0, x1, z1, y0, y1) {
    let nx = z1 - z0, nz = -(x1 - x0);
    if (nx * (x0 + x1) + nz * (z0 + z1) < 0) { nx = -nx; nz = -nz; }
    const a = this.v(x0, y0, z0, nx, 0, nz), b = this.v(x1, y0, z1, nx, 0, nz), c = this.v(x1, y1, z1, nx, 0, nz), d = this.v(x0, y1, z0, nx, 0, nz);
    this.quad(a, b, c, d, [nx, 0, nz]);
  }
  box(cx, cz, ang, len, wid, y0, y1) {
    const ux = Math.cos(ang), uz = Math.sin(ang), wx = -uz, wz = ux;
    const p = [[-len / 2, -wid / 2], [len / 2, -wid / 2], [len / 2, wid / 2], [-len / 2, wid / 2]].map(([a, b]) => [cx + ux * a + wx * b, cz + uz * a + wz * b]);
    for (const y of [y0, y1]) { const s = y === y1 ? 1 : -1; const q = p.map(([x, z]) => this.v(x, y, z, 0, s, 0, x / 300 + 0.5, z / 300 + 0.5)); this.quad(q[0], q[1], q[2], q[3], [0, s, 0]); }
    for (let k = 0; k < 4; k++) { const [x0, z0] = p[k], [x1, z1] = p[(k + 1) % 4]; let nx = z1 - z0, nz = -(x1 - x0); if (nx * (x0 - cx) + nz * (z0 - cz) < 0) { nx = -nx; nz = -nz; } const a = this.v(x0, y0, z0, nx, 0, nz), b = this.v(x1, y0, z1, nx, 0, nz), c = this.v(x1, y1, z1, nx, 0, nz), d = this.v(x0, y1, z0, nx, 0, nz); this.quad(a, b, c, d, [nx, 0, nz]); }
  }
  build() { return mesh.custom(new Float32Array(this.P), new Float32Array(this.N), new Uint32Array(this.I), new Float32Array(this.U)); }
}
const disc = (b, cx, cz, r, y0, y1, K = 32) => {
  for (const [y, s] of [[y1, 1], [y0, -1]]) { const c0 = b.v(cx, y, cz, 0, s, 0); for (let i = 0; i < K; i++) { const a0 = (i / K) * TAU, a1 = ((i + 1) / K) * TAU; b.tri(c0, b.v(cx + Math.cos(a0) * r, y, cz + Math.sin(a0) * r, 0, s, 0), b.v(cx + Math.cos(a1) * r, y, cz + Math.sin(a1) * r, 0, s, 0), [0, s, 0]); } }
  for (let i = 0; i < K; i++) { const a0 = (i / K) * TAU, a1 = ((i + 1) / K) * TAU; const p = [[a0, y0], [a1, y0], [a1, y1], [a0, y1]].map(([a, y]) => b.v(cx + Math.cos(a) * r, y, cz + Math.sin(a) * r, Math.cos(a), 0, Math.sin(a))); b.quad(p[0], p[1], p[2], p[3]); }
};
// A wheel: a toothed outline extruded from y0 to y1, a rim down to rIn, spokes and a hub.
const gear = ({ n, rp, add, ded, rIn = 0, spokes = 0, spokeW = 3, hubR = 0, y0 = 0, y1 = 3, kind = 'wheel', per = 8, tw = 0.5 }) => {
  const b = new MB(), rT = rp + add, rR = rp - ded, out = [];
  if (n === 0) for (let i = 0; i < 96; i++) out.push([(i / 96) * TAU, rp]);
  else for (let j = 0; j < n; j++) for (let s = 0; s < per; s++) {
    const u = s / per, a = ((j + u - tw / 2) / n) * TAU;
    let r;
    if (u < tw) {
      const v = u / tw;
      if (kind === 'escape') r = rR + (rT - rR) * (v < 0.8 ? (v / 0.8) ** 0.6 : (1 - v) / 0.2);
      else r = rR + (rT - rR) * Math.min(1, 1.08 * Math.sqrt(Math.sin(PI * v)));
    } else r = rR - 0.12 * (rp - rR) * Math.sin((PI * (u - tw)) / (1 - tw));
    out.push([a, r]);
  }
  const L = out.length;
  const ring = (y, s) => {
    const o = [], q = [];
    for (const [a, r] of out) {
      const x = Math.cos(a) * r, z = Math.sin(a) * r;
      o.push(b.v(x, y, z, 0, s, 0, x / (2 * rT) + 0.5, z / (2 * rT) + 0.5));
      const xi = Math.cos(a) * rIn, zi = Math.sin(a) * rIn;
      q.push(b.v(xi, y, zi, 0, s, 0));
    }
    for (let i = 0; i < L; i++) { const k = (i + 1) % L; if (rIn > 0) b.quad(o[i], o[k], q[k], q[i], [0, s, 0]); else b.tri(o[i], o[k], q[i], [0, s, 0]); }
  };
  ring(y1, 1); ring(y0, -1);
  for (let i = 0; i < L; i++) { const [a0, r0] = out[i], [a1, r1] = out[(i + 1) % L]; b.wall(Math.cos(a0) * r0, Math.sin(a0) * r0, Math.cos(a1) * r1, Math.sin(a1) * r1, y0, y1); }
  if (rIn > 0) {
    const K = 64;
    for (let i = 0; i < K; i++) {
      const a0 = (i / K) * TAU, a1 = ((i + 1) / K) * TAU;
      const p = [[a0, y0], [a1, y0], [a1, y1], [a0, y1]].map(([a, y]) => b.v(Math.cos(a) * rIn, y, Math.sin(a) * rIn, -Math.cos(a), 0, -Math.sin(a)));
      b.quad(p[0], p[1], p[2], p[3]);
    }
    for (let k = 0; k < spokes; k++) { const a = (k / spokes) * TAU + 0.3, rm = (hubR + rIn) / 2; b.box(Math.cos(a) * rm, Math.sin(a) * rm, a, rIn - hubR + 1.5, spokeW, y0 + 0.4, y1 - 0.4); }
    if (hubR > 0) disc(b, 0, 0, hubR, y0, y1 + 0.6, 24);
  }
  return b.build();
};
const polar = ({ r0, r1, a0 = 0, a1 = TAU, nr, na, y, normal, flatSeg = false, yAt, uvR = r1, walls = true, y0 }) => {
  const b = new MB(), full = Math.abs(a1 - a0 - TAU) < 1e-6;
  const hy = (x, z) => (yAt ? yAt(x, z) : y);
  if (!flatSeg) {
    const idx = [];
    for (let i = 0; i <= nr; i++) for (let j = 0; j <= na; j++) {
      const r = lerp(r0, r1, i / nr), a = lerp(a0, a1, j / na), x = Math.cos(a) * r, z = Math.sin(a) * r;
      const n = normal(x, z, a, r, j); idx.push(b.v(x, hy(x, z), z, n[0], n[1], n[2], x / (2 * uvR) + 0.5, z / (2 * uvR) + 0.5));
    }
    for (let i = 0; i < nr; i++) for (let j = 0; j < na; j++) { const p = i * (na + 1) + j; b.quad(idx[p], idx[p + 1], idx[p + na + 2], idx[p + na + 1], [0, 1, 0]); }
  } else {
    for (let j = 0; j < na; j++) {
      const aa = lerp(a0, a1, j / na), ab = lerp(a0, a1, (j + 1) / na), n = normal(0, 0, (aa + ab) / 2, 0, j);
      const pts = [[r0, aa], [r0, ab], [r1, ab], [r1, aa]].map(([r, a]) => { const x = Math.cos(a) * r, z = Math.sin(a) * r; return b.v(x, hy(x, z), z, n[0], n[1], n[2], x / (2 * uvR) + 0.5, z / (2 * uvR) + 0.5); });
      if (r0 > 0) b.quad(pts[0], pts[1], pts[2], pts[3], [0, 1, 0]); else b.tri(pts[0], pts[2], pts[3], [0, 1, 0]);
    }
  }
  if (walls && y0 !== undefined) {
    const K = Math.max(8, Math.round(na / 3));
    for (const r of r0 > 0 ? [r0, r1] : [r1]) for (let j = 0; j < K; j++) {
      const aa = lerp(a0, a1, j / K), ab = lerp(a0, a1, (j + 1) / K), s = r === r1 ? 1 : -1;
      const p = [[aa, y0], [ab, y0], [ab, null], [aa, null]].map(([a, yy]) => { const x = Math.cos(a) * r, z = Math.sin(a) * r; return b.v(x, yy ?? hy(x, z), z, s * Math.cos(a), 0, s * Math.sin(a)); });
      b.quad(p[0], p[1], p[2], p[3]);
    }
    if (!full) for (const a of [a0, a1]) {
      const s = a === a0 ? -1 : 1, nx = -Math.sin(a) * s, nz = Math.cos(a) * s;
      const p = [[r0, y0], [r1, y0], [r1, null], [r0, null]].map(([r, yy]) => { const x = Math.cos(a) * r, z = Math.sin(a) * r; return b.v(x, yy ?? hy(x, z), z, nx, 0, nz); });
      b.quad(p[0], p[1], p[2], p[3]);
    }
  }
  return b.build();
};
const lathe = (prof, K = 96) => {
  const b = new MB();
  for (let k = 0; k < prof.length - 1; k++) {
    const segN = (p, q) => { const dr = q[0] - p[0], dy = q[1] - p[1]; const l = Math.hypot(dr, dy); return [-dy / l, dr / l]; };
    const [ra, ya] = prof[k], [rb, yb] = prof[k + 1];
    const nAB = segN(prof[k], prof[k + 1]);
    const nAt = (i) => {
      if (prof[i][2] || i === 0 || i === prof.length - 1) return nAB;
      const o = i === k ? segN(prof[k - 1], prof[k]) : segN(prof[k + 1], prof[k + 2] ?? prof[k + 1]);
      if (i === k + 1 && !prof[k + 2]) return nAB;
      const s = [nAB[0] + o[0], nAB[1] + o[1]]; const l = Math.hypot(s[0], s[1]); return [s[0] / l, s[1] / l];
    };
    const na = nAt(k), nb = nAt(k + 1);
    for (let i = 0; i < K; i++) {
      const a0 = (i / K) * TAU, a1 = ((i + 1) / K) * TAU;
      const vv = (r, y, n, a) => b.v(Math.cos(a) * r, y, Math.sin(a) * r, Math.cos(a) * n[0], n[1], Math.sin(a) * n[0], i / K, k / prof.length);
      const p0 = vv(ra, ya, na, a0), p1 = vv(ra, ya, na, a1), p2 = vv(rb, yb, nb, a1), p3 = vv(rb, yb, nb, a0);
      const am = (a0 + a1) / 2;
      b.quad(p0, p1, p2, p3, [Math.cos(am) * nAB[0], nAB[1], Math.sin(am) * nAB[0]]);
    }
  }
  return b.build();
};

// ---- Textures (OffscreenCanvas works on the main thread and in workers) ---------------------------
const canvas2d = (w, h) => { const c = typeof OffscreenCanvas !== 'undefined' ? new OffscreenCanvas(w, h) : Object.assign(document.createElement('canvas'), { width: w, height: h }); return [c, c.getContext('2d')]; };
function makeTextures() {
  let seed = 11;
  const rnd = () => ((seed = (seed * 1664525 + 1013904223) >>> 0) / 4294967296);
  const perlage = (() => {
    const [c, g] = canvas2d(512, 512);
    g.fillStyle = 'rgb(150,150,152)'; g.fillRect(0, 0, 512, 512);
    const sp = 16;
    for (let row = -1; row < 512 / sp + 2; row++) for (let col = -1; col < 512 / sp + 2; col++) {
      const x = col * sp + (row % 2 ? sp / 2 : 0) + (rnd() - 0.5), y = row * sp * 0.87 + (rnd() - 0.5);
      const gr = g.createRadialGradient(x - 3, y - 4, 0.5, x, y, 12);
      gr.addColorStop(0, 'rgb(236,236,238)'); gr.addColorStop(0.35, 'rgb(196,196,199)'); gr.addColorStop(0.75, 'rgb(150,150,153)'); gr.addColorStop(1, 'rgb(118,118,121)');
      g.fillStyle = gr; g.beginPath(); g.arc(x, y, 11, 0, TAU); g.fill();
    }
    return texture(g.getImageData(0, 0, 512, 512));
  })();
  const P = 1024, R = P / 2;
  const dial = (() => {
    const [c, g] = canvas2d(P, P);
    const gr = g.createRadialGradient(R, R, 0, R, R, R);
    gr.addColorStop(0, 'rgb(38,66,132)'); gr.addColorStop(0.55, 'rgb(22,42,98)'); gr.addColorStop(0.9, 'rgb(10,20,50)'); gr.addColorStop(1, 'rgb(6,12,30)');
    g.fillStyle = gr; g.fillRect(0, 0, P, P);
    g.lineWidth = 1;
    for (let i = 0; i < 1800; i++) {
      const a = rnd() * TAU, k = rnd();
      g.strokeStyle = k < 0.5 ? `rgba(255,255,255,${0.014 * rnd()})` : `rgba(0,0,0,${0.024 * rnd()})`;
      g.beginPath(); g.moveTo(R + Math.cos(a) * 10, R + Math.sin(a) * 10); g.lineTo(R + Math.cos(a) * R, R + Math.sin(a) * R); g.stroke();
    }
    g.strokeStyle = 'rgba(232,236,242,0.92)'; g.lineWidth = 1.6;
    for (const rr of [0.905, 0.955]) { g.beginPath(); g.arc(R, R, R * rr, 0, TAU); g.stroke(); }
    for (let m = 0; m < 60; m++) {
      const a = (m / 60) * TAU, five = m % 5 === 0;
      g.lineWidth = five ? 3.6 : 1.8;
      g.beginPath(); g.moveTo(R + Math.cos(a) * R * (five ? 0.895 : 0.905), R + Math.sin(a) * R * (five ? 0.895 : 0.905)); g.lineTo(R + Math.cos(a) * R * 0.955, R + Math.sin(a) * R * 0.955); g.stroke();
    }
    g.fillStyle = 'rgba(220,226,236,0.7)';
    for (let m = 0; m < 60; m++) { const a = (m / 60) * TAU; g.beginPath(); g.arc(R + Math.cos(a) * R * 0.62, R + Math.sin(a) * R * 0.62, m % 5 ? 1.1 : 2, 0, TAU); g.fill(); }
    // Name and caption. The dial's u runs along +x, v along +z; 12 o'clock is −z (top of the image).
    g.fillStyle = 'rgba(236,238,244,0.94)'; g.textAlign = 'center'; g.textBaseline = 'middle';
    g.font = '600 38px Helvetica, Arial, sans-serif';
    if ('letterSpacing' in g) g.letterSpacing = '7px';
    g.fillText('ANIKIT', R, R * 0.6);
    g.font = '400 17px Helvetica, Arial, sans-serif';
    if ('letterSpacing' in g) g.letterSpacing = '5px';
    g.fillStyle = 'rgba(214,220,232,0.8)';
    g.fillText('AUTOMATIC', R, R * 0.7);
    g.fillText('28 800 A/h', R, R * 1.42);
    return texture(g.getImageData(0, 0, P, P));
  })();
  return { perlage, dial };
}

// ---- Geometry (built once) -------------------------------------------------------------------------
function build() {
  const at = (p, d, a) => [p[0] + d * Math.cos(a * DEG), p[1] + d * Math.sin(a * DEG)];
  const ang = (p, q) => Math.atan2(q[1] - p[1], q[0] - p[0]);
  const C0 = [0, 0], CW = { n: 80, m: 1.0 }, P3 = { n: 10 }, TW = { n: 75, m: 0.9 }, P4 = { n: 10 }, FW = { n: 112, m: 0.75 }, PE = { n: 7 }, EW = { n: 15 };
  const T3 = at(C0, CW.m * (CW.n + P3.n) / 2, -20);
  const F4 = at(T3, TW.m * (TW.n + P4.n) / 2, -30);
  const E5 = at(F4, FW.m * (FW.n + PE.n) / 2, -100);
  const PAL = at(E5, 30, 200);
  const BAL = at(E5, 70, 200);
  const psiC3 = ang(C0, T3), psi34 = ang(T3, F4), psi4E = ang(F4, E5);
  const PH = { cw: psiC3, p3: psiC3 + PI - PI / P3.n, tw: psi34, p4: psi34 + PI - PI / P4.n, fw: psi4E, pe: psi4E + PI - PI / PE.n, ew: 0.1 };
  const Y = { cw: [1, 3.5], tw: [6, 8.5], fw: [10.5, 13], ew: [2.5, 5], bridge: [16, 21] };
  const wheelM = (spec, y, extra) => gear({ n: spec.n, rp: spec.m * spec.n / 2, add: spec.m * 1.25, ded: spec.m * 1.5, y0: y[0], y1: y[1], per: spec.n > 90 ? 6 : 7, tw: 0.46, ...extra });
  const pinionM = (n, m, y) => gear({ n, rp: m * n / 2, add: m * 0.75, ded: m * 1.4, rIn: 0, y0: y[0], y1: y[1], per: 10, tw: 0.42 });
  const BARREL = [-88, 4], CROWNW = [-52, 58], BW = 17;
  const cotes = (x) => { const f = ((x / BW) % 1 + 1) % 1; const k = 0.22 * Math.sin(TAU * f) + 0.06 * Math.sin(2 * TAU * f); return [-k, 1, 0]; };
  const G = {
    C0, T3, F4, E5, PAL, BAL, PH, BARREL, CROWNW, ang,
    centreWheel: wheelM(CW, Y.cw, { rIn: 30, spokes: 4, spokeW: 4.5, hubR: 6 }),
    thirdWheel: wheelM(TW, Y.tw, { rIn: 25, spokes: 4, spokeW: 3.5, hubR: 4.5 }),
    fourthWheel: wheelM(FW, Y.fw, { rIn: 34, spokes: 4, spokeW: 3, hubR: 4.5 }),
    escapeWheel: gear({ n: EW.n, rp: 20, add: 4, ded: 3.5, rIn: 13, spokes: 5, spokeW: 1.8, hubR: 3.2, y0: Y.ew[0], y1: Y.ew[1], kind: 'escape', per: 12, tw: 0.62 }),
    pinion3: pinionM(P3.n, CW.m, [0.4, 4.4]), pinion4: pinionM(P4.n, TW.m, [5.4, 9.2]), pinionE: pinionM(PE.n, FW.m, [9.8, 13.6]),
    ratchet: gear({ n: 90, rp: 40, add: 1.1, ded: 1.4, rIn: 0, y0: 27, y1: 31.5, per: 5, tw: 0.5 }),
    crownWheel: gear({ n: 40, rp: 15, add: 0.9, ded: 1.1, rIn: 0, y0: 27, y1: 30, per: 5, tw: 0.5 }),
    mainplate: polar({ r0: 0, r1: 150, nr: 1, na: 96, y: 0, y0: -18, normal: () => [0, 1, 0], uvR: 150 }),
    mainBottom: polar({ r0: 0, r1: 150, nr: 1, na: 96, y: -18.4, walls: false, normal: () => [0, -1, 0], uvR: 150 }),
    barrelBridge: polar({ r0: 56, r1: 146, a0: 108 * DEG, a1: 252 * DEG, nr: 40, na: 120, y: 26, y0: 18, normal: (x) => cotes(x), uvR: 150 }),
  };
  G.bars = (() => {
    const b = new MB(), y0 = Y.bridge[0], y1 = Y.bridge[1];
    const bar = (p, q, w) => { const a = ang(p, q), l = Math.hypot(q[0] - p[0], q[1] - p[1]); b.box((p[0] + q[0]) / 2, (p[1] + q[1]) / 2, a, l, w, y0, y1); };
    const A1 = [112, 26], A2 = [104, -100], A3 = [-57, 12];
    bar(C0, T3, 10); bar(T3, F4, 10); bar(F4, A1, 12); bar(F4, E5, 9); bar(E5, A2, 11); bar(C0, A3, 10); bar(PAL, E5, 7);
    for (const [p, r] of [[C0, 9], [T3, 7.5], [F4, 7.5], [E5, 7], [PAL, 5.5], [A1, 13], [A2, 13]]) disc(b, p[0], p[1], r, y0, y1, 24);
    return b.build();
  })();
  const COCKF = [-58, -112];
  G.cock = (() => {
    const b = new MB(), y0 = 22, y1 = 27;
    const a = ang(BAL, COCKF), l = Math.hypot(COCKF[0] - BAL[0], COCKF[1] - BAL[1]);
    b.box((BAL[0] + COCKF[0]) / 2, (BAL[1] + COCKF[1]) / 2, a, l, 13, y0, y1);
    disc(b, BAL[0], BAL[1], 10, y0, y1, 32); disc(b, COCKF[0], COCKF[1], 19, y0, y1, 32);
    disc(b, COCKF[0], COCKF[1], 13, 0, y0, 24);
    return b.build();
  })();
  G.pallet = (() => {
    const b = new MB(), y0 = 3, y1 = 4.6;
    b.box(11, 0, 0, 24, 2.4, y0, y1);
    b.box(23.5, -1.8, 0, 3.5, 1.2, y0, y1); b.box(23.5, 1.8, 0, 3.5, 1.2, y0, y1);
    b.box(-9, 0, 0, 18, 3.2, y0, y1);
    b.box(-13, 0, PI / 2, 26, 3.2, y0, y1);
    disc(b, 0, 0, 2.6, y0, y1 + 0.4, 16);
    return b.build();
  })();
  G.balanceWheel = gear({ n: 0, rp: 38, add: 0, ded: 0, rIn: 34.5, spokes: 3, spokeW: 3.2, hubR: 3.5, y0: 8.5, y1: 12 });
  G.roller = (() => { const b = new MB(); disc(b, 0, 0, 7, 6, 7.4, 24); disc(b, 0, 0, 1.2, -1, 24, 12); return b.build(); })();
  G.hair = (() => {
    const n = 520, pts = new Float32Array(n * 3);
    for (let i = 0; i < n; i++) { const u = i / (n - 1), a = u * 12.25 * TAU, r = 4.2 + 23.5 * u; pts[i * 3] = Math.cos(a) * r; pts[i * 3 + 1] = 6.5; pts[i * 3 + 2] = Math.sin(a) * r; }
    return mesh.tube(pts, { radius: 0.36, sides: 4 });
  })();
  G.rotorPlate = polar({ r0: 15, r1: 128, a0: -90 * DEG, a1: 90 * DEG, nr: 36, na: 110, y: 41, y0: 38, normal: (x, z) => { const f = (((z + 500) / BW) % 1); return [0, 1, -(0.22 * Math.sin(TAU * f) + 0.06 * Math.sin(2 * TAU * f))]; }, uvR: 150 });
  G.rotorRim = polar({ r0: 126, r1: 146, a0: -92 * DEG, a1: 92 * DEG, nr: 3, na: 96, y: 42, y0: 30, normal: (x, z, a, r) => [Math.cos(a) * (r - 136) * 0.04, 1, Math.sin(a) * (r - 136) * 0.04], uvR: 150 });
  G.rotorHub = (() => { const b = new MB(); disc(b, 0, 0, 16, 36, 44, 40); return b.build(); })();
  G.cyl = mesh.cylinder(20, true); G.cube = mesh.cube(); G.sph = mesh.sphere(8, 14);
  G.JEWELS = [[C0, Y.bridge[1]], [T3, Y.bridge[1]], [F4, Y.bridge[1]], [E5, Y.bridge[1]], [PAL, Y.bridge[1]], [BAL, 27], [BARREL, 26], [CROWNW, 30]];
  G.SCREWS = [[[112, 26], Y.bridge[1], 6.5], [[104, -100], Y.bridge[1], 6.5], [COCKF, 27, 7], [[-100, 95], 26, 6], [[-128, 40], 26, 6], [[-128, -40], 26, 6], [[-100, -95], 26, 6], [[-62, -26], 26, 5.5], [BARREL, 31.5, 9], [CROWNW, 30, 5.5], [[-57, 12], Y.bridge[1], 5]];

  // The watch (local space: caseback at y = 0, dial facing +y, 12 o'clock towards −z).
  G.caseMesh = lathe([[150, 80, 1], [153, 86, 1], [197, 87, 1], [205, 82, 1], [209, 70], [210, 52], [208, 30, 1], [200, 14, 1], [182, 6], [120, 3], [0, 2.5, 1]], 112);
  G.caseFloor = polar({ r0: 0, r1: 152, nr: 1, na: 64, y: 4, walls: false, normal: () => [0, 1, 0] });
  G.caseInner = lathe([[150, 4, 1], [150, 80, 1]], 96);
  G.bezelMesh = lathe([[151, 85, 1], [152, 93, 1], [157, 97], [167, 96.5], [175, 92, 1], [178, 87, 1]], 112);
  G.crystal = polar({ r0: 0, r1: 152, nr: 6, na: 72, y: 0, walls: false, yAt: (x, z) => 98.5 - 3 * ((x * x + z * z) / (152 * 152)), normal: (x, z) => [x * 0.0026, 1, z * 0.0026] });
  G.dialTop = polar({ r0: 3, r1: 150, nr: 1, na: 180, y: 0, flatSeg: true, uvR: 150, normal: (x, z, a, r, j) => { const s = Math.sin(j * 12.9898 + 78.233) * 43758.5453, h = (s - Math.floor(s)) - 0.5; const tl = 0.1 * h; return [-Math.sin(a) * tl, 1, Math.cos(a) * tl]; } });
  G.dialBody = (() => { const b = new MB(); disc(b, 0, 0, 149.5, -3, -1.2, 96); return b.build(); })();
  const dauphine = (L, w, tail) => {
    const b = new MB(), h = 1.6;
    const tip = [0, h, -L], base = [0, h, tail], l = [-w, 0, -L * 0.22], r = [w, 0, -L * 0.22], lt = [-w * 0.55, 0, tail], rt = [w * 0.55, 0, tail];
    const face = (p, q, s) => { const ux = q[0] - p[0], uy = q[1] - p[1], uz = q[2] - p[2], vx = s[0] - p[0], vy = s[1] - p[1], vz = s[2] - p[2]; let n = [uy * vz - uz * vy, uz * vx - ux * vz, ux * vy - uy * vx]; if (n[1] < 0) n = n.map((v) => -v); b.tri(b.v(...p, ...n), b.v(...q, ...n), b.v(...s, ...n), n); };
    face(tip, l, base); face(tip, base, r); face(l, lt, base); face(r, base, rt);
    face([lt[0], 0, tail], [rt[0], 0, tail], [0, h, tail]);
    disc(b, 0, 0, w * 0.9, -0.6, 0.9, 20);
    return b.build();
  };
  G.hourHand = dauphine(88, 7.5, 6); G.minuteHand = dauphine(132, 6.2, 6);
  G.secondHand = (() => { const b = new MB(); b.box(0, -40, PI / 2, 182, 1.5, 0, 1.0); disc(b, 0, 0, 4, -0.3, 1.6, 20); disc(b, 0, 36, 4.2, 0, 1.0, 20); return b.build(); })();
  G.crownMesh = gear({ n: 36, rp: 24, add: 0.9, ded: 1.2, rIn: 0, y0: -12, y1: 12, per: 5, tw: 0.55 });
  const lugPts = []; for (let i = 0; i <= 12; i++) { const u = i / 12; lugPts.push([0, 62 - 26 * u * u - 6 * u, 176 + 66 * u]); }
  G.lugMesh = mesh.tube(lugPts, { radius: 17, sides: 14, radii: lugPts.map((_, i) => 1 - 0.2 * (i / 12)) });
  G.lugEnd = lugPts[12];
  // The strap: a closed leather loop around a wrist-sized ellipse under the case (local y–z plane).
  G.strap = (() => {
    const b = new MB(), yc = -69, rz = 300, ry = 170, hw = 84, th = 7;
    const phi0 = Math.asin(222 / rz), K = 72;
    for (const s of [-1, 1]) {
      const pts = [];
      for (let i = 0; i <= K; i++) { const ph = lerp(phi0, PI, i / K); pts.push([yc + ry * Math.cos(ph), s * rz * Math.sin(ph), ph]); }
      // Outer and inner faces, and the two edges.
      const nrm = (ph) => { const ny = Math.cos(ph) / ry, nz = s * Math.sin(ph) / rz; const l = Math.hypot(ny, nz); return [ny / l, nz / l]; };
      for (let i = 0; i < K; i++) {
        const [y0, z0, p0] = pts[i], [y1, z1, p1] = pts[i + 1];
        const n0 = nrm(p0), n1 = nrm(p1);
        for (const side of [1, -1]) { // outer (+), inner (−)
          const o = side > 0 ? 0 : -th;
          const q = [[-hw, y0 + n0[0] * o, z0 + n0[1] * o, n0], [hw, y0 + n0[0] * o, z0 + n0[1] * o, n0], [hw, y1 + n1[0] * o, z1 + n1[1] * o, n1], [-hw, y1 + n1[0] * o, z1 + n1[1] * o, n1]]
            .map(([x, y, z, n], k) => b.v(x, y, z, 0, n[0] * side, n[1] * side, k === 0 || k === 3 ? 0 : 1, i / K));
          b.quad(q[0], q[1], q[2], q[3], [0, n0[0] * side, n0[1] * side]);
        }
        for (const ex of [-hw, hw]) {
          const q = [[y0, z0], [y1, z1], [y1 - n1[0] * th, z1 - n1[1] * th], [y0 - n0[0] * th, z0 - n0[1] * th]].map(([y, z]) => b.v(ex, y, z, Math.sign(ex), 0, 0));
          b.quad(q[0], q[1], q[2], q[3], [Math.sign(ex), 0, 0]);
        }
      }
    }
    return b.build();
  })();
  // A studio floor: a dark glossy plane (mirror).
  G.floor = mesh.plane(1);
  return G;
}

// ---- Materials and colours ---------------------------------------------------------------------------
const GOLD = [1.0, 0.73, 0.42], STEEL = [0.88, 0.89, 0.92], BLUE = [0.16, 0.28, 0.75], RUBY = [0.75, 0.035, 0.07];
function materials(tex) {
  return {
    steel: { ambient: 0.14, diffuse: 0.55, metal: 0.92, specular: 1.7, shininess: 170, rim: 0.12 },
    polish: { ambient: 0.1, diffuse: 0.4, metal: 0.97, specular: 2.2, shininess: 400, rim: 0.15 },
    gold: { ambient: 0.15, diffuse: 0.6, metal: 0.86, specular: 1.5, shininess: 120, rim: 0.1 },
    blued: { ambient: 0.1, diffuse: 0.5, metal: 0.85, specular: 1.6, shininess: 180, rim: 0.15 },
    cotes: { ambient: 0.12, diffuse: 0.55, metal: 0.78, specular: 1.3, shininess: 70, rim: 0.08 },
    perlage: { ambient: 0.12, diffuse: 0.7, metal: 0.4, specular: 0.7, shininess: 40, rim: 0.05, texture: tex.perlage, textureRepeat: [1, 1] },
    perlageB: { ambient: 0.12, diffuse: 0.95, metal: 0.12, specular: 0.4, shininess: 40, rim: 0.05, texture: tex.perlage, textureRepeat: [1, 1] },
    ruby: { ambient: 0.25, diffuse: 0.55, specular: 2.4, shininess: 420, emissive: 0.5, metal: 0.25, rim: 0.7 },
    dial: { ambient: 0.22, diffuse: 0.7, metal: 0.12, specular: 0.45, shininess: 40, rim: 0.05, texture: tex.dial },
    crystal: { ambient: 0, diffuse: 0, specular: 1.4, shininess: 300, rim: 0.08, metal: 0.14 },
    leather: { ambient: 0.25, diffuse: 0.8, specular: 0.35, shininess: 14, rim: 0.2, metal: 0 },
    floor: { ambient: 0, diffuse: 0, specular: 0, shininess: 300, rim: 0 },
  };
}

export async function createWatch({ width, height, canvas = null }) {
  await ready;
  const G = build(), mats = materials(makeTextures());
  let W = width, H = height;
  const cam = scene3d({ fov: 28, width: W, height: H, near: 4, far: 12000 });
  const r3 = renderer3d({ width: W, height: H });
  const ctx = canvas ? canvas.getContext('2d', { alpha: false }) : null;
  let img = null;
  const one = () => instances(1);
  const I = {
    plate: one(), plateB: one(), bbridge: one(), bars: one(), cock: one(), ratchet: one(), crownW: one(),
    cw: one(), tw: one(), fw: one(), ew: one(), p3: one(), p4: one(), pe: one(), pal: one(), bal: one(), roller: one(), hair: one(),
    rotor: one(), rotorRim: one(), rotorHub: one(),
    arbors: instances(5), jewels: instances(9), chatons: instances(9), screws: instances(11), slots: instances(11), balScrews: instances(12), stones: instances(3),
    case: one(), caseFloor: one(), caseInner: one(), bezel: one(), crystal: one(), dialTop: one(), dialBody: one(), indices: instances(13),
    hh: one(), mh: one(), sh: one(), crown: one(), stem: one(), lugs: instances(4), lugCaps: instances(4), strap: one(), floor: one(),
  };
  const it = (m, inst, material, extra) => ({ mesh: m, instances: inst, material, ...extra });
  const nr = { reflect: false }; // small or upward-facing parts stay out of the floor's reflection
  const movItems = [
    it(G.mainplate, I.plate, mats.perlage), it(G.mainBottom, I.plateB, mats.perlageB), it(G.barrelBridge, I.bbridge, mats.cotes),
    it(G.bars, I.bars, mats.steel, nr), it(G.cock, I.cock, mats.steel, nr), it(G.ratchet, I.ratchet, mats.steel, nr), it(G.crownWheel, I.crownW, mats.steel, nr),
    it(G.centreWheel, I.cw, mats.gold, nr), it(G.thirdWheel, I.tw, mats.gold, nr), it(G.fourthWheel, I.fw, mats.gold, nr), it(G.escapeWheel, I.ew, mats.steel, nr),
    it(G.pinion3, I.p3, mats.steel, nr), it(G.pinion4, I.p4, mats.steel, nr), it(G.pinionE, I.pe, mats.steel, nr),
    it(G.cyl, I.arbors, mats.steel, nr), it(G.pallet, I.pal, mats.steel, nr), it(G.cube, I.stones, mats.ruby, nr),
    it(G.balanceWheel, I.bal, mats.gold, nr), it(G.roller, I.roller, mats.steel, nr), it(G.cyl, I.balScrews, mats.gold, nr), it(G.hair, I.hair, { ...mats.steel, metal: 0.7 }, nr),
    it(G.rotorPlate, I.rotor, mats.cotes), it(G.rotorRim, I.rotorRim, mats.gold), it(G.rotorHub, I.rotorHub, mats.steel),
    it(G.sph, I.jewels, mats.ruby, nr), it(G.cyl, I.chatons, mats.gold, nr), it(G.cyl, I.screws, mats.blued, nr), it(G.cube, I.slots, { ambient: 0.02, diffuse: 0.05, specular: 0, rim: 0 }, nr),
  ];
  const caseItems = [
    it(G.caseMesh, I.case, mats.polish), it(G.caseFloor, I.caseFloor, mats.steel), it(G.caseInner, I.caseInner, mats.steel, { cull: false }),
    it(G.lugMesh, I.lugs, mats.polish, { cull: false }), it(G.sph, I.lugCaps, mats.polish), it(G.crownMesh, I.crown, mats.steel), it(G.cyl, I.stem, mats.steel, nr),
  ];
  const strapItem = it(G.strap, I.strap, mats.leather, { cull: false });
  const dialItems = [it(G.dialTop, I.dialTop, mats.dial, nr), it(G.dialBody, I.dialBody, mats.steel), it(G.cube, I.indices, mats.polish, nr)];
  const handItems = [it(G.hourHand, I.hh, mats.polish, nr), it(G.minuteHand, I.mh, mats.polish, nr), it(G.secondHand, I.sh, mats.polish, nr)];
  const bezelItem = it(G.bezelMesh, I.bezel, mats.polish);
  const crystalItem = it(G.crystal, I.crystal, mats.crystal, { blend: 'additive', reflect: false });
  const floorItem = it(G.floor, I.floor, mats.floor, { mirror: true, reflect: false });
  const items = [];
  const tg = [0, 0, 0], cp = [0, 0, 0], tmp = [0, 0, 0], tmp2 = [0, 0, 0];
  const H0 = 340; // the watch floats this high above the floor
  const opts = {
    sort: 'near', background: '#000000',
    key: [-0.55, 0.8, 0.5], keyColor: [1.0, 0.97, 0.92], fill: [0.7, 0.3, 0.6], fillColor: [0.08, 0.09, 0.11], rimColor: [0.55, 0.6, 0.7],
    sky: [0.5, 0.5, 0.53], horizon: [0.13, 0.13, 0.14], ground: [0.01, 0.01, 0.012], strips: [1.8, 1.76, 1.7],
  };
  const mirror = { y: 0, strength: 0.3 };

  /**
   * t: seconds (page clock, drives the escapement); clock: local time in seconds since midnight;
   * explode 0–1; dive 0–1 (camera into the movement); yaw/pitch: pointer tilt (radians); turn: extra turn.
   */
  const ROTOR_AWAY = PI + TAU; // in the dive the rotor swings over the barrel, clear of the going train
  const GLIDE = [[G.BAL[0] + 6, G.BAL[1] + 4, 10, 1], [G.PAL[0], G.PAL[1], 6, 0.8], [G.E5[0], G.E5[1], 6, 0.85], [G.F4[0] - 10, G.F4[1], 10, 1.25]];
  // Label anchors: [frame (0 watch, 1 dial, 2 movement, 3 case), x, explode offset, y, z].
  const LABELS = [[0, 160, 560, 98, 0], [0, 178, 480, 92, 0], [0, 60, 370, 82, 0], [1, 150, 0, 0, 0], [2, 150, 0, 0, 0], [3, 200, 0, 30, 90], [0, 84, -200, -160, 250]];
  function render(s) {
    pi = 0;
    const t = s.t, ex = s.explode, dv = s.dive;
    // 28,800 vph: the balance swings at 4 Hz, ±270° (eased by `slow` in the macro shot only).
    const beats = (() => { const b = t * 8, k = Math.floor(b), f = b - k; return k + (1 - (1 - clamp(f / 0.13)) ** 3); })();
    const th = 270 * DEG * Math.sin(TAU * 4 * t);
    // Pose: hero tilt, then the exploded three-quarter view.
    const e = ease(clamp(ex)), d = ease(clamp(dv));
    const tilt = lerp(lerp(56, 40, e), 18, d) * DEG + s.pitch;
    const yaw = lerp(-14, -38, e) * DEG + s.yaw + s.turn;
    const Gw = ch(T(0, H0 + lerp(0, 40, e), 0), spin(yaw), Rx(tilt));
    const gap = 1.15 * e; // explode along the watch axis (local y)
    const exY = (k) => k * gap;
    // Movement: seated dial-side up in the case; during the dive it lifts and turns its bridges to the camera.
    const flip = PI * (1 - d);
    const Gmov = ch(Gw, T(0, 53 + exY(150) + d * 120, 0), Rx(flip));
    const P = (x, z, y = 0) => T(x, y, z);
    const gearAt = (inst, p, phase, theta, col) => put(inst, 0, ch(Gmov, P(p[0], p[1]), spin(phase + theta)), col);
    const thE = -12 * DEG * beats, th4 = 0.75 * DEG * beats, th3 = -0.1 * DEG * beats, thC = 0.0125 * DEG * beats;
    put(I.plate, 0, Gmov, [0.52, 0.52, 0.54]); put(I.plateB, 0, Gmov, [0.62, 0.62, 0.64]);
    put(I.bbridge, 0, Gmov, [0.86, 0.87, 0.9]); put(I.bars, 0, Gmov, [0.84, 0.85, 0.88]); put(I.cock, 0, Gmov, [0.84, 0.85, 0.88]);
    put(I.ratchet, 0, ch(Gmov, P(G.BARREL[0], G.BARREL[1]), spin(0.02 * t)), STEEL);
    put(I.crownW, 0, ch(Gmov, P(G.CROWNW[0], G.CROWNW[1]), spin(-0.05 * t)), STEEL);
    const { C0, T3, F4, E5, PAL, BAL, PH } = G;
    gearAt(I.cw, C0, PH.cw, thC, GOLD); gearAt(I.p3, T3, PH.p3, th3, STEEL); gearAt(I.tw, T3, PH.tw, th3, GOLD);
    gearAt(I.p4, F4, PH.p4, th4, STEEL); gearAt(I.fw, F4, PH.fw, th4, GOLD); gearAt(I.pe, E5, PH.pe, thE, STEEL); gearAt(I.ew, E5, PH.ew, thE, STEEL);
    put(I.arbors, 0, ch(Gmov, P(C0[0], C0[1], 8), S(1.3, 16, 1.3)), STEEL); put(I.arbors, 1, ch(Gmov, P(T3[0], T3[1], 8), S(1.3, 16, 1.3)), STEEL);
    put(I.arbors, 2, ch(Gmov, P(F4[0], F4[1], 8), S(1.3, 16, 1.3)), STEEL); put(I.arbors, 3, ch(Gmov, P(E5[0], E5[1], 8), S(1.3, 16, 1.3)), STEEL);
    put(I.arbors, 4, ch(Gmov, P(PAL[0], PAL[1], 8), S(1.0, 16, 1.0)), STEEL);
    const kb = Math.floor(beats), fb = beats - kb, side = (kb % 2 ? 1 : -1) * (1 - 2 * smooth(0, 1, fb));
    const Gp = ch(Gmov, P(PAL[0], PAL[1]), spin(G.ang(PAL, BAL) + 7 * DEG * side));
    put(I.pal, 0, Gp, STEEL);
    put(I.stones, 0, ch(Gp, P(-14.5, -12.5, 3.8), spin(-0.5), S(2.2, 2.0, 3.6)), RUBY);
    put(I.stones, 1, ch(Gp, P(-14.5, 12.5, 3.8), spin(0.5), S(2.2, 2.0, 3.6)), RUBY);
    const Gb = ch(Gmov, P(BAL[0], BAL[1]), spin(th));
    put(I.bal, 0, Gb, GOLD); put(I.roller, 0, Gb, STEEL);
    put(I.stones, 2, ch(Gb, P(5.6, 0, 5), S(1.1, 2.4, 1.1)), RUBY);
    for (let k = 0; k < 12; k++) { const a = (k / 12) * TAU + PI / 12; put(I.balScrews, k, ch(Gb, P(Math.cos(a) * 39, Math.sin(a) * 39, 10.25), spin(a), Rz(PI / 2), S(1.7, 3.2, 1.7)), GOLD); }
    put(I.hair, 0, ch(Gmov, P(BAL[0], BAL[1]), spin(th * 0.12)), [0.55, 0.6, 0.72]);
    const Gr = ch(Gmov, spin(lerp(PI / 2 + 0.35 * Math.sin(t * 0.4), ROTOR_AWAY, smooth(0, 0.6, d))));
    put(I.rotor, 0, Gr, GOLD); put(I.rotorRim, 0, Gr, GOLD); put(I.rotorHub, 0, Gr, STEEL);
    for (let i = 0; i < 8; i++) { const [p, y] = G.JEWELS[i]; put(I.jewels, i, ch(Gmov, P(p[0], p[1], y + 0.6), S(3.0, 1.7, 3.0)), RUBY); put(I.chatons, i, ch(Gmov, P(p[0], p[1], y + 0.5), S(5.2, 1.2, 5.2)), GOLD); }
    put(I.jewels, 8, ch(Gr, P(0, 0, 44.6), S(4, 2, 4)), RUBY); put(I.chatons, 8, ch(Gr, P(0, 0, 44.4), S(7, 1.4, 7)), STEEL);
    for (let i = 0; i < G.SCREWS.length; i++) {
      const [p, y, r] = G.SCREWS[i];
      put(I.screws, i, ch(Gmov, P(p[0], p[1], y + 1.1), S(r, 2.2, r)), i >= 8 && i <= 9 ? STEEL : BLUE);
      put(I.slots, i, ch(Gmov, P(p[0], p[1], y + 2.25), spin(i * 1.7), S(r * 1.9, 0.12, r * 0.28)), [0.01, 0.01, 0.012]);
    }

    // Case, strap, dial, hands, bezel, crystal. During the dive the front parts part sideways.
    const part = d * 900;
    const Gc = ch(Gw, T(0, exY(-60), 0));
    put(I.case, 0, Gc, STEEL); put(I.caseFloor, 0, Gc, [0.3, 0.3, 0.32]); put(I.caseInner, 0, Gc, [0.6, 0.6, 0.62]);
    put(I.crown, 0, ch(Gc, T(226 + ex * 60, 46, 0), Rz(PI / 2), spin(t * 0.0)), STEEL); put(I.stem, 0, ch(Gc, T(208 + ex * 40, 46, 0), Rz(PI / 2), S(5, 18 + ex * 30, 5)), STEEL);
    for (let i = 0; i < 4; i++) { const sx = i & 1 ? 1 : -1, sz = i & 2 ? 1 : -1; put(I.lugs, i, ch(Gc, T(sx * 104, 0, 0), S(1, 1, sz)), STEEL); put(I.lugCaps, i, ch(Gc, T(sx * 104, G.lugEnd[1], sz * G.lugEnd[2]), S(13.6)), STEEL); }
    put(I.strap, 0, ch(Gw, T(0, exY(-200), 0)), [0.16, 0.1, 0.07]);
    const Gd = ch(Gw, T(-part, 74 + exY(260), 0));
    put(I.dialTop, 0, Gd, [1, 1, 1]); put(I.dialBody, 0, Gd, [0.02, 0.03, 0.06]);
    for (let h = 0; h < 12; h++) {
      const a = (h / 12) * TAU - PI / 2;
      if (h === 0) { put(I.indices, 0, ch(Gd, T(Math.cos(a) * 124 - 5.5, 1.7, Math.sin(a) * 124), spin(a), S(24, 3.4, 4.6)), STEEL); put(I.indices, 12, ch(Gd, T(Math.cos(a) * 124 + 5.5, 1.7, Math.sin(a) * 124), spin(a), S(24, 3.4, 4.6)), STEEL); }
      else put(I.indices, h, ch(Gd, T(Math.cos(a) * 124, 1.7, Math.sin(a) * 124), spin(a), S(h % 3 ? 20 : 25, 3.4, h % 3 ? 4.6 : 6.2)), STEEL);
    }
    // Real time: the seconds hand sweeps in eight steps a second.
    const c = s.clock, sec = Math.floor(c * 8) / 8 + (1 - (1 - clamp(((c * 8) % 1) / 0.13)) ** 3) / 8;
    const hDeg = ((c / 3600) % 12) * 30, mDeg = ((c / 60) % 60) * 6, sDeg = (sec % 60) * 6;
    // Dauphine hands point to −z at 0°; spin turns +x towards +z (clockwise seen from +y).
    put(I.hh, 0, ch(Gw, T(-part * 1.15, 78.2 + exY(330), 0), spin(hDeg * DEG)), STEEL);
    put(I.mh, 0, ch(Gw, T(-part * 1.25, 80.6 + exY(370), 0), spin(mDeg * DEG)), STEEL);
    put(I.sh, 0, ch(Gw, T(-part * 1.35, 83.2 + exY(410), 0), spin(sDeg * DEG)), STEEL);
    put(I.bezel, 0, ch(Gw, T(part * 1.1, exY(480), 0)), STEEL);
    put(I.crystal, 0, ch(Gw, T(part * 1.3, exY(560), 0)), [0.5, 0.52, 0.56]);
    put(I.floor, 0, S(1500, 1, 1500), [0, 0, 0]);

    items.length = 0;
    const movVisible = ex > 0.02 || dv > 0;
    if (movVisible) for (const m of movItems) items.push(m);
    else { items.push(movItems[1]); }
    for (const m of caseItems) items.push(m);
    if (dv < 0.98) { items.push(strapItem); for (const m of dialItems) items.push(m); for (const m of handItems) items.push(m); items.push(bezelItem); }
    if (d < 0.35) items.push(floorItem);
    if (dv < 0.98) items.push(crystalItem);

    // Camera: hero framing, pull back for the exploded view, then dive to the balance.
    const hero = s.rest ?? 1;
    xf(Gw, 0, exY(230), 0, tg);
    let dist = lerp(1750, 2600, e) * Math.max(1, 0.72 / (W / H)); // portrait screens step back
    tg[1] += s.lift ?? 0;
    let el = lerp(8, 12, e) * DEG, az = lerp(0, 8, e) * DEG;
    const camx = Math.sin(az) * Math.cos(el), camy = Math.sin(el), camz = Math.cos(az) * Math.cos(el);
    cp[0] = tg[0] + camx * dist; cp[1] = tg[1] + camy * dist; cp[2] = tg[2] + camz * dist;
    if (d > 0) {
      // Macro: look at the balance from above its plane.
      // Glide along the going train: balance → pallet fork → escape wheel → fourth wheel.
      const gl = clamp(s.glide ?? 0) * (GLIDE.length - 1), gi = Math.min(GLIDE.length - 2, Math.floor(gl)), gk = smooth(0, 1, gl - gi);
      const A = GLIDE[gi], B = GLIDE[gi + 1];
      const gx = lerp(A[0], B[0], gk), gz = lerp(A[1], B[1], gk), gy = lerp(A[2], B[2], gk), gr = lerp(A[3], B[3], gk);
      xf(Gmov, gx, gy, gz, tmp);
      const mx = xf(Gmov, gx + 120 * gr, 150 * gr, gz + 150 * gr, tmp2);
      const k = d * d * (3 - 2 * d);
      for (let i = 0; i < 3; i++) { tg[i] = lerp(tg[i], tmp[i], k); cp[i] = lerp(cp[i], mx[i], k); }
    }
    cam.camera.x = cp[0]; cam.camera.y = cp[1]; cam.camera.z = cp[2];
    cam.target.x = tg[0]; cam.target.y = tg[1]; cam.target.z = tg[2];
    mirror.strength = 0.3 * (1 - d);
    opts.mirror = d < 0.35 ? mirror : undefined;
    const px = r3.render(cam, items, opts);
    // Screen positions (0–1) of the parts' right-hand edges, for the exploded-view labels.
    if (s.labels) {
      const m = cam.viewProjection().matrix, out = s.labels;
      for (let i = 0; i < 7; i++) {
        const L = LABELS[i], g = L[0] === 1 ? Gd : L[0] === 2 ? Gmov : L[0] === 3 ? Gc : Gw;
        const v = xf(g, L[1], exY(L[2]) + L[3], L[4], tmp);
        const cx = m[0] * v[0] + m[4] * v[1] + m[8] * v[2] + m[12], cy = m[1] * v[0] + m[5] * v[1] + m[9] * v[2] + m[13], cw = m[3] * v[0] + m[7] * v[1] + m[11] * v[2] + m[15];
        out[i * 2] = (cx / cw + 1) / 2; out[i * 2 + 1] = (1 - cy / cw) / 2;
      }
    }
    if (ctx) {
      if (!img || img.data.buffer !== px.buffer || img.data.byteOffset !== px.byteOffset || img.width !== W) img = new ImageData(px, W, H);
      ctx.putImageData(img, 0, 0);
    }
    return px;
  }
  return {
    render,
    resize(w, h) { W = w; H = h; r3.resize(w, h); cam.resize(w, h, 1); if (canvas) { canvas.width = w; canvas.height = h; } img = null; },
    get width() { return W; }, get height() { return H; },
    get triangles() { return r3.triangles; },
    get stats() { return r3.stats; },
    /** Put the last frame on the canvas again (an OffscreenCanvas may not show a lone draw). */
    present() { if (ctx && img) ctx.putImageData(img, 0, 0); },
  };
}
