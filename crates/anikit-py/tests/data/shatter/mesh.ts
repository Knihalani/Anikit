// Stub of mesh.ts for running shatter.ts under Node: the real Instances.set
// maths (copied verbatim), array colours only, and a dummy unit triangle.
type V3 = [number, number, number];
const STRIDE = 15;
export type Mesh3D = { id: number };
export interface InstanceInit {
  x?: number; y?: number; z?: number; rx?: number; ry?: number; rz?: number;
  scale?: number; sx?: number; sy?: number; sz?: number; color?: V3;
}
export class Instances {
  readonly data: Float32Array;
  count: number;
  constructor(count: number) {
    this.count = count;
    this.data = new Float32Array(count * STRIDE);
    for (let i = 0; i < count; i++) this.set(i, {});
  }
  set(i: number, o: InstanceInit): this {
    const d = this.data, b = i * STRIDE, r = Math.PI / 180;
    const [cx, sx_] = [Math.cos((o.rx ?? 0) * r), Math.sin((o.rx ?? 0) * r)];
    const [cy, sy_] = [Math.cos((o.ry ?? 0) * r), Math.sin((o.ry ?? 0) * r)];
    const [cz, sz_] = [Math.cos((o.rz ?? 0) * r), Math.sin((o.rz ?? 0) * r)];
    const s = o.scale ?? 1, kx = (o.sx ?? 1) * s, ky = (o.sy ?? 1) * s, kz = (o.sz ?? 1) * s;
    const m00 = cz * cy, m10 = sz_ * cy, m20 = -sy_;
    const m01 = cz * sy_ * sx_ - sz_ * cx, m11 = sz_ * sy_ * sx_ + cz * cx, m21 = cy * sx_;
    const m02 = cz * sy_ * cx + sz_ * sx_, m12 = sz_ * sy_ * cx - cz * sx_, m22 = cy * cx;
    d[b] = m00 * kx; d[b + 1] = m10 * kx; d[b + 2] = m20 * kx;
    d[b + 3] = m01 * ky; d[b + 4] = m11 * ky; d[b + 5] = m21 * ky;
    d[b + 6] = m02 * kz; d[b + 7] = m12 * kz; d[b + 8] = m22 * kz;
    d[b + 9] = o.x ?? 0; d[b + 10] = o.y ?? 0; d[b + 11] = o.z ?? 0;
    const c = o.color ?? [1, 1, 1];
    d[b + 12] = c[0]; d[b + 13] = c[1]; d[b + 14] = c[2];
    return this;
  }
}
export function instances(count: number, init?: (i: number) => InstanceInit): Instances {
  const n = new Instances(count);
  if (init) for (let i = 0; i < count; i++) n.set(i, init(i));
  return n;
}
export const mesh = { custom: (..._a: unknown[]): Mesh3D => ({ id: 1 }) };
