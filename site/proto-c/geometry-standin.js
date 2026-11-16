// Procedural headphones. Every part is built from a lathe (profile spun around
// an axis) or a swept tube with an elliptical section. Vertex layout:
// position xyz, normal xyz, tangent xyz + brush coordinate (interleaved, 10 floats).

const STRIDE = 10;

function out(nv, ni) {
  return { v: new Float32Array(nv * STRIDE), i: new Uint32Array(ni), nv: 0, ni: 0 };
}

/**
 * Spin a 2D profile [[r, h], …] around the Y axis. Normals come from the profile
 * tangent, so smooth profiles give smooth shading; `closed` joins the last point
 * to the first (pillows, rings).
 */
export function lathe(profile, segments = 64, closed = false) {
  const n = profile.length;
  const rings = closed ? n + 1 : n;
  const m = out(rings * (segments + 1), (rings - 1) * segments * 6);
  // Profile normals (outward), averaged across neighbours.
  const pn = [];
  for (let k = 0; k < n; k++) {
    const a = profile[closed ? (k - 1 + n) % n : Math.max(0, k - 1)];
    const b = profile[closed ? (k + 1) % n : Math.min(n - 1, k + 1)];
    let tr = b[0] - a[0], th = b[1] - a[1];
    const l = Math.hypot(tr, th) || 1;
    tr /= l; th /= l;
    pn.push([th, -tr]); // rotate tangent clockwise: outward for counter-clockwise (r, h) profiles going up
  }
  for (let k = 0; k < rings; k++) {
    const [r, h] = profile[k % n];
    const [nr, nh] = pn[k % n];
    for (let s = 0; s <= segments; s++) {
      const a = (s / segments) * Math.PI * 2;
      const c = Math.cos(a), sn = Math.sin(a);
      const o = m.nv++ * STRIDE;
      m.v[o] = r * c; m.v[o + 1] = h; m.v[o + 2] = r * sn;
      m.v[o + 3] = nr * c; m.v[o + 4] = nh; m.v[o + 5] = nr * sn;
      m.v[o + 6] = -sn; m.v[o + 7] = 0; m.v[o + 8] = c; m.v[o + 9] = r; // brushed rings: coordinate = radius
    }
  }
  const row = segments + 1;
  for (let k = 0; k < rings - 1; k++) {
    for (let s = 0; s < segments; s++) {
      const a = k * row + s, b = a + row;
      m.i.set([a, b, a + 1, a + 1, b, b + 1], m.ni);
      m.ni += 6;
    }
  }
  return m;
}

/**
 * Sweep an elliptical section along a curve. `curve(t)` returns [x, y, z];
 * `up` is the section's fixed binormal (planar curves). `rx` is the radial
 * half-thickness in the curve's plane, `rz` across it; `taper(t)` scales both.
 */
export function sweep(curve, up, { rx, rz, steps = 96, sides = 24, taper = () => 1, caps = true }) {
  const m = out((steps + 1) * (sides + 1) + (caps ? 2 * (sides + 2) : 0), steps * sides * 6 + (caps ? sides * 6 : 0));
  const P = [], T = [];
  for (let k = 0; k <= steps; k++) P.push(curve(k / steps));
  for (let k = 0; k <= steps; k++) {
    const a = P[Math.max(0, k - 1)], b = P[Math.min(steps, k + 1)];
    const t = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    const l = Math.hypot(...t);
    T.push(t.map((x) => x / l));
  }
  const cross = (a, b) => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
  for (let k = 0; k <= steps; k++) {
    const t = T[k], N = cross(up, t), s = taper(k / steps);
    for (let j = 0; j <= sides; j++) {
      const a = (j / sides) * Math.PI * 2, c = Math.cos(a), sn = Math.sin(a);
      const o = m.nv++ * STRIDE;
      for (let q = 0; q < 3; q++) {
        m.v[o + q] = P[k][q] + (N[q] * c * rx + up[q] * sn * rz) * s;
        m.v[o + 3 + q] = N[q] * c * rz + up[q] * sn * rx; // ellipse normal (unnormalised)
        m.v[o + 6 + q] = t[q];
      }
      const l = Math.hypot(m.v[o + 3], m.v[o + 4], m.v[o + 5]);
      m.v[o + 3] /= l; m.v[o + 4] /= l; m.v[o + 5] /= l;
      m.v[o + 9] = j / sides * 6.283 * Math.max(rx, rz);
    }
  }
  const row = sides + 1;
  for (let k = 0; k < steps; k++) for (let j = 0; j < sides; j++) {
    const a = k * row + j, b = a + row;
    m.i.set([a, a + 1, b, a + 1, b + 1, b], m.ni);
    m.ni += 6;
  }
  if (caps) for (const end of [0, steps]) {
    const t = T[end], N = cross(up, t), s = taper(end / steps), dir = end ? 1 : -1;
    const centre = m.nv;
    const put = (p) => {
      const o = m.nv++ * STRIDE;
      for (let q = 0; q < 3; q++) { m.v[o + q] = p[q]; m.v[o + 3 + q] = t[q] * dir; m.v[o + 6 + q] = N[q]; }
      m.v[o + 9] = 0;
    };
    put(P[end]);
    for (let j = 0; j <= sides; j++) {
      const a = (j / sides) * Math.PI * 2, c = Math.cos(a), sn = Math.sin(a);
      put([0, 1, 2].map((q) => P[end][q] + (N[q] * c * rx + up[q] * sn * rz) * s));
    }
    for (let j = 0; j < sides; j++) {
      if (dir > 0) m.i.set([centre, centre + 1 + j, centre + 2 + j], m.ni);
      else m.i.set([centre, centre + 2 + j, centre + 1 + j], m.ni);
      m.ni += 3;
    }
  }
  return m;
}

