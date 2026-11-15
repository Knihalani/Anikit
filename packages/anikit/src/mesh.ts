/**
 * Solid 3D: lit, z-buffered, instanced meshes rendered in WebAssembly.
 *
 * ```ts
 * const cube = mesh.cube();
 * const cubes = instances(5000);
 * for (let i = 0; i < 5000; i++) cubes.set(i, { x, y, z, rx, ry, scale: 12, color: '#ff3b14' });
 * const r3 = renderer3d(canvas);                 // or renderer3d({ width, height })
 * engine.onFrame(() => r3.render(scene, [{ mesh: cube, instances: cubes }], { background: '#050505', fog: [800, 4000] }));
 * ```
 *
 * Per-vertex Blinn–Phong with a key light, a fill light, a fresnel rim and
 * depth fog; per-pixel depth test; back-face culling. Tens of thousands of
 * triangles per frame at 1080p on the CPU — no WebGL.
 */
import { core, F64, f64Buf, W } from './wasm';
import { parseColor } from './values';
import { track } from './context';
import type { Scene3D } from './scene';

type V3 = [number, number, number];
const STRIDE = 15;

function rgb(c: string | V3 | undefined, d: V3 = [1, 1, 1]): V3 {
  if (!c) return d;
  if (Array.isArray(c)) return c;
  const p = parseColor(c);
  return p ? [p[0] / 255, p[1] / 255, p[2] / 255] : d;
}

/** A mesh living in WebAssembly memory. */
export class Mesh3D {
  constructor(readonly id: number) {}
  get vertices(): number {
    return core().ak_mesh_ptr(this.id, 3);
  }
  get triangles(): number {
    return core().ak_mesh_ptr(this.id, 4);
  }
  free(): void {
    core().ak_mesh_free(this.id);
  }
}

const builtin = (kind: number, params: number[] = []) => {
  const at = f64Buf(Math.max(1, params.length));
  F64.set(params, at);
  return new Mesh3D(core().ak_mesh_builtin(kind));
};

export const mesh = {
  /** Unit cube centred on the origin, flat-shaded. */
  cube: () => builtin(0),
  /** Unit sphere. */
  sphere: (rings = 16, segments = 24) => builtin(1, [rings, segments]),
  /** A (p, q) torus knot tube. */
  torusKnot: (o: { p?: number; q?: number; radius?: number; spread?: number; tube?: number; segments?: number; sides?: number } = {}) =>
    builtin(2, [o.p ?? 2, o.q ?? 5, o.radius ?? 300, o.spread ?? 120, o.tube ?? 40, o.segments ?? 360, o.sides ?? 16]),
  /** A torus in the XY plane. */
  torus: (radius = 300, tube = 60, segments = 96, sides = 24) => builtin(3, [radius, tube, segments, sides]),
  /** Your own geometry: xyz positions and normals per vertex, three indices per triangle. */
  custom(positions: ArrayLike<number>, normals: ArrayLike<number>, indices: ArrayLike<number>): Mesh3D {
    const w = core();
    const id = w.ak_mesh_custom(positions.length / 3, indices.length / 3);
    const mem = W!.memory.buffer;
    new Float32Array(mem, w.ak_mesh_ptr(id, 0), positions.length).set(positions);
    new Float32Array(mem, w.ak_mesh_ptr(id, 1), normals.length).set(normals);
    new Uint32Array(mem, w.ak_mesh_ptr(id, 2), indices.length).set(indices);
    return new Mesh3D(id);
  },
};

export interface InstanceInit {
  x?: number;
  y?: number;
  z?: number;
  /** Euler rotation in degrees (applied X, then Y, then Z). */
  rx?: number;
  ry?: number;
  rz?: number;
  scale?: number;
  sx?: number;
  sy?: number;
  sz?: number;
  color?: string | V3;
}

