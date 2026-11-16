/**
 * Geometry for the real-time GPU scene (`anikit/scene`): primitives, lathe,
 * extrude, tube, procedural SDF meshing (surface nets), smooth normals and
 * subdivision. Everything returns a plain {@link Geometry} (typed arrays), so
 * meshes can be generated in a worker, cached, or fed to `scene().mesh()`.
 *
 * ```ts
 * const g = sphere(1, 48);                 // 49×25 vertices
 * const vase = lathe([[0, 0], [0.6, 0.1], [0.3, 1], [0.45, 1.6]], 64);
 * const blob = sdfMesh((x, y, z) => smoothUnion(sdSphere(x - 0.4, y, z, 0.6), sdSphere(x + 0.4, y, z, 0.5), 0.3));
 * ```
 */

/** Indexed triangle mesh: positions/normals (xyz), uvs (uv), indices. */
export interface Geometry {
  /** Vertex positions, 3 floats per vertex. */
  positions: Float32Array;
  /** Vertex normals, 3 floats per vertex (unit length). */
  normals: Float32Array;
  /** Texture coordinates, 2 floats per vertex. */
  uvs: Float32Array;
  /** Triangle indices (counter-clockwise front faces). */
  indices: Uint32Array;
}

class Builder {
  p: number[] = [];
  n: number[] = [];
  t: number[] = [];
  i: number[] = [];
  v(x: number, y: number, z: number, nx: number, ny: number, nz: number, u: number, w: number): number {
    this.p.push(x, y, z); this.n.push(nx, ny, nz); this.t.push(u, w);
    return this.p.length / 3 - 1;
  }
  tri(a: number, b: number, c: number): void { this.i.push(a, b, c); }
  /** Grid of (cols+1)×(rows+1) vertices from `f(u, v)`; returns the first index. */
  grid(cols: number, rows: number, f: (u: number, v: number, out: number[]) => void, flip = false): void {
    const base = this.p.length / 3;
    const o = [0, 0, 0, 0, 0, 0];
    for (let r = 0; r <= rows; r++) for (let c = 0; c <= cols; c++) {
      f(c / cols, r / rows, o);
      this.v(o[0], o[1], o[2], o[3], o[4], o[5], c / cols, r / rows);
    }
    const w = cols + 1;
    for (let r = 0; r < rows; r++) for (let c = 0; c < cols; c++) {
      const a = base + r * w + c, b = a + 1, d = a + w, e = d + 1;
      if (flip) { this.tri(a, b, d); this.tri(b, e, d); } else { this.tri(a, d, b); this.tri(b, d, e); }
    }
  }
  done(): Geometry {
    return { positions: new Float32Array(this.p), normals: new Float32Array(this.n), uvs: new Float32Array(this.t), indices: new Uint32Array(this.i) };
  }
}

const norm3 = (o: number[], at: number) => {
  const l = Math.hypot(o[at], o[at + 1], o[at + 2]) || 1;
  o[at] /= l; o[at + 1] /= l; o[at + 2] /= l;
};

/** Box centred on the origin, flat-shaded faces (24 vertices). */
export function box(w = 1, h = 1, d = 1, segments = 1): Geometry {
  const b = new Builder();
  const hx = w / 2, hy = h / 2, hz = d / 2;
  // [axis u, axis v, normal axis, sign]
  const faces: Array<[number, number, number, number]> = [[2, 1, 0, 1], [2, 1, 0, -1], [0, 2, 1, 1], [0, 2, 1, -1], [0, 1, 2, 1], [0, 1, 2, -1]];
  const half = [hx, hy, hz];
  for (const [ua, va, na, s] of faces) {
    b.grid(segments, segments, (u, v, o) => {
      const p = [0, 0, 0];
      p[na] = s * half[na];
      // Keep counter-clockwise winding when viewed from outside.
      const su = na === 1 ? 1 : s * (na === 0 ? -1 : 1);
      p[ua] = (u * 2 - 1) * half[ua] * su;
      p[va] = (v * 2 - 1) * half[va] * (na === 1 ? s : 1);
      o[0] = p[0]; o[1] = p[1]; o[2] = p[2];
      o[3] = na === 0 ? s : 0; o[4] = na === 1 ? s : 0; o[5] = na === 2 ? s : 0;
    }, true);
  }
  return fixWinding(b.done());
}