/** Rotate/translate a mesh in place: maps the lathe's Y axis to ±X (ear cups). */
function toSide(m, side, cx, cy, cz) {
  for (let k = 0; k < m.nv; k++) {
    const o = k * STRIDE;
    for (const b of [0, 3, 6]) {
      const x = m.v[o + b], y = m.v[o + b + 1];
      // Y→side*X, X→−side*Y keeps handedness (rotation about Z).
      m.v[o + b] = y * side;
      m.v[o + b + 1] = -x * side;
    }
    m.v[o] += cx; m.v[o + 1] += cy; m.v[o + 2] += cz;
  }
  return m;
}

function arc(n, f) {
  const p = [];
  for (let k = 0; k <= n; k++) p.push(f(k / n));
  return p;
}

/** Rounded profile helper: quarter ellipse from (r0,h0) to (r1,h1). */
function corner(r0, h0, r1, h1, n = 6, rOuter = true) {
  return arc(n, (t) => {
    const a = (t * Math.PI) / 2;
    return rOuter ? [r0 + (r1 - r0) * Math.sin(a), h0 + (h1 - h0) * (1 - Math.cos(a))]
      : [r0 + (r1 - r0) * (1 - Math.cos(a)), h0 + (h1 - h0) * Math.sin(a)];
  });
}

const CUP_Y = -4.2, CUP_X = 6.6;

/**
 * Parts of the headphones. `detail` 1 for the hero, lower for the 300 copies.
 * Each part: { name, group, mesh, centre, explode: [dx,dy,dz], spin: [ax,ay,az,deg], mat }.
 */