/** Per-instance transforms and colours for one draw. */
export class Instances {
  readonly data: Float32Array;
  constructor(public count: number) {
    this.data = new Float32Array(count * STRIDE);
    for (let i = 0; i < count; i++) this.set(i, {});
  }
  /** Set instance `i` (unspecified fields default: origin, no rotation, scale 1, white). */
  set(i: number, o: InstanceInit): this {
    const d = this.data, b = i * STRIDE, r = Math.PI / 180;
    const [cx, sx_] = [Math.cos((o.rx ?? 0) * r), Math.sin((o.rx ?? 0) * r)];
    const [cy, sy_] = [Math.cos((o.ry ?? 0) * r), Math.sin((o.ry ?? 0) * r)];
    const [cz, sz_] = [Math.cos((o.rz ?? 0) * r), Math.sin((o.rz ?? 0) * r)];
    const s = o.scale ?? 1, kx = (o.sx ?? 1) * s, ky = (o.sy ?? 1) * s, kz = (o.sz ?? 1) * s;
    // R = Rz · Ry · Rx (columns), then scale per axis.
    const m00 = cz * cy, m10 = sz_ * cy, m20 = -sy_;
    const m01 = cz * sy_ * sx_ - sz_ * cx, m11 = sz_ * sy_ * sx_ + cz * cx, m21 = cy * sx_;
    const m02 = cz * sy_ * cx + sz_ * sx_, m12 = sz_ * sy_ * cx - cz * sx_, m22 = cy * cx;
    d[b] = m00 * kx; d[b + 1] = m10 * kx; d[b + 2] = m20 * kx;
    d[b + 3] = m01 * ky; d[b + 4] = m11 * ky; d[b + 5] = m21 * ky;
    d[b + 6] = m02 * kz; d[b + 7] = m12 * kz; d[b + 8] = m22 * kz;
    d[b + 9] = o.x ?? 0; d[b + 10] = o.y ?? 0; d[b + 11] = o.z ?? 0;
    const c = rgb(o.color);
    d[b + 12] = c[0]; d[b + 13] = c[1]; d[b + 14] = c[2];
    return this;
  }
}

/** Create an instance buffer (one instance = identity, white, unless set). */
export function instances(count: number, init?: (i: number) => InstanceInit): Instances {
  const n = new Instances(count);
  if (init) for (let i = 0; i < count; i++) n.set(i, init(i));
  return n;
}

export interface MaterialOptions {
  ambient?: number;
  diffuse?: number;
  specular?: number;
  shininess?: number;
  rim?: number;
  /** Unlit self-illumination (×base colour): glows under the film grade's bloom. */
  emissive?: number;
  /** Chrome: mirror reflection of the sky/horizon/ground environment (0–1). */
  metal?: number;
  /** Thin-film rainbow sheen by viewing angle (0–1), and its hue offset. */
  iridescence?: number;
  hue?: number;
}
export interface LightOptions {
  key?: V3;
  keyColor?: string | V3;
  fill?: V3;
  fillColor?: string | V3;
  rimColor?: string | V3;
  /** Fog from `[near, far]` distance to `fogColor` (default: the background). */
  fog?: [number, number];
  fogColor?: string | V3;
  /** Environment reflected by metal materials. */
  sky?: string | V3;
  horizon?: string | V3;
  ground?: string | V3;
}
export interface DrawItem {
  mesh: Mesh3D;
  /** Instances, or omit for one untransformed white instance. */
  instances?: Instances;
  material?: MaterialOptions;
  /** Back-face culling (default true). */
  cull?: boolean;
}
type CanvasLike = { width: number; height: number; getContext(t: '2d'): CanvasRenderingContext2D | null };

export class Renderer3D {
  width: number;
  height: number;
  /** Triangles rasterised by the last render. */
  triangles = 0;
  /** Milliseconds the last render took. */
  lastMs = 0;
  private id: number;
  private img: ImageData | null = null;
  private single = new Instances(1);

