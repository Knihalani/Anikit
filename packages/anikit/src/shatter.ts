/**
 * Shatter any mesh into flying shards: every triangle becomes one instance of
 * a unit triangle, mapped onto it by its instance transform, then thrown
 * outward with spin, drag and gravity. Rendered by `renderer3d` like any
 * instanced mesh (use `cull: false`, shards tumble).
 *
 * ```ts
 * const shards = shatter(knot, { transform: { rx: 20, scale: 1.2 }, force: 900, spin: 540 });
 * // in draw(): shards.update(t - 3.2) — seconds since the hit (≤ 0 = intact)
 * items.push({ mesh: shards.mesh, instances: shards.instances, cull: false, material: { metal: 0.9 } });
 * ```
 */
import { core, W } from './wasm';
import { mesh, instances, Instances, type Mesh3D, type InstanceInit } from './mesh';

type V3 = [number, number, number];

export interface ShatterOptions {
  /** Where the blast comes from (default: the mesh's centre after `transform`). */
  center?: V3;
  /** Outward speed in units/s (default 600), and its random spread 0–1 (default 0.5). */
  force?: number;
  spread?: number;
  /** Tumble in degrees/s (default 360). */
  spin?: number;
  /** Downward acceleration in units/s² (default 0). */
  gravity?: number;
  /** Air drag per second (default 1.2): shards slow down and hang. */
  drag?: number;
  /** Placement of the source mesh (like an instance). */
  transform?: InstanceInit;
  /** Shard colours: one colour, or per shard (default: `transform.color` or white). */
  color?: V3 | ((i: number, centroid: V3) => V3);
  /** Delay before each shard flies, by distance from the centre (s per 1000 units; default 0). */
  ripple?: number;
}

let tri: Mesh3D | null = null;

export class Shards {
  /** The unit triangle every shard instances. */
  readonly mesh: Mesh3D;
  readonly instances: Instances;
  readonly count: number;
  private base: Float32Array; // per shard: p0(3) e1(3) e2(3) n(3) centroid(3)
  private motion: Float32Array; // per shard: dir(3) speed, axis(3) rate, delay
  private o: Required<Omit<ShatterOptions, 'center' | 'transform' | 'color'>>;