export function buildHeadphones(detail = 1) {
  const seg = Math.max(16, Math.round(72 * detail));
  const pr = (n) => Math.max(2, Math.round(n * detail));
  const parts = [];
  const add = (p) => parts.push(p);

  // Headband: a chrome spring-steel arc, wide and thin.
  const band = (t) => {
    const a = Math.PI * (0.02 + 0.96 * t);
    return [-7.55 * Math.cos(a), 4.6 + 6.6 * Math.sin(a), 0];
  };
  add({ name: 'Headband', group: 'band', mesh: sweep(band, [0, 0, 1], { rx: 0.3, rz: 1.05, steps: pr(120), sides: pr(28) }), explode: [0, 5.5, 0], spin: [1, 0, 0, -18], mat: 'chrome' });

  // Cushion under the band.
  const pad = (t) => {
    const a = Math.PI * (0.22 + 0.56 * t);
    return [-7.0 * Math.cos(a), 4.6 + 5.85 * Math.sin(a), 0];
  };
  add({ name: 'Band cushion', group: 'bandpad', mesh: sweep(pad, [0, 0, 1], { rx: 0.42, rz: 0.95, steps: pr(80), sides: pr(24), taper: (t) => 0.55 + 0.45 * Math.sin(Math.PI * t) ** 0.5 }), explode: [0, 1.2, 6.5], spin: [1, 0, 0, 24], mat: 'rubberSand' });

  for (const s of [-1, 1]) {
    // Telescopic slider: a capsule.
    const cap = [...corner(0, 0, 0.42, 0.42, pr(6), false)].map(([r, h]) => [r, h]);
    const slider = [[0.001, 0], ...arc(pr(6), (t) => [0.42 * Math.sin((t * Math.PI) / 2), 0.42 - 0.42 * Math.cos((t * Math.PI) / 2)]).slice(1),
      ...arc(pr(6), (t) => [0.42 * Math.cos((t * Math.PI) / 2), 3.9 + 0.42 * Math.sin((t * Math.PI) / 2)]), [0.001, 4.32]];
    void cap;
    const sm = lathe(slider, pr(28));
    for (let k = 0; k < sm.nv; k++) { const o = k * STRIDE; sm.v[o] += s * 7.55; sm.v[o + 1] += 1.2; }
    add({ name: 'Slider', group: 'slider', mesh: sm, explode: [s * 3.2, 2.6, -1.5], spin: [0, 0, 1, s * 14], mat: 'brushed' });

    // Yoke: a fork that holds the cup at two pivots, plus its stem.
    const R = 4.75;
    const yoke = (t) => {
      if (t < 0.18) { const u = t / 0.18; return [s * 7.55, 1.3 - 0.75 * u, 0]; }
      const a = Math.PI * ((t - 0.18) / 0.82);
      return [s * 7.55, CUP_Y + R * Math.sin(a) * 0.98 + 0.5 * (1 - Math.sin(a)) * 0, R * Math.cos(a) * (t < 0.59 ? 1 : 1)];
    };
    // Two arms from the stem down to each pivot.
    const arm = (dir) => (t) => {
      const a = (Math.PI / 2) * (1 - t);
      const top = [s * 7.55, CUP_Y + R, 0];
      if (t < 0) return top;
      return [s * 7.55, CUP_Y + R * Math.sin(a), dir * R * Math.cos(a)];
    };
    void yoke;
    const yk = merge([
      sweep(arm(1), [s, 0, 0], { rx: 0.3, rz: 0.22, steps: pr(40), sides: pr(16) }),
      sweep(arm(-1), [s, 0, 0], { rx: 0.3, rz: 0.22, steps: pr(40), sides: pr(16) }),
      sweep((t) => [s * 7.55, CUP_Y + R - 0.1 + t * (1.5 - CUP_Y - R + 0.1), 0], [0, 0, 1], { rx: 0.26, rz: 0.26, steps: 4, sides: pr(16) }),
    ]);
    add({ name: 'Yoke', group: 'yoke', mesh: yk, explode: [s * 5.2, 0.6, 0], spin: [0, 1, 0, s * 20], mat: 'chrome' });

    // Housing: brushed aluminium shell, lathed around X.
    const shell = [[0.001, 0], [3.0, 0], ...corner(3.9, 0.02, 4.25, 0.5, pr(5), true).map(([r, h]) => [r, h]),
      [4.28, 1.5], ...arc(pr(6), (t) => [4.28 - 0.75 * (1 - Math.cos((t * Math.PI) / 2)), 1.5 + 0.7 * Math.sin((t * Math.PI) / 2)]), [3.35, 2.25], [3.3, 2.15], [0.001, 2.15]];
    add({ name: 'Housing', group: 'cup', mesh: toSide(lathe(shell, seg), s, s * CUP_X, CUP_Y, 0), explode: [s * 9.5, -0.6, 0.6], spin: [0, 0, 1, s * -8], mat: 'brushedMist' });

    // Glass cap: frosted dome on the outside.
    const glass = [[0.001, 2.62], ...arc(pr(10), (t) => [3.22 * Math.sin((t * Math.PI) / 2), 2.62 - 0.32 * (1 - Math.cos((t * Math.PI) / 2))]), [3.24, 2.1], [0.001, 2.1]].reverse();
    add({ name: 'Glass cap', group: 'glass', mesh: toSide(lathe(glass, seg), s, s * CUP_X, CUP_Y, 0), explode: [s * 14.5, 0.3, 1.0], spin: [0, 1, 0, s * 26], mat: 'glass' });

    // Ear cushion: a soft pillow ring (closed profile).
    const cushion = arc(pr(28), (t) => {
      const a = t * Math.PI * 2;
      const r = 3.05 + 1.05 * Math.cos(a), h = -0.62 + 0.72 * Math.sin(a) * (Math.sin(a) > 0 ? 0.85 : 1);
      return [r, h];
    }).slice(0, -1);
    add({ name: 'Ear cushion', group: 'pad', mesh: toSide(lathe(cushion, seg, true), s, s * CUP_X, CUP_Y, 0), explode: [s * 1.0, -3.4, 6.5], spin: [0, 1, 0, s * -38], mat: 'rubberSand' });
  }
  for (const p of parts) {
    const v = p.mesh.v;
    let x = 0, y = 0, z = 0;
    for (let k = 0; k < p.mesh.nv; k++) { x += v[k * STRIDE]; y += v[k * STRIDE + 1]; z += v[k * STRIDE + 2]; }
    p.centre = [x / p.mesh.nv, y / p.mesh.nv, z / p.mesh.nv];
  }
  return parts;
}

function merge(list) {
  const nv = list.reduce((a, m) => a + m.nv, 0), ni = list.reduce((a, m) => a + m.ni, 0);
  const m = out(nv, ni);
  for (const p of list) {
    m.v.set(p.v.subarray(0, p.nv * STRIDE), m.nv * STRIDE);
    for (let k = 0; k < p.ni; k++) m.i[m.ni + k] = p.i[k] + m.nv;
    m.nv += p.nv; m.ni += p.ni;
  }
  return m;
}

export const VERTEX_STRIDE = STRIDE;