/** Box with rounded edges and corners of radius `r` (smooth normals). */
export function roundedBox(w = 1, h = 1, d = 1, r = 0.1, segments = 6): Geometry {
  r = Math.max(1e-4, Math.min(r, w / 2, h / 2, d / 2));
  const S = segments * 2 + 1;
  const g = box(1, 1, 1, S);
  const half = [w / 2, h / 2, d / 2], inner = half.map((x) => x - r);
  // Cells: `segments` across each rounded band, one across the flat middle.
  const remap = (c: number, a: number) => {
    const t = (c + 0.5) * S;
    if (t <= segments) return -half[a] + (t / segments) * r;
    if (t >= segments + 1) return inner[a] + ((t - segments - 1) / segments) * r;
    return -inner[a] + (t - segments) * 2 * inner[a];
  };
  const p = g.positions, n = g.normals, q = [0, 0, 0];
  for (let i = 0; i < p.length; i += 3) {
    let l = 0;
    for (let a = 0; a < 3; a++) {
      const v = remap(p[i + a], a);
      const c = Math.max(-inner[a], Math.min(inner[a], v));
      q[a] = c; n[i + a] = v - c; l += (v - c) ** 2;
    }
    l = Math.sqrt(l) || 1;
    for (let a = 0; a < 3; a++) { n[i + a] /= l; p[i + a] = q[a] + n[i + a] * r; }
  }
  return fixWinding(g);
}

/** UV sphere of radius `r` with `segments` around and `segments/2` rings. */
export function sphere(r = 1, segments = 32): Geometry {
  const b = new Builder();
  const rings = Math.max(2, segments >> 1);
  b.grid(segments, rings, (u, v, o) => {
    const th = u * Math.PI * 2, ph = v * Math.PI;
    const x = -Math.cos(th) * Math.sin(ph), y = Math.cos(ph), z = Math.sin(th) * Math.sin(ph);
    o[0] = x * r; o[1] = y * r; o[2] = z * r; o[3] = x; o[4] = y; o[5] = z;
  });
  return fixWinding(b.done());
}

/** Surface of revolution around Y from a `[radius, y]` profile (bottom to top). */
export function lathe(profile: ReadonlyArray<readonly [number, number]>, segments = 32): Geometry {
  const b = new Builder();
  const m = profile.length - 1;
  // Profile normals (2D), averaged at interior points.
  const pn: number[] = [];
  for (let i = 0; i <= m; i++) {
    const a = profile[Math.max(0, i - 1)], c = profile[Math.min(m, i + 1)];
    const dr = c[0] - a[0], dy = c[1] - a[1];
    const l = Math.hypot(dr, dy) || 1;
    pn.push(dy / l, -dr / l);
  }
  b.grid(segments, m, (u, v, o) => {
    const i = Math.round(v * m);
    const th = u * Math.PI * 2, c = Math.cos(th), s = Math.sin(th);
    const [r, y] = profile[i];
    o[0] = c * r; o[1] = y; o[2] = -s * r;
    o[3] = c * pn[i * 2]; o[4] = pn[i * 2 + 1]; o[5] = -s * pn[i * 2];
  });
  return fixWinding(b.done());
}

/** Cylinder (or cone) along Y with caps. */
export function cylinder(rTop = 0.5, rBottom = 0.5, h = 1, segments = 32): Geometry {
  const y = h / 2;
  return lathe([[0, -y], [rBottom, -y], [rBottom, -y], [rTop, y], [rTop, y], [0, y]], segments);
}

/** Capsule along Y: total height `h` (including the hemispheres) and radius `r`. */
export function capsule(r = 0.5, h = 2, segments = 32): Geometry {
  const prof: Array<[number, number]> = [];
  const q = Math.max(4, segments >> 2), half = Math.max(0, h / 2 - r);
  for (let i = 0; i <= q; i++) { const a = -Math.PI / 2 + (i / q) * Math.PI / 2; prof.push([Math.cos(a) * r, -half + Math.sin(a) * r]); }
  for (let i = 0; i <= q; i++) { const a = (i / q) * Math.PI / 2; prof.push([Math.cos(a) * r, half + Math.sin(a) * r]); }
  prof[0][0] = 0; prof[prof.length - 1][0] = 0;
  return smoothNormals(lathe(prof, segments));
}