  constructor(source: Mesh3D, o: ShatterOptions = {}) {
    this.o = { force: o.force ?? 600, spread: o.spread ?? 0.5, spin: o.spin ?? 360, gravity: o.gravity ?? 0, drag: o.drag ?? 1.2, ripple: o.ripple ?? 0 };
    const w = core();
    const nv = w.ak_mesh_ptr(source.id, 3), nt = w.ak_mesh_ptr(source.id, 4);
    const pos = new Float32Array(W!.memory.buffer, w.ak_mesh_ptr(source.id, 0), nv * 3).slice();
    const idx = new Uint32Array(W!.memory.buffer, w.ak_mesh_ptr(source.id, 2), nt * 3).slice();
    const m = instances(1, () => o.transform ?? {}).data;
    const xf = (i: number): V3 => {
      const x = pos[i * 3], y = pos[i * 3 + 1], z = pos[i * 3 + 2];
      return [m[0] * x + m[3] * y + m[6] * z + m[9], m[1] * x + m[4] * y + m[7] * z + m[10], m[2] * x + m[5] * y + m[8] * z + m[11]];
    };
    this.count = nt;
    this.base = new Float32Array(nt * 15);
    this.motion = new Float32Array(nt * 9);
    const cents: V3[] = [];
    let cx = 0, cy = 0, cz = 0;
    for (let t = 0; t < nt; t++) {
      const a = xf(idx[t * 3]), b = xf(idx[t * 3 + 1]), c = xf(idx[t * 3 + 2]);
      const e1: V3 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]], e2: V3 = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
      let n: V3 = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
      const l = Math.hypot(n[0], n[1], n[2]) || 1;
      n = [n[0] / l, n[1] / l, n[2] / l];
      const ce: V3 = [(a[0] + b[0] + c[0]) / 3, (a[1] + b[1] + c[1]) / 3, (a[2] + b[2] + c[2]) / 3];
      cents.push(ce);
      cx += ce[0]; cy += ce[1]; cz += ce[2];
      this.base.set([...a, ...e1, ...e2, ...n, ...ce], t * 15);
    }
    const center: V3 = o.center ?? [cx / Math.max(1, nt), cy / Math.max(1, nt), cz / Math.max(1, nt)];
    // Deterministic per-shard randomness (films render identically in every worker).
    let seed = 0x9e3779b9 ^ nt;
    const rnd = () => ((seed = (Math.imul(seed ^ (seed >>> 15), 0x2c1b3c6d) + 0x6d2b79f5) >>> 0) / 4294967296);
    for (let t = 0; t < nt; t++) {
      const ce = cents[t];
      let d: V3 = [ce[0] - center[0], ce[1] - center[1], ce[2] - center[2]];
      const dist = Math.hypot(d[0], d[1], d[2]);
      const j = this.o.spread;
      d = [d[0] / (dist || 1) + (rnd() - 0.5) * j, d[1] / (dist || 1) + (rnd() - 0.5) * j, d[2] / (dist || 1) + (rnd() - 0.5) * j];
      const dl = Math.hypot(d[0], d[1], d[2]) || 1;
      let ax: V3 = [rnd() - 0.5, rnd() - 0.5, rnd() - 0.5];
      const al = Math.hypot(ax[0], ax[1], ax[2]) || 1;
      ax = [ax[0] / al, ax[1] / al, ax[2] / al];
      this.motion.set([d[0] / dl, d[1] / dl, d[2] / dl, this.o.force * (1 - j / 2 + rnd() * j), ...ax, this.o.spin * (0.4 + rnd() * 1.2), (dist / 1000) * this.o.ripple], t * 9);
    }
    tri ??= mesh.custom([0, 0, 0, 1, 0, 0, 0, 1, 0], [0, 0, 1, 0, 0, 1, 0, 0, 1], [0, 1, 2], [0, 0, 1, 0, 0, 1]);
    this.mesh = tri;
    this.instances = new Instances(nt);
    const col = o.color, fallback = o.transform?.color;
    const def: V3 = Array.isArray(fallback) ? (fallback as V3) : [1, 1, 1];
    const cd = this.instances.data;
    for (let t = 0; t < nt; t++) {
      const c = typeof col === 'function' ? col(t, cents[t]) : col ?? def;
      cd[t * 15 + 12] = c[0]; cd[t * 15 + 13] = c[1]; cd[t * 15 + 14] = c[2];
    }
    this.update(0);
  }

  /** Pose every shard `t` seconds after the hit (t ≤ 0: the intact mesh). */
  update(t: number): this {
    const B = this.base, M = this.motion, D = this.instances.data, { drag, gravity } = this.o;
    for (let i = 0; i < this.count; i++) {
      const b = i * 15, k = i * 9, o = i * 15;
      const tt = Math.max(0, t - M[k + 8]);
      // Distance travelled with linear drag: v·(1 − e^(−drag·t))/drag.
      const s = drag > 0 ? (M[k + 3] * (1 - Math.exp(-drag * tt))) / drag : M[k + 3] * tt;
      const dx = M[k] * s, dy = M[k + 1] * s - 0.5 * gravity * tt * tt, dz = M[k + 2] * s;
      // Rotation about the shard's centroid (Rodrigues).
      const ang = (M[k + 7] * tt * Math.PI) / 180, ux = M[k + 4], uy = M[k + 5], uz = M[k + 6];
      const c = Math.cos(ang), sn = Math.sin(ang), C = 1 - c;
      const r00 = c + ux * ux * C, r01 = ux * uy * C - uz * sn, r02 = ux * uz * C + uy * sn;
      const r10 = uy * ux * C + uz * sn, r11 = c + uy * uy * C, r12 = uy * uz * C - ux * sn;
      const r20 = uz * ux * C - uy * sn, r21 = uz * uy * C + ux * sn, r22 = c + uz * uz * C;
      const rot = (x: number, y: number, z: number, j: number) => {
        D[o + j] = r00 * x + r01 * y + r02 * z;
        D[o + j + 1] = r10 * x + r11 * y + r12 * z;
        D[o + j + 2] = r20 * x + r21 * y + r22 * z;
      };
      rot(B[b + 3], B[b + 4], B[b + 5], 0); // e1
      rot(B[b + 6], B[b + 7], B[b + 8], 3); // e2
      rot(B[b + 9], B[b + 10], B[b + 11], 6); // normal
      // Origin: R·(p0 − centroid) + centroid + offset.
      const px = B[b] - B[b + 12], py = B[b + 1] - B[b + 13], pz = B[b + 2] - B[b + 14];
      D[o + 9] = r00 * px + r01 * py + r02 * pz + B[b + 12] + dx;
      D[o + 10] = r10 * px + r11 * py + r12 * pz + B[b + 13] + dy;
      D[o + 11] = r20 * px + r21 * py + r22 * pz + B[b + 14] + dz;
    }
    return this;
  }
}

/** Break `source` into per-triangle shards (see {@link Shards}). */
export function shatter(source: Mesh3D, options?: ShatterOptions): Shards {
  return new Shards(source, options);
}