  constructor(private target: CanvasLike | { width: number; height: number }) {
    this.width = target.width;
    this.height = target.height;
    this.id = core().ak_raster_create(this.width, this.height);
    track(this);
  }

  resize(w: number, h: number): this {
    this.width = w;
    this.height = h;
    core().ak_raster_resize(this.id, w, h);
    this.img = null;
    return this;
  }

  /**
   * Render `items` through `scene`'s camera. Draws into the target canvas
   * (when it has one) and returns the RGBA pixels (a view into WebAssembly
   * memory, valid until the next call).
   */
  render(scene: Scene3D, items: DrawItem[], o: LightOptions & { background?: string; transparent?: boolean } = {}): Uint8ClampedArray {
    const t0 = performance.now();
    const w = core();
    const bg = rgb(o.background, [0, 0, 0]);
    const hex = (Math.round(bg[0] * 255) << 16) | (Math.round(bg[1] * 255) << 8) | Math.round(bg[2] * 255);
    w.ak_raster_clear(this.id, hex, o.transparent ? 0 : 255);
    const sw = scene.width, sh = scene.height;
    scene.width = this.width; scene.height = this.height;
    const { matrix, eye } = scene.viewProjection();
    scene.width = sw; scene.height = sh;
    this.triangles = 0;
    for (const it of items) {
      const inst = it.instances ?? this.single;
      const ptr = w.ak_raster_instances(this.id, inst.count);
      new Float32Array(W!.memory.buffer, ptr, inst.count * STRIDE).set(inst.data.subarray(0, inst.count * STRIDE));
      const m = it.material ?? {};
      const kc = rgb(o.keyColor), fc = rgb(o.fillColor, [0.25, 0.28, 0.35]), rc = rgb(o.rimColor), fog = o.fog ?? [0, 0], fogc = rgb(o.fogColor, bg);
      const at = f64Buf(59);
      F64.set([...matrix, ...eye, m.ambient ?? 0.22, m.diffuse ?? 0.85, m.specular ?? 0.5, m.shininess ?? 40, m.rim ?? 0.35,
        ...(o.key ?? [0.4, 0.8, 0.6]), ...kc, ...(o.fill ?? [-0.6, -0.2, 0.4]), ...fc, ...rc, fog[0], fog[1], ...fogc,
        m.emissive ?? 0, m.metal ?? 0, m.iridescence ?? 0, m.hue ?? 0, ...rgb(o.sky, [0.25, 0.3, 0.55]), ...rgb(o.horizon, [1, 0.85, 0.7]), ...rgb(o.ground, [0.05, 0.05, 0.07])], at);
      this.triangles += w.ak_raster_draw(this.id, it.mesh.id, inst.count, it.cull === false ? 0 : 1);
    }
    const px = new Uint8ClampedArray(W!.memory.buffer, w.ak_raster_ptr(this.id), this.width * this.height * 4);
    const ctx = 'getContext' in this.target ? this.target.getContext('2d') : null;
    if (ctx) {
      if (!this.img || this.img.data.buffer !== W!.memory.buffer || (this.img.data as Uint8ClampedArray).byteOffset !== px.byteOffset) this.img = new ImageData(px, this.width, this.height);
      // Re-wrap each frame for canvases that copy (native Node canvases).
      else this.img = new ImageData(px, this.width, this.height);
      ctx.putImageData(this.img, 0, 0);
    }
    this.lastMs = performance.now() - t0;
    return px;
  }

  destroy(): void {
    if (this.id >= 0) core().ak_raster_free(this.id);
    this.id = -1;
  }
  kill(): void {
    this.destroy();
  }
  revert(): void {
    this.destroy();
  }
}

/** A solid 3D renderer for a canvas (or an offscreen `{ width, height }` target). */
export function renderer3d(target: CanvasLike | { width: number; height: number }): Renderer3D {
  return new Renderer3D(target);
}