/** Torus in the XZ plane: ring radius `R`, tube radius `r`. */
export function torus(R = 1, r = 0.3, segments = 48, tubeSegments = 16): Geometry {
  const b = new Builder();
  b.grid(segments, tubeSegments, (u, v, o) => {
    const th = u * Math.PI * 2, ph = v * Math.PI * 2;
    const nx = Math.cos(th) * Math.cos(ph), ny = Math.sin(ph), nz = -Math.sin(th) * Math.cos(ph);
    o[0] = Math.cos(th) * R + nx * r; o[1] = ny * r; o[2] = -Math.sin(th) * R + nz * r;
    o[3] = nx; o[4] = ny; o[5] = nz;
  });
  return fixWinding(b.done());
}

/** Plane in XZ facing +Y. */
export function plane(w = 1, d = 1, segments = 1): Geometry {
  const b = new Builder();
  b.grid(segments, segments, (u, v, o) => {
    o[0] = (u - 0.5) * w; o[1] = 0; o[2] = (v - 0.5) * d; o[3] = 0; o[4] = 1; o[5] = 0;
  });
  return fixWinding(b.done());
}

/** Area of a 2D polygon (positive when counter-clockwise). */
function area2(pts: ReadonlyArray<readonly [number, number]>): number {
  let a = 0;
  for (let i = 0, j = pts.length - 1; i < pts.length; j = i++) a += pts[j][0] * pts[i][1] - pts[i][0] * pts[j][1];
  return a / 2;
}

/** Ear-clipping triangulation of a simple polygon; returns index triples. */
export function triangulate(pts: ReadonlyArray<readonly [number, number]>): number[] {
  const n = pts.length;
  const idx = Array.from({ length: n }, (_, i) => i);
  if (area2(pts) < 0) idx.reverse();
  const out: number[] = [];
  const cross = (a: number, b: number, c: number) => (pts[b][0] - pts[a][0]) * (pts[c][1] - pts[a][1]) - (pts[b][1] - pts[a][1]) * (pts[c][0] - pts[a][0]);
  const inside = (p: number, a: number, b: number, c: number) => cross(a, b, p) >= 0 && cross(b, c, p) >= 0 && cross(c, a, p) >= 0;
  let guard = 0;
  while (idx.length > 3 && guard++ < n * n) {
    let clipped = false;
    for (let i = 0; i < idx.length; i++) {
      const a = idx[(i + idx.length - 1) % idx.length], b = idx[i], c = idx[(i + 1) % idx.length];
      if (cross(a, b, c) <= 0) continue;
      let ear = true;
      for (const p of idx) if (p !== a && p !== b && p !== c && inside(p, a, b, c)) { ear = false; break; }
      if (!ear) continue;
      out.push(a, b, c); idx.splice(i, 1); clipped = true; break;
    }
    if (!clipped) break;
  }
  if (idx.length === 3) out.push(idx[0], idx[1], idx[2]);
  return out;
}

/** Extrude a closed 2D outline (`[x, y]` points, XY plane) along Z by `depth`, with caps. */
export function extrude(shape: ReadonlyArray<readonly [number, number]>, depth = 0.2): Geometry {
  const b = new Builder();
  const pts = area2(shape) < 0 ? [...shape].reverse() : shape;
  const n = pts.length, z0 = -depth / 2, z1 = depth / 2;
  const tris = triangulate(pts);
  let base = b.p.length / 3;
  for (const [x, y] of pts) b.v(x, y, z1, 0, 0, 1, x, y);
  for (let i = 0; i < tris.length; i += 3) b.tri(base + tris[i], base + tris[i + 1], base + tris[i + 2]);
  base = b.p.length / 3;
  for (const [x, y] of pts) b.v(x, y, z0, 0, 0, -1, x, y);
  for (let i = 0; i < tris.length; i += 3) b.tri(base + tris[i], base + tris[i + 2], base + tris[i + 1]);
  for (let i = 0; i < n; i++) {
    const [ax, ay] = pts[i], [bx, by] = pts[(i + 1) % n];
    let nx = by - ay, ny = -(bx - ax);
    const l = Math.hypot(nx, ny) || 1; nx /= l; ny /= l;
    const s = b.v(ax, ay, z0, nx, ny, 0, 0, 0);
    b.v(bx, by, z0, nx, ny, 0, 1, 0); b.v(bx, by, z1, nx, ny, 0, 1, 1); b.v(ax, ay, z1, nx, ny, 0, 0, 1);
    b.tri(s, s + 1, s + 2); b.tri(s, s + 2, s + 3);
  }
  return b.done();
}

/** Tube of radius `r` along a curve (`[x, y, z]` points, or `t ∈ [0,1] → [x,y,z]`). */
export function tube(
  curve: ReadonlyArray<readonly [number, number, number]> | ((t: number) => readonly [number, number, number]),
  r = 0.1, segments = 64, radial = 12,
): Geometry {
  const pts: Array<readonly [number, number, number]> = typeof curve === 'function'
    ? Array.from({ length: segments + 1 }, (_, i) => curve(i / segments)) : curve.slice();
  const m = pts.length - 1;
  // Parallel-transport frames.
  const T: number[][] = [], N: number[][] = [], B: number[][] = [];
  for (let i = 0; i <= m; i++) {
    const a = pts[Math.max(0, i - 1)], c = pts[Math.min(m, i + 1)];
    const t = [c[0] - a[0], c[1] - a[1], c[2] - a[2]]; norm3(t, 0); T.push(t);
  }
  const t0 = T[0];
  let n0 = Math.abs(t0[1]) < 0.9 ? [0, 1, 0] : [1, 0, 0];
  n0 = cross3(cross3(t0, n0), t0); norm3(n0, 0);
  N.push(n0); B.push(cross3(t0, n0));
  for (let i = 1; i <= m; i++) {
    const t = T[i];
    let n = N[i - 1];
    const d = n[0] * t[0] + n[1] * t[1] + n[2] * t[2];
    n = [n[0] - t[0] * d, n[1] - t[1] * d, n[2] - t[2] * d]; norm3(n, 0);
    N.push(n); B.push(cross3(t, n));
  }
  const b = new Builder();
  b.grid(radial, m, (u, v, o) => {
    const i = Math.round(v * m), a = -u * Math.PI * 2, c = Math.cos(a), s = Math.sin(a);
    const nx = N[i][0] * c + B[i][0] * s, ny = N[i][1] * c + B[i][1] * s, nz = N[i][2] * c + B[i][2] * s;
    o[0] = pts[i][0] + nx * r; o[1] = pts[i][1] + ny * r; o[2] = pts[i][2] + nz * r; o[3] = nx; o[4] = ny; o[5] = nz;
  });
  return fixWinding(b.done());
}

function cross3(a: number[], b: number[]): number[] {
  return [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
}

/**
 * Make every triangle's winding agree with its vertex normals (front faces
 * counter-clockwise). Generators call it so the conventions can't drift.
 */
function fixWinding(g: Geometry): Geometry {
  const p = g.positions, n = g.normals, ix = g.indices;
  for (let t = 0; t < ix.length; t += 3) {
    const a = ix[t] * 3, b = ix[t + 1] * 3, c = ix[t + 2] * 3;
    const ux = p[b] - p[a], uy = p[b + 1] - p[a + 1], uz = p[b + 2] - p[a + 2];
    const vx = p[c] - p[a], vy = p[c + 1] - p[a + 1], vz = p[c + 2] - p[a + 2];
    const fx = uy * vz - uz * vy, fy = uz * vx - ux * vz, fz = ux * vy - uy * vx;
    const nx = n[a] + n[b] + n[c], ny = n[a + 1] + n[b + 1] + n[c + 1], nz = n[a + 2] + n[b + 2] + n[c + 2];
    if (fx * nx + fy * ny + fz * nz < 0) { const s = ix[t + 1]; ix[t + 1] = ix[t + 2]; ix[t + 2] = s; }
  }
  return g;
}

/**
 * Recompute smooth (area-weighted) vertex normals. Vertices at the same
 * position (seams) are welded for the average, so seams stay invisible.
 */
export function smoothNormals(g: Geometry): Geometry {
  const p = g.positions, ix = g.indices, n = new Float32Array(p.length);
  const key = (i: number) => `${Math.round(p[i * 3] * 1e4)},${Math.round(p[i * 3 + 1] * 1e4)},${Math.round(p[i * 3 + 2] * 1e4)}`;
  const weld = new Map<string, number>();
  const rep = new Uint32Array(p.length / 3);
  for (let i = 0; i < rep.length; i++) { const k = key(i); let r = weld.get(k); if (r === undefined) { r = i; weld.set(k, i); } rep[i] = r; }
  for (let t = 0; t < ix.length; t += 3) {
    const a = ix[t] * 3, b = ix[t + 1] * 3, c = ix[t + 2] * 3;
    const ux = p[b] - p[a], uy = p[b + 1] - p[a + 1], uz = p[b + 2] - p[a + 2];
    const vx = p[c] - p[a], vy = p[c + 1] - p[a + 1], vz = p[c + 2] - p[a + 2];
    const fx = uy * vz - uz * vy, fy = uz * vx - ux * vz, fz = ux * vy - uy * vx;
    for (const v of [ix[t], ix[t + 1], ix[t + 2]]) { const r = rep[v] * 3; n[r] += fx; n[r + 1] += fy; n[r + 2] += fz; }
  }
  for (let i = 0; i < rep.length; i++) {
    const r = rep[i] * 3, l = Math.hypot(n[r], n[r + 1], n[r + 2]) || 1;
    g.normals[i * 3] = n[r] / l; g.normals[i * 3 + 1] = n[r + 1] / l; g.normals[i * 3 + 2] = n[r + 2] / l;
  }
  return g;
}

/**
 * Subdivide every triangle into four (shared edge midpoints). With
 * `smooth > 0` new and old vertices relax toward their neighbours
 * (Laplacian smoothing), rounding hard shapes; normals are recomputed.
 */
export function subdivide(g: Geometry, levels = 1, smooth = 0.5): Geometry {
  let p = Array.from(g.positions), uv = Array.from(g.uvs), ix = Array.from(g.indices);
  for (let l = 0; l < levels; l++) {
    const mid = new Map<string, number>();
    const out: number[] = [];
    const m = (a: number, b: number) => {
      const k = a < b ? `${a}_${b}` : `${b}_${a}`;
      let i = mid.get(k);
      if (i === undefined) {
        i = p.length / 3;
        p.push((p[a * 3] + p[b * 3]) / 2, (p[a * 3 + 1] + p[b * 3 + 1]) / 2, (p[a * 3 + 2] + p[b * 3 + 2]) / 2);
        uv.push((uv[a * 2] + uv[b * 2]) / 2, (uv[a * 2 + 1] + uv[b * 2 + 1]) / 2);
        mid.set(k, i);
      }
      return i;
    };
    for (let t = 0; t < ix.length; t += 3) {
      const a = ix[t], b = ix[t + 1], c = ix[t + 2], ab = m(a, b), bc = m(b, c), ca = m(c, a);
      out.push(a, ab, ca, ab, b, bc, ca, bc, c, ab, bc, ca);
    }
    ix = out;
    if (smooth > 0) {
      const sum = new Float64Array(p.length), cnt = new Float64Array(p.length / 3);
      for (let t = 0; t < ix.length; t += 3) for (let e = 0; e < 3; e++) {
        const a = ix[t + e], b = ix[t + (e + 1) % 3];
        sum[a * 3] += p[b * 3]; sum[a * 3 + 1] += p[b * 3 + 1]; sum[a * 3 + 2] += p[b * 3 + 2]; cnt[a]++;
      }
      for (let i = 0; i < cnt.length; i++) if (cnt[i]) for (let k = 0; k < 3; k++) p[i * 3 + k] += (sum[i * 3 + k] / cnt[i] - p[i * 3 + k]) * smooth;
    }
  }
  const r: Geometry = { positions: new Float32Array(p), normals: new Float32Array(p.length), uvs: new Float32Array(uv), indices: new Uint32Array(ix) };
  return smoothNormals(r);
}

/** Signed distance to a sphere of radius `r` centred on the origin. */
export function sdSphere(x: number, y: number, z: number, r: number): number { return Math.hypot(x, y, z) - r; }
/** Signed distance to a box of half-extents `bx, by, bz` with edge rounding `r`. */
export function sdBox(x: number, y: number, z: number, bx: number, by: number, bz: number, r = 0): number {
  const qx = Math.abs(x) - bx + r, qy = Math.abs(y) - by + r, qz = Math.abs(z) - bz + r;
  return Math.hypot(Math.max(qx, 0), Math.max(qy, 0), Math.max(qz, 0)) + Math.min(Math.max(qx, qy, qz), 0) - r;
}
/** Signed distance to a torus in XZ (ring `R`, tube `r`). */
export function sdTorus(x: number, y: number, z: number, R: number, r: number): number { return Math.hypot(Math.hypot(x, z) - R, y) - r; }
/** Polynomial smooth minimum: a blend of two SDFs over distance `k`. */
export function smoothUnion(a: number, b: number, k: number): number {
  const h = Math.max(k - Math.abs(a - b), 0) / k;
  return Math.min(a, b) - h * h * k * 0.25;
}
/** Smooth subtraction of `b` from `a`. */
export function smoothSubtract(a: number, b: number, k: number): number { return -smoothUnion(-a, b, k); }

/** Options for {@link sdfMesh}. */
export interface SdfOptions {
  /** Half-size of the sampled box around the origin: a number (cube) or `[x, y, z]` (default 1.5). */
  bounds?: number | readonly [number, number, number];
  /** Cells along the longest axis (default 48). Cost grows with the cube of this. */
  resolution?: number;
}

/**
 * Mesh a signed distance function `f(x, y, z)` (negative inside) with
 * surface nets: one vertex per surface cell, smooth gradient normals, no
 * cracks. Combine shapes with {@link smoothUnion} / {@link smoothSubtract}.
 */
export function sdfMesh(f: (x: number, y: number, z: number) => number, opts: SdfOptions = {}): Geometry {
  const B = opts.bounds ?? 1.5;
  const hx = typeof B === 'number' ? B : B[0], hy = typeof B === 'number' ? B : B[1], hz = typeof B === 'number' ? B : B[2];
  const R = Math.max(4, Math.min(512, opts.resolution ?? 48) | 0);
  const s = (2 * Math.max(hx, hy, hz)) / R;
  const Rx = Math.max(2, Math.ceil((2 * hx) / s)), Ry = Math.max(2, Math.ceil((2 * hy) / s)), Rz = Math.max(2, Math.ceil((2 * hz) / s));
  const Nx = Rx + 1, Ny = Ry + 1, Nz = Rz + 1;
  const ox = -Rx * s / 2, oy = -Ry * s / 2, oz = -Rz * s / 2;
  const d = new Float32Array(Nx * Ny * Nz);
  for (let k = 0; k < Nz; k++) for (let j = 0; j < Ny; j++) for (let i = 0; i < Nx; i++) d[i + Nx * (j + Ny * k)] = f(ox + i * s, oy + j * s, oz + k * s);
  const cell = new Int32Array(Rx * Ry * Rz).fill(-1);
  const pos: number[] = [], nor: number[] = [];
  const at = (i: number, j: number, k: number) => d[i + Nx * (j + Ny * k)];
  const E = [[0, 1], [2, 3], [4, 5], [6, 7], [0, 2], [1, 3], [4, 6], [5, 7], [0, 4], [1, 5], [2, 6], [3, 7]];
  const c = new Float32Array(8);
  for (let k = 0; k < Rz; k++) for (let j = 0; j < Ry; j++) for (let i = 0; i < Rx; i++) {
    let mask = 0;
    for (let q = 0; q < 8; q++) { c[q] = at(i + (q & 1), j + ((q >> 1) & 1), k + ((q >> 2) & 1)); if (c[q] < 0) mask |= 1 << q; }
    if (mask === 0 || mask === 255) continue;
    let x = 0, y = 0, z = 0, m = 0;
    for (const [a, b] of E) {
      if ((c[a] < 0) === (c[b] < 0)) continue;
      const t = c[a] / (c[a] - c[b]);
      x += (a & 1) + t * ((b & 1) - (a & 1)); y += ((a >> 1) & 1) + t * (((b >> 1) & 1) - ((a >> 1) & 1)); z += ((a >> 2) & 1) + t * (((b >> 2) & 1) - ((a >> 2) & 1)); m++;
    }
    const px = ox + (i + x / m) * s, py = oy + (j + y / m) * s, pz = oz + (k + z / m) * s;
    const e = s * 0.5;
    const gx = f(px + e, py, pz) - f(px - e, py, pz), gy = f(px, py + e, pz) - f(px, py - e, pz), gz = f(px, py, pz + e) - f(px, py, pz - e);
    const l = Math.hypot(gx, gy, gz) || 1;
    cell[i + Rx * (j + Ry * k)] = pos.length / 3;
    pos.push(px, py, pz); nor.push(gx / l, gy / l, gz / l);
  }
  const idx: number[] = [];
  const C = (i: number, j: number, k: number) => cell[i + Rx * (j + Ry * k)];
  const quad = (a: number, b: number, cc: number, dd: number, flip: boolean) => {
    if (a < 0 || b < 0 || cc < 0 || dd < 0) return;
    if (flip) idx.push(a, cc, b, b, cc, dd); else idx.push(a, b, cc, b, dd, cc);
  };
  for (let k = 1; k < Rz; k++) for (let j = 1; j < Ry; j++) for (let i = 0; i < Rx; i++) {
    const a = at(i, j, k), b = at(i + 1, j, k);
    if ((a < 0) !== (b < 0)) quad(C(i, j - 1, k - 1), C(i, j, k - 1), C(i, j - 1, k), C(i, j, k), a < 0);
  }
  for (let k = 1; k < Rz; k++) for (let j = 0; j < Ry; j++) for (let i = 1; i < Rx; i++) {
    const a = at(i, j, k), b = at(i, j + 1, k);
    if ((a < 0) !== (b < 0)) quad(C(i - 1, j, k - 1), C(i - 1, j, k), C(i, j, k - 1), C(i, j, k), a < 0);
  }
  for (let k = 0; k < Rz; k++) for (let j = 1; j < Ry; j++) for (let i = 1; i < Rx; i++) {
    const a = at(i, j, k), b = at(i, j, k + 1);
    if ((a < 0) !== (b < 0)) quad(C(i - 1, j - 1, k), C(i, j - 1, k), C(i - 1, j, k), C(i, j, k), a < 0);
  }
  const g: Geometry = { positions: new Float32Array(pos), normals: new Float32Array(nor), uvs: new Float32Array((pos.length / 3) * 2), indices: new Uint32Array(idx) };
  return fixWinding(g);
}

/** Options for {@link text3d}. */
export interface Text3dOptions {
  /** CSS font (default `'bold 100px sans-serif'`). The size only sets raster detail. */
  font?: string;
  /** Height of a capital letter in scene units (default 1). */
  size?: number;
  /** Extrusion depth (default 0.3). */
  depth?: number;
  /** Rounded bevel radius (default 0.04). */
  bevel?: number;
  /** Mesh cells along the text width (default 160). */
  resolution?: number;
}

/**
 * Extruded, bevelled 3D text, centred on the origin and facing +Z. The glyphs
 * are rasterised with Canvas 2D (needs a DOM or OffscreenCanvas), turned into
 * a 2D distance field, extruded with a rounded bevel and meshed with
 * surface nets.
 */
export function text3d(text: string, o: Text3dOptions = {}): Geometry {
  const font = o.font ?? 'bold 100px sans-serif', size = o.size ?? 1, depth = o.depth ?? 0.3, bevel = o.bevel ?? 0.04;
  const px = Number(/(\d+(?:\.\d+)?)px/.exec(font)?.[1] ?? 100);
  const pad = Math.ceil(px * 0.25);
  const mk = (w: number, h: number): OffscreenCanvas | HTMLCanvasElement => {
    if (typeof OffscreenCanvas === 'function') return new OffscreenCanvas(w, h);
    const c = document.createElement('canvas'); c.width = w; c.height = h; return c;
  };
  let cv = mk(8, 8);
  let ctx = cv.getContext('2d') as CanvasRenderingContext2D | null;
  if (!ctx) throw new Error('anikit: text3d() needs Canvas 2D');
  ctx.font = font;
  const W = Math.ceil(ctx.measureText(text).width) + pad * 2, H = Math.ceil(px * 1.3) + pad * 2;
  cv = mk(W, H); ctx = cv.getContext('2d') as CanvasRenderingContext2D;
  ctx.font = font; ctx.textBaseline = 'middle'; ctx.fillStyle = '#fff';
  ctx.fillText(text, pad, H / 2);
  const img = ctx.getImageData(0, 0, W, H).data;
  // Two-pass (Felzenszwalb) exact Euclidean distance transform, inside and outside.
  const edt = (inside: boolean): Float32Array => {
    const INF = 1e20, g = new Float32Array(W * H);
    for (let i = 0; i < W * H; i++) g[i] = (img[i * 4 + 3] > 127) === inside ? 0 : INF;
    const f = new Float32Array(Math.max(W, H)), dd = new Float32Array(Math.max(W, H)), v = new Int32Array(Math.max(W, H)), z = new Float32Array(Math.max(W, H) + 1);
    const pass = (n: number, get: (i: number) => number, set: (i: number, x: number) => void) => {
      for (let i = 0; i < n; i++) f[i] = get(i);
      let k = 0; v[0] = 0; z[0] = -INF; z[1] = INF;
      for (let q = 1; q < n; q++) {
        let s2 = ((f[q] + q * q) - (f[v[k]] + v[k] * v[k])) / (2 * q - 2 * v[k]);
        while (s2 <= z[k]) { k--; s2 = ((f[q] + q * q) - (f[v[k]] + v[k] * v[k])) / (2 * q - 2 * v[k]); }
        k++; v[k] = q; z[k] = s2; z[k + 1] = INF;
      }
      k = 0;
      for (let q = 0; q < n; q++) { while (z[k + 1] < q) k++; dd[q] = (q - v[k]) ** 2 + f[v[k]]; }
      for (let i = 0; i < n; i++) set(i, dd[i]);
    };
    for (let x = 0; x < W; x++) pass(H, (y) => g[y * W + x], (y, val) => { g[y * W + x] = val; });
    for (let y = 0; y < H; y++) pass(W, (x) => g[y * W + x], (x, val) => { g[y * W + x] = val; });
    return g;
  };
  const dOut = edt(true), dIn = edt(false);
  const scale = size / (px * 0.72); // cap height ≈ 0.72 em
  const sd = new Float32Array(W * H);
  for (let i = 0; i < W * H; i++) sd[i] = (Math.sqrt(dOut[i]) - Math.sqrt(dIn[i])) * scale;
  const hw = (W * scale) / 2, hh = (H * scale) / 2;
  const field2 = (x: number, y: number) => {
    const fx = Math.max(0, Math.min(W - 1.001, x / scale + W / 2 - 0.5)), fy = Math.max(0, Math.min(H - 1.001, H / 2 - y / scale - 0.5));
    const ix = fx | 0, iy = fy | 0, tx = fx - ix, ty = fy - iy, i = iy * W + ix;
    return (sd[i] * (1 - tx) + sd[i + 1] * tx) * (1 - ty) + (sd[i + W] * (1 - tx) + sd[i + W + 1] * tx) * ty;
  };
  // Extrusion with a rounded bevel: shrink the outline and depth by the bevel, then round.
  const f = (x: number, y: number, z: number) => {
    const a = field2(x, y) + bevel, b = Math.abs(z) - depth / 2 + bevel;
    return Math.min(Math.max(a, b), 0) + Math.hypot(Math.max(a, 0), Math.max(b, 0)) - bevel;
  };
  return sdfMesh(f, { bounds: [hw, hh, depth / 2 + bevel + (hw * 2) / (o.resolution ?? 160) * 2], resolution: o.resolution ?? 160 });
}
