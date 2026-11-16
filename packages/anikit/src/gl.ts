/**
 * `scene()`: a real-time WebGL2 3D layer. One call gives a camera, procedural
 * image-based lighting, PBR materials, PCF shadows and optional bloom / DOF /
 * FXAA post, driven by anikit's single rAF loop. Object transforms and
 * material numbers are plain properties, so `animate()`, springs and
 * timelines drive them directly.
 *
 * ```ts
 * const s = scene('#c', { camera: { fov: 45, position: [0, 2, 6] }, environment: 'studio', shadows: true, bloom: true });
 * const ball = s.add(sphere(1, 64), { material: 'chrome', position: [0, 1, 0] });
 * s.add(plane(20, 20), { material: { color: '#222', roughness: 0.9 } });
 * ball.on('click', () => animate(ball.position, { y: 2 }, { duration: 0.6, ease: 'outBack' }));
 * s.orbit({ autoRotate: 10 });
 * ```
 */
import { engine } from './core';
import { track } from './context';
import { checkKeys, warn } from './debug';
import type { OptionKeys } from './debug';
import type { Geometry } from './geometry';
import * as G from './geometry';
import type { FlightPath } from './flight';

// ───────────────────────────── maths (column-major) ─────────────────────────────

type M4 = Float32Array;
const m4 = (): M4 => { const m = new Float32Array(16); m[0] = m[5] = m[10] = m[15] = 1; return m; };
function mul(o: M4, a: ArrayLike<number>, b: ArrayLike<number>): M4 {
  for (let c = 0; c < 4; c++) {
    const b0 = b[c * 4], b1 = b[c * 4 + 1], b2 = b[c * 4 + 2], b3 = b[c * 4 + 3];
    o[c * 4] = a[0] * b0 + a[4] * b1 + a[8] * b2 + a[12] * b3;
    o[c * 4 + 1] = a[1] * b0 + a[5] * b1 + a[9] * b2 + a[13] * b3;
    o[c * 4 + 2] = a[2] * b0 + a[6] * b1 + a[10] * b2 + a[14] * b3;
    o[c * 4 + 3] = a[3] * b0 + a[7] * b1 + a[11] * b2 + a[15] * b3;
  }
  return o;
}
function invert(o: M4, m: ArrayLike<number>): boolean {
  const a00 = m[0], a01 = m[1], a02 = m[2], a03 = m[3], a10 = m[4], a11 = m[5], a12 = m[6], a13 = m[7];
  const a20 = m[8], a21 = m[9], a22 = m[10], a23 = m[11], a30 = m[12], a31 = m[13], a32 = m[14], a33 = m[15];
  const b00 = a00 * a11 - a01 * a10, b01 = a00 * a12 - a02 * a10, b02 = a00 * a13 - a03 * a10, b03 = a01 * a12 - a02 * a11;
  const b04 = a01 * a13 - a03 * a11, b05 = a02 * a13 - a03 * a12, b06 = a20 * a31 - a21 * a30, b07 = a20 * a32 - a22 * a30;
  const b08 = a20 * a33 - a23 * a30, b09 = a21 * a32 - a22 * a31, b10 = a21 * a33 - a23 * a31, b11 = a22 * a33 - a23 * a32;
  let det = b00 * b11 - b01 * b10 + b02 * b09 + b03 * b08 - b04 * b07 + b05 * b06;
  if (!det) return false;
  det = 1 / det;
  o[0] = (a11 * b11 - a12 * b10 + a13 * b09) * det; o[1] = (a02 * b10 - a01 * b11 - a03 * b09) * det;
  o[2] = (a31 * b05 - a32 * b04 + a33 * b03) * det; o[3] = (a22 * b04 - a21 * b05 - a23 * b03) * det;
  o[4] = (a12 * b08 - a10 * b11 - a13 * b07) * det; o[5] = (a00 * b11 - a02 * b08 + a03 * b07) * det;
  o[6] = (a32 * b02 - a30 * b05 - a33 * b01) * det; o[7] = (a20 * b05 - a22 * b02 + a23 * b01) * det;
  o[8] = (a10 * b10 - a11 * b08 + a13 * b06) * det; o[9] = (a01 * b08 - a00 * b10 - a03 * b06) * det;
  o[10] = (a30 * b04 - a31 * b02 + a33 * b00) * det; o[11] = (a21 * b02 - a20 * b04 - a23 * b00) * det;
  o[12] = (a11 * b07 - a10 * b09 - a12 * b06) * det; o[13] = (a00 * b09 - a01 * b07 + a02 * b06) * det;
  o[14] = (a31 * b01 - a30 * b03 - a32 * b00) * det; o[15] = (a20 * b03 - a21 * b01 + a22 * b00) * det;
  return true;
}
/** Compose translation × rotation (XYZ Euler, radians) × scale. */
function compose(o: M4, p: Vector3, r: Vector3, s: Vector3): M4 {
  const cx = Math.cos(r.x), sx = Math.sin(r.x), cy = Math.cos(r.y), sy = Math.sin(r.y), cz = Math.cos(r.z), sz = Math.sin(r.z);
  // R = Rz * Ry * Rx (intrinsic XYZ applied x first)
  const r00 = cy * cz, r01 = sx * sy * cz - cx * sz, r02 = cx * sy * cz + sx * sz;
  const r10 = cy * sz, r11 = sx * sy * sz + cx * cz, r12 = cx * sy * sz - sx * cz;
  const r20 = -sy, r21 = sx * cy, r22 = cx * cy;
  o[0] = r00 * s.x; o[1] = r10 * s.x; o[2] = r20 * s.x; o[3] = 0;
  o[4] = r01 * s.y; o[5] = r11 * s.y; o[6] = r21 * s.y; o[7] = 0;
  o[8] = r02 * s.z; o[9] = r12 * s.z; o[10] = r22 * s.z; o[11] = 0;
  o[12] = p.x; o[13] = p.y; o[14] = p.z; o[15] = 1;
  return o;
}
function lookAt(o: M4, e: Vector3, t: Vector3, roll: number): M4 {
  let zx = e.x - t.x, zy = e.y - t.y, zz = e.z - t.z;
  let l = Math.hypot(zx, zy, zz) || 1; zx /= l; zy /= l; zz /= l;
  let ux = Math.sin(roll), uy = Math.cos(roll), uz = 0;
  if (Math.abs(zy) > 0.9999) { ux = 0; uy = 0; uz = -1; }
  let xx = uy * zz - uz * zy, xy = uz * zx - ux * zz, xz = ux * zy - uy * zx;
  l = Math.hypot(xx, xy, xz) || 1; xx /= l; xy /= l; xz /= l;
  const yx = zy * xz - zz * xy, yy = zz * xx - zx * xz, yz = zx * xy - zy * xx;
  o[0] = xx; o[1] = yx; o[2] = zx; o[3] = 0; o[4] = xy; o[5] = yy; o[6] = zy; o[7] = 0;
  o[8] = xz; o[9] = yz; o[10] = zz; o[11] = 0;
  o[12] = -(xx * e.x + xy * e.y + xz * e.z); o[13] = -(yx * e.x + yy * e.y + yz * e.z); o[14] = -(zx * e.x + zy * e.y + zz * e.z); o[15] = 1;
  return o;
}

// ───────────────────────────── public value types ─────────────────────────────

/** A mutable 3-vector; animate its `x`/`y`/`z` with `animate(v, { y: 2 })`. */
export class Vector3 {
  constructor(public x = 0, public y = 0, public z = 0) {}
  /** Set all three components; returns `this`. */
  set(x: number, y = x, z = x): this { this.x = x; this.y = y; this.z = z; return this; }
  /** Copy from an `[x, y, z]` array or another vector. */
  copy(v: Vec3Like): this { return Array.isArray(v) ? this.set(v[0], v[1], v[2]) : this.set((v as Vector3).x, (v as Vector3).y, (v as Vector3).z); }
}
/** `[x, y, z]` or `{ x, y, z }`. */
export type Vec3Like = readonly [number, number, number] | { x: number; y: number; z: number };
const vec = (v: Vec3Like | number | undefined, d: number): Vector3 =>
  v === undefined ? new Vector3(d, d, d) : typeof v === 'number' ? new Vector3(v, v, v) : new Vector3().copy(v);

/** Parse `#rgb`, `#rrggbb` or `[r, g, b]` (0–1, sRGB) into linear RGB. */
export function linearColor(c: string | readonly number[], out: number[] = [0, 0, 0]): number[] {
  let r = 1, g = 1, b = 1;
  if (typeof c === 'string') {
    let h = c.trim().replace('#', '');
    if (h.length === 3) h = h[0] + h[0] + h[1] + h[1] + h[2] + h[2];
    const n = parseInt(h.slice(0, 6), 16);
    if (Number.isNaN(n)) warn(`anikit: scene(): cannot parse colour "${c}" (use "#rrggbb")`);
    else { r = ((n >> 16) & 255) / 255; g = ((n >> 8) & 255) / 255; b = (n & 255) / 255; }
  } else { r = c[0]; g = c[1]; b = c[2]; }
  const lin = (x: number) => (x <= 0.04045 ? x / 12.92 : Math.pow((x + 0.055) / 1.055, 2.4));
  out[0] = lin(r); out[1] = lin(g); out[2] = lin(b);
  return out;
}

// ───────────────────────────── materials ─────────────────────────────

/** Built-in material looks. */
export type MaterialPreset = 'chrome' | 'gold' | 'copper' | 'glass' | 'plastic' | 'ceramic' | 'rubber' | 'emissive' | 'toon';

/** Material options (all optional; start from `preset`). */
export interface SceneMaterialOptions {
  /** Start from a preset, then apply the other keys. */
  preset?: MaterialPreset;
  /** Base colour (`#rrggbb` or sRGB `[r, g, b]` in 0–1). */
  color?: string | readonly number[];
  /** 0 = dielectric, 1 = metal. */
  metalness?: number;
  /** 0 = mirror, 1 = fully rough. */
  roughness?: number;
  /** Emitted colour (glows with bloom). */
  emissive?: string | readonly number[];
  /** Multiplier on `emissive` (HDR: values > 1 bloom). */
  emissiveIntensity?: number;
  /** 0–1. Below 1 the object is drawn in the transparent pass. */
  opacity?: number;
  /** Cel-shaded bands and a rim light instead of PBR. */
  toon?: boolean;
  /** Glass: Fresnel-weighted transparency with refraction-tinted environment. */
  glass?: boolean;
  /**
   * Colour texture, multiplied into `color` (and `emissive`, so a black, emissive material shows it unlit like a screen).
   * A canvas or image (uploaded once), or a live source with `canvas` + `version` such as `htmlTexture(el)` /
   * `htmlCanvas(el)` (re-uploaded only when `version` changes; pointer events on the mesh are forwarded to the element).
   * UV (0, 0) is the image's top-left.
   */
  map?: MapSource | null;
}
/** A texture source for {@link SceneMaterialOptions.map}. */
export type MapSource = HTMLCanvasElement | HTMLImageElement | ImageBitmap | { readonly canvas: HTMLCanvasElement; readonly version: number; readonly element?: Element; readonly width?: number; readonly height?: number };
const MATERIAL_KEYS: OptionKeys<SceneMaterialOptions> = {
  preset: 0, color: 0, metalness: 0, roughness: 0, emissive: 0, emissiveIntensity: 0, opacity: 0, toon: 0, glass: 0, map: 0,
};
const PRESETS: Record<MaterialPreset, SceneMaterialOptions> = {
  chrome: { color: '#f4f5f7', metalness: 1, roughness: 0.06 },
  gold: { color: '#ffd27a', metalness: 1, roughness: 0.22 },
  copper: { color: '#f2a37c', metalness: 1, roughness: 0.3 },
  glass: { color: '#ffffff', metalness: 0, roughness: 0.03, opacity: 0.12, glass: true },
  plastic: { color: '#e8e8e8', metalness: 0, roughness: 0.38 },
  ceramic: { color: '#f3efe8', metalness: 0, roughness: 0.12 },
  rubber: { color: '#2a2a2e', metalness: 0, roughness: 0.92 },
  emissive: { color: '#000000', emissive: '#7fd4ff', emissiveIntensity: 4, roughness: 1 },
  toon: { color: '#ff7a59', roughness: 0.5, toon: true },
};

let materialIds = 0;
/**
 * A PBR (metal/roughness) material. Numeric fields are plain properties, so
 * `animate(m, { roughness: 0.1, emissiveIntensity: 6 })` just works; colours
 * live in linear RGB arrays (`m.set({ color: '#f80' })` to change them).
 */
export class Material {
  /** Linear base colour. */
  color = [1, 1, 1];
  /** 0 = dielectric, 1 = metal. */
  metalness = 0;
  /** 0 = mirror, 1 = fully rough. */
  roughness = 0.5;
  /** Linear emissive colour. */
  emissive = [0, 0, 0];
  /** Multiplier on `emissive`. */
  emissiveIntensity = 1;
  /** Opacity; below 1 the object draws in the sorted transparent pass. */
  opacity = 1;
  /** Cel shading. */
  toon = false;
  /** Glass shading. */
  glass = false;
  /** Colour texture (see {@link SceneMaterialOptions.map}). */
  map: MapSource | null = null;
  /** Stable id (used for state sorting). */
  readonly id = ++materialIds;
  constructor(o: SceneMaterialOptions | MaterialPreset = {}) { this.set(typeof o === 'string' ? { preset: o } : o); }
  /** Apply options (a preset first, then the rest). */
  set(o: SceneMaterialOptions): this {
    checkKeys('material', o, MATERIAL_KEYS);
    if (o.preset) {
      const p = PRESETS[o.preset];
      if (!p) warn(`anikit: material(): unknown preset "${o.preset}" (one of ${Object.keys(PRESETS).join(', ')})`);
      else this.set(p);
    }
    if (o.color !== undefined) linearColor(o.color, this.color);
    if (o.emissive !== undefined) linearColor(o.emissive, this.emissive);
    if (o.metalness !== undefined) this.metalness = o.metalness;
    if (o.roughness !== undefined) this.roughness = o.roughness;
    if (o.emissiveIntensity !== undefined) this.emissiveIntensity = o.emissiveIntensity;
    if (o.opacity !== undefined) this.opacity = o.opacity;
    if (o.toon !== undefined) this.toon = o.toon;
    if (o.glass !== undefined) this.glass = o.glass;
    if (o.map !== undefined) this.map = o.map;
    return this;
  }
}
/** Create a {@link Material} from a preset name or options. */
export function material(o: SceneMaterialOptions | MaterialPreset = {}): Material { return new Material(o); }

// ───────────────────────────── objects ─────────────────────────────

/** Pointer event delivered to `obj.on(...)` handlers. */
export interface ScenePointerEvent {
  /** The object hit. */
  object: Object3D;
  /** Instance index for instanced objects (else -1). */
  instance: number;
  /** World-space hit point. */
  point: [number, number, number];
  /** Distance from the camera. */
  distance: number;
  /** Texture coordinate at the hit (meshes only; (0, 0) = image top-left). */
  uv: [number, number] | null;
  /** The DOM event that caused it. */
  event: PointerEvent | MouseEvent;
}
/** Events objects can listen to. */
export type ScenePointerType = 'click' | 'pointerdown' | 'pointerup' | 'pointerenter' | 'pointerleave' | 'pointermove';

/** Options for `scene.add(geometry, options)`. */
export interface ObjectOptions {
  /** Position (default origin). */
  position?: Vec3Like;
  /** Euler rotation in radians, XYZ order. */
  rotation?: Vec3Like;
  /** Uniform or per-axis scale. */
  scale?: Vec3Like | number;
  /** A preset name, options, or a shared {@link Material}. */
  material?: MaterialPreset | SceneMaterialOptions | Material;
  /** Casts a shadow (default true). */
  castShadow?: boolean;
  /** Visible (default true). */
  visible?: boolean;
  /** Parent object (transforms compose). */
  parent?: Object3D;
}
const OBJECT_KEYS: OptionKeys<ObjectOptions> = { position: 0, rotation: 0, scale: 0, material: 0, castShadow: 0, visible: 0, parent: 0 };

interface GpuGeometry {
  id: number; vao: WebGLVertexArrayObject; count: number; type: number;
  buffers: WebGLBuffer[]; uvBuffer: WebGLBuffer | null; geometry: Geometry; center: [number, number, number]; radius: number;
}

/**
 * A node in the scene: transform (`position`, `rotation`, `scale` as
 * animatable {@link Vector3}s), optional mesh and material, children, and
 * pointer events (`obj.on('click', e => …)`).
 */
export class Object3D {
  /** Position. */
  readonly position: Vector3;
  /** Euler rotation (radians, XYZ). */
  readonly rotation: Vector3;
  /** Scale. */
  readonly scale: Vector3;
  /** Material (shared instances are fine). */
  material: Material;
  /** Hidden objects (and their children) are skipped. */
  visible: boolean;
  /** Casts a shadow. */
  castShadow: boolean;
  /** Children (transforms compose). */
  readonly children: Object3D[] = [];
  /** Parent, or null at the root. */
  parent: Object3D | null = null;
  /** When set, used as the local matrix instead of position/rotation/scale (glTF nodes). */
  matrix: Float32Array | null = null;
  /** World matrix, updated every frame. */
  readonly world: Float32Array = m4();
  /** Free slot for app data. */
  data: Record<string, unknown> = {};
  /** @internal */ gpu: GpuGeometry | null;
  /** @internal */ handlers: Map<string, Set<(e: ScenePointerEvent) => void>> | null = null;
  /** @internal */ scene: Scene | null = null;
  /** @internal squared camera distance (transparent sort) */ depth = 0;
  constructor(gpu: GpuGeometry | null, o: ObjectOptions, mat: Material) {
    this.gpu = gpu;
    this.position = vec(o.position, 0);
    this.rotation = vec(o.rotation, 0);
    this.scale = vec(o.scale, 1);
    this.material = mat;
    this.visible = o.visible ?? true;
    this.castShadow = o.castShadow ?? true;
  }
  /** Listen to a pointer event on this object; returns an unsubscribe fn. */
  on(type: ScenePointerType, fn: (e: ScenePointerEvent) => void): () => void {
    (this.handlers ??= new Map());
    let set = this.handlers.get(type);
    if (!set) this.handlers.set(type, (set = new Set()));
    set.add(fn);
    this.scene?.listen();
    return () => { set!.delete(fn); };
  }
  /** Attach `child` (removing it from its old parent). */
  add(child: Object3D): this {
    child.parent?.children.splice(child.parent.children.indexOf(child), 1);
    child.parent = this;
    this.children.push(child);
    return this;
  }
  /** Remove from the scene (and the parent). */
  remove(): void { this.scene?.remove(this); }
  /** Point the object's -Z axis at a world position (sets `rotation`). */
  lookAt(x: number, y: number, z: number): this {
    const dx = x - this.position.x, dy = y - this.position.y, dz = z - this.position.z;
    this.rotation.set(Math.atan2(dy, Math.hypot(dx, dz)), Math.atan2(-dx, -dz), 0);
    return this;
  }
}

/** Per-instance values set by the callback of `scene.instances()`. */
export interface InstanceState {
  /** Position. */
  position: Vector3;
  /** Euler rotation (radians). */
  rotation: Vector3;
  /** Scale. */
  scale: Vector3;
  /** Linear RGB tint (multiplies the material colour); write `[r, g, b]` items. */
  color: number[];
}

/**
 * Many copies of one mesh in a single draw call. Update them with
 * `inst.update((i, o) => { o.position.y = … })` (allocation-free) or write
 * `inst.matrices` / `inst.colors` directly and call `inst.needsUpdate()`.
 */
export class InstancedMesh extends Object3D {
  /** Instance count. */
  readonly count: number;
  /** `count` column-major 4×4 matrices. */
  readonly matrices: Float32Array;
  /** `count` linear RGB tints. */
  readonly colors: Float32Array;
  /** @internal */ dirty = true;
  /** @internal */ ibuf: WebGLBuffer | null = null;
  /** @internal */ ivao: WebGLVertexArrayObject | null = null;
  /** @internal */ bound = [0, 0, 0, 0];
  private st: InstanceState = { position: new Vector3(), rotation: new Vector3(), scale: new Vector3(1, 1, 1), color: [1, 1, 1] };
  private tmp = m4();
  constructor(gpu: GpuGeometry, count: number, o: ObjectOptions, mat: Material) {
    super(gpu, o, mat);
    this.count = count;
    this.matrices = new Float32Array(count * 16);
    this.colors = new Float32Array(count * 3).fill(1);
  }
  /** Re-run `fn(i, state)` for every instance (state is reset to identity / white first). */
  update(fn: (i: number, o: InstanceState) => void): this {
    const st = this.st;
    for (let i = 0; i < this.count; i++) {
      st.position.set(0); st.rotation.set(0); st.scale.set(1); st.color[0] = st.color[1] = st.color[2] = 1;
      fn(i, st);
      compose(this.tmp, st.position, st.rotation, st.scale);
      this.matrices.set(this.tmp, i * 16);
      this.colors[i * 3] = st.color[0]; this.colors[i * 3 + 1] = st.color[1]; this.colors[i * 3 + 2] = st.color[2];
    }
    return this.needsUpdate();
  }
  /** Flag `matrices`/`colors` for re-upload (and recompute the bounds). */
  needsUpdate(): this {
    this.dirty = true;
    const m = this.matrices, g = this.gpu!;
    let lx = Infinity, ly = Infinity, lz = Infinity, hx = -Infinity, hy = -Infinity, hz = -Infinity, s = 0;
    for (let i = 0; i < this.count; i++) {
      const o = i * 16, x = m[o + 12], y = m[o + 13], z = m[o + 14];
      if (x < lx) lx = x; if (y < ly) ly = y; if (z < lz) lz = z; if (x > hx) hx = x; if (y > hy) hy = y; if (z > hz) hz = z;
      const sc = Math.max(Math.hypot(m[o], m[o + 1], m[o + 2]), Math.hypot(m[o + 4], m[o + 5], m[o + 6]), Math.hypot(m[o + 8], m[o + 9], m[o + 10]));
      if (sc > s) s = sc;
    }
    if (!this.count) { lx = ly = lz = hx = hy = hz = 0; }
    this.bound[0] = (lx + hx) / 2; this.bound[1] = (ly + hy) / 2; this.bound[2] = (lz + hz) / 2;
    this.bound[3] = Math.hypot(hx - lx, hy - ly, hz - lz) / 2 + (g.radius + Math.hypot(...g.center)) * s;
    return this;
  }
}

// ───────────────────────────── scene options ─────────────────────────────

/** Procedural lighting environments (prefiltered analytically per roughness). */
export type EnvironmentName = 'studio' | 'sunset' | 'night';

/** Camera options. */
export interface CameraOptions {
  /** Vertical field of view in degrees (default 45). */
  fov?: number;
  /** Eye position (default `[0, 2, 6]`). */
  position?: Vec3Like;
  /** Look-at target (default origin). */
  target?: Vec3Like;
  /** Near plane (default 0.05). */
  near?: number;
  /** Far plane (default 200). */
  far?: number;
}
/** Bloom options. */
export interface BloomOptions {
  /** Strength (default 0.6). */
  strength?: number;
  /** HDR threshold (default 1). */
  threshold?: number;
}
/** Depth-of-field options. */
export interface DofOptions {
  /** Focus distance from the camera (default: the camera target distance). */
  focus?: number;
  /** Blur growth per unit away from focus (default 0.15). */
  aperture?: number;
}
/** Options for {@link scene}. */
export interface SceneInit {
  /** Camera setup. */
  camera?: CameraOptions;
  /** Lighting environment (default 'studio'). */
  environment?: EnvironmentName;
  /** Background: `'environment'` (default), a `#rrggbb` colour, or `'transparent'`. */
  background?: string;
  /** PCF shadow map from the key light (default true). */
  shadows?: boolean;
  /** Bloom (true or options). Turns on the HDR post chain. */
  bloom?: boolean | BloomOptions;
  /** Depth of field (true or options). Turns on the HDR post chain. */
  dof?: boolean | DofOptions;
  /** FXAA (default: on when the post chain runs; MSAA is used otherwise). */
  fxaa?: boolean;
  /** Exposure multiplier before ACES tonemapping (default 1). */
  exposure?: number;
  /** Max device pixel ratio (default 2). */
  dpr?: number;
  /** Adaptive resolution: lower the pixel ratio when frames run slow (default true). */
  adaptive?: boolean;
  /** Render automatically every frame through anikit's engine (default true). */
  autoRender?: boolean;
}
const SCENE_KEYS: OptionKeys<SceneInit> = {
  camera: 0, environment: 0, background: 0, shadows: 0, bloom: 0, dof: 0, fxaa: 0, exposure: 0, dpr: 0, adaptive: 0, autoRender: 0,
};
const CAMERA_KEYS: OptionKeys<CameraOptions> = { fov: 0, position: 0, target: 0, near: 0, far: 0 };

/** Orbit-control options. */
export interface OrbitOptions {
  /** Auto-rotation in degrees per second (default 0). */
  autoRotate?: number;
  /** Inertia: fraction of velocity kept per 1/60 s (default 0.92). */
  damping?: number;
  /** Wheel / pinch zoom (default true). */
  zoom?: boolean;
  /** Distance limits (default `[0.5, 100]`). */
  distance?: [number, number];
  /** Pitch limits in degrees (default `[-85, 85]`). */
  pitch?: [number, number];
}
const ORBIT_KEYS: OptionKeys<OrbitOptions> = { autoRotate: 0, damping: 0, zoom: 0, distance: 0, pitch: 0 };

/** Camera: animate `position`, `target`, `fov` and `roll` directly. */
export interface Camera {
  /** Eye position. */
  position: Vector3;
  /** Look-at target. */
  target: Vector3;
  /** Vertical field of view (degrees). */
  fov: number;
  /** Roll around the view axis (degrees). */
  roll: number;
  /** Near plane. */
  near: number;
  /** Far plane. */
  far: number;
}

/** What a custom layer gets each frame (see `scene.add(layer)`). */
export interface LayerContext {
  /** The WebGL2 context (the scene's framebuffer is bound). */
  gl: WebGL2RenderingContext;
  /** View matrix (column-major). */
  view: Float32Array;
  /** Projection matrix. */
  proj: Float32Array;
  /** Projection × view. */
  viewProj: Float32Array;
  /** Camera position. */
  eye: Vector3;
  /** Drawing-buffer size in pixels. */
  width: number;
  /** Drawing-buffer height in pixels. */
  height: number;
  /** True when rendering linear HDR (post chain on): don't tonemap in the layer. */
  hdr: boolean;
}
/** A custom renderer drawn inside the scene after opaque objects (e.g. a Gaussian-splat layer). */
export interface SceneLayer {
  /** Draw; must restore any GL state it changes except blend/depth flags. */
  render(ctx: LayerContext): void;
  /** Free GPU resources (called by `scene.revert()`). */
  dispose?(): void;
}

/** Result of {@link Scene.raycast}. */
export interface RayHit {
  /** Object hit. */
  object: Object3D;
  /** Instance index for instanced objects (else -1). */
  instance: number;
  /** Hit point (world). */
  point: [number, number, number];
  /** Distance from the camera. */
  distance: number;
  /** Texture coordinate at the hit (meshes only). */
  uv?: [number, number];
}

/** Live counters for the last frame. */
export interface SceneStats {
  /** Draw calls (all passes). */
  drawCalls: number;
  /** Triangles submitted in the main pass. */
  triangles: number;
  /** Objects culled by the frustum test. */
  culled: number;
  /** Smoothed time between frames (ms). */
  frameMs: number;
  /** Current device pixel ratio. */
  dpr: number;
}

// ───────────────────────────── shaders ─────────────────────────────

const UBO = `layout(std140) uniform Frame {
  mat4 uView; mat4 uProj; mat4 uLightVP; mat4 uInvVP;
  vec4 uCam; vec4 uSunDir; vec4 uSunCol; vec4 uSkyTop; vec4 uSkyHor; vec4 uGround; vec4 uExtra;
};`;
// uCam.w exposure · uSunDir.w intensity · uSunCol.w shadows · uSkyTop.w env id · uSkyHor.w tonemap · uGround.w env intensity · uExtra.x time
const ENV = `
const float PI = 3.14159265;
float band(float x, float e, float s) { return smoothstep(e - s, e + s, x); }
vec3 envMap(vec3 d, float r) {
  float id = uSkyTop.w;
  float s = 0.02 + r * r * 0.9;
  vec3 c = mix(uGround.rgb, uSkyHor.rgb, band(d.y, 0.0, s + 0.04));
  c = mix(c, uSkyTop.rgb, smoothstep(0.0, 0.9, d.y));
  float sd = max(dot(d, uSunDir.xyz), 0.0);
  float a = max(r * r, 0.002);
  float n = 2.0 / (a * a) - 2.0;
  float glow = pow(sd, min(n, 6000.0)) * min((n + 2.0) / 60.0, 40.0);
  if (id < 0.5) {
    // studio: overhead softbox + two strip lights + a floor bounce
    float top = band(d.y, 0.82, s) * (1.0 - band(abs(d.x), 0.45 + s, s));
    float strips = band(abs(d.x), 0.86, s) * (1.0 - band(abs(d.y), 0.35, s)) * (1.0 - band(abs(d.z), 0.5, s));
    float back = band(-d.z, 0.9, s) * (1.0 - band(abs(d.y), 0.25, s));
    c += vec3(1.0, 0.98, 0.95) * (top * 3.2 + strips * 2.2 + back * 0.8) * mix(1.0, 0.55, r);
  } else if (id < 1.5) {
    c += uSunCol.rgb * glow * 0.35 + uSunCol.rgb * pow(sd, 6.0) * 0.6;
  } else {
    c += vec3(0.75, 0.82, 1.0) * glow * 0.25;
  }
  return c;
}`;
const COMMON_VS = `#version 300 es
${UBO}
layout(location=0) in vec3 aPos; layout(location=1) in vec3 aNor; layout(location=2) in vec2 aUv;
#ifdef INSTANCED
layout(location=3) in vec4 aI0; layout(location=4) in vec4 aI1; layout(location=5) in vec4 aI2; layout(location=6) in vec4 aI3;
layout(location=7) in vec3 aTint;
#endif
uniform mat4 uModel;
out vec3 vPos; out vec3 vNor; out vec4 vShadow; out vec3 vTint; out vec2 vUv;
void main() {
  mat4 m = uModel; vec3 tint = vec3(1.0);
#ifdef INSTANCED
  m = uModel * mat4(aI0, aI1, aI2, aI3); tint = aTint;
#endif
  vec4 wp = m * vec4(aPos, 1.0);
#ifdef DEPTH
  gl_Position = uLightVP * wp;
#else
  vPos = wp.xyz; mat3 r = mat3(m); vNor = r * (aNor / vec3(dot(r[0], r[0]), dot(r[1], r[1]), dot(r[2], r[2]))); vShadow = uLightVP * wp; vTint = tint; vUv = aUv;
  gl_Position = uProj * uView * wp;
#endif
}`;
const DEPTH_FS = `#version 300 es
precision mediump float; out vec4 o; void main() { o = vec4(1.0); }`;
const PBR_FS = `#version 300 es
precision highp float; precision highp sampler2DShadow;
${UBO}
${ENV}
uniform vec4 uColor; uniform vec4 uPbr; uniform vec3 uEmissive;
uniform sampler2DShadow uShadow; uniform sampler2D uMap; uniform float uHasMap;
in vec3 vPos; in vec3 vNor; in vec4 vShadow; in vec3 vTint; in vec2 vUv;
out vec4 outColor;
float pcf(float ndl) {
  vec3 p = vShadow.xyz / vShadow.w * 0.5 + 0.5;
  if (p.z > 1.0 || p.x < 0.0 || p.y < 0.0 || p.x > 1.0 || p.y > 1.0) return 1.0;
  float bias = 0.0008 + 0.002 * (1.0 - ndl);
  vec2 t = 1.0 / vec2(textureSize(uShadow, 0));
  float s = 0.0;
  // 4 hardware-filtered taps (each a bilinear 2×2 compare): a smooth 4×4 PCF footprint.
  s += texture(uShadow, vec3(p.xy + vec2(-0.75, -0.75) * t, p.z - bias));
  s += texture(uShadow, vec3(p.xy + vec2(0.75, -0.75) * t, p.z - bias));
  s += texture(uShadow, vec3(p.xy + vec2(-0.75, 0.75) * t, p.z - bias));
  s += texture(uShadow, vec3(p.xy + vec2(0.75, 0.75) * t, p.z - bias));
  return s * 0.25;
}
vec3 aces(vec3 x) { return clamp((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14), 0.0, 1.0); }
void main() {
  vec3 N = normalize(vNor); if (!gl_FrontFacing) N = -N;
  vec3 V = normalize(uCam.xyz - vPos);
  vec4 tx = vec4(1.0);
  if (uHasMap > 0.5) { tx = texture(uMap, vUv); tx.rgb = pow(tx.rgb, vec3(2.2)); }
  vec3 albedo = uColor.rgb * vTint * tx.rgb;
  float metal = uPbr.x, rough = clamp(uPbr.y, 0.03, 1.0), flags = uPbr.w, opacity = uColor.a * tx.a;
  vec3 L = uSunDir.xyz, H = normalize(L + V);
  float NdL = max(dot(N, L), 0.0), NdV = max(dot(N, V), 1e-4), NdH = max(dot(N, H), 0.0), VdH = max(dot(V, H), 0.0);
  float sh = uSunCol.w > 0.5 ? pcf(NdL) : 1.0;
  vec3 col;
  if (flags > 0.5 && flags < 1.5) {
    float b = NdL * sh; b = b > 0.6 ? 1.0 : b > 0.2 ? 0.6 : 0.25;
    float rim = pow(1.0 - NdV, 3.0);
    col = albedo * (b * uSunCol.rgb * uSunDir.w * 0.45 + envMap(N, 1.0) * 0.35) + rim * 0.4 * envMap(N, 1.0);
    col += step(0.96, NdH) * sh * uSunCol.rgb * 0.6;
  } else {
    vec3 F0 = mix(vec3(0.04), albedo, metal);
    float a = rough * rough, a2 = a * a;
    float dn = NdH * NdH * (a2 - 1.0) + 1.0;
    float D = a2 / (PI * dn * dn);
    float k = (rough + 1.0) * (rough + 1.0) / 8.0;
    float Gm = (NdV / (NdV * (1.0 - k) + k)) * (NdL / (NdL * (1.0 - k) + k));
    vec3 F = F0 + (1.0 - F0) * pow(1.0 - VdH, 5.0);
    vec3 spec = D * Gm * F / (4.0 * NdV * max(NdL, 1e-4));
    vec3 kd = (1.0 - F) * (1.0 - metal);
    vec3 direct = (kd * albedo / PI + spec) * uSunCol.rgb * uSunDir.w * NdL * sh;
    vec4 c0 = vec4(-1.0, -0.0275, -0.572, 0.022), c1 = vec4(1.0, 0.0425, 1.04, -0.04);
    vec4 rr = rough * c0 + c1;
    float a004 = min(rr.x * rr.x, exp2(-9.28 * NdV)) * rr.x + rr.y;
    vec2 AB = vec2(-1.04, 1.04) * a004 + rr.zw;
    vec3 R = reflect(-V, N);
    vec3 specE = F0 * AB.x + AB.y;
    vec3 specI = envMap(R, rough) * specE;
    float ao = mix(0.55, 1.0, sh);
    vec3 diffI = envMap(N, 1.0) * albedo * (1.0 - metal) * (1.0 - specE) * ao;
    col = direct + (specI * mix(0.7, 1.0, sh) + diffI) * uGround.w;
    if (flags > 1.5) {
      vec3 T = refract(-V, N, 0.67);
      vec3 trans = envMap(T, rough + 0.05) * albedo;
      float fr = specE.x;
      col = specI * uGround.w + direct * 0.5 + trans * (1.0 - fr) * opacity * uGround.w;
      opacity = clamp(opacity + fr * 1.5, 0.0, 1.0);
    }
  }
  col += uEmissive * uPbr.z * tx.rgb;
  if (uSkyHor.w > 0.5) col = pow(aces(col * uCam.w), vec3(1.0 / 2.2));
  outColor = vec4(col * (flags > 1.5 ? 1.0 : opacity), opacity);
}`;
const FS_TRI = `#version 300 es
out vec2 vUv; void main() { vec2 p = vec2((gl_VertexID << 1) & 2, gl_VertexID & 2); vUv = p; gl_Position = vec4(p * 2.0 - 1.0, 0.0, 1.0); }`;
const SKY_FS = `#version 300 es
precision highp float;
${UBO}
${ENV}
in vec2 vUv; out vec4 o;
uniform float uBlur;
vec3 aces(vec3 x) { return clamp((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14), 0.0, 1.0); }
void main() {
  vec4 p = uInvVP * vec4(vUv * 2.0 - 1.0, 1.0, 1.0);
  vec3 d = normalize(p.xyz / p.w - uCam.xyz);
  vec3 c = envMap(d, uBlur);
  if (uSkyTop.w > 1.5 && d.y > 0.0) {
    vec3 q = floor(d * 300.0);
    float h = fract(sin(dot(q, vec3(12.9898, 78.233, 37.719))) * 43758.5453);
    c += step(0.9985, h) * vec3(1.6) * smoothstep(0.0, 0.3, d.y);
  }
  c *= uGround.w;
  if (uSkyHor.w > 0.5) c = pow(aces(c * uCam.w), vec3(1.0 / 2.2));
  o = vec4(c, 1.0);
}`;
const BRIGHT_FS = `#version 300 es
precision highp float; in vec2 vUv; out vec4 o; uniform sampler2D uTex; uniform vec2 uTexel; uniform float uThreshold;
void main() {
  vec3 c = (texture(uTex, vUv + uTexel * vec2(-1,-1)).rgb + texture(uTex, vUv + uTexel * vec2(1,-1)).rgb
          + texture(uTex, vUv + uTexel * vec2(-1,1)).rgb + texture(uTex, vUv + uTexel * vec2(1,1)).rgb) * 0.25;
  float l = max(c.r, max(c.g, c.b));
  float k = max(l - uThreshold, 0.0) / max(l, 1e-4);
  o = vec4(uThreshold < 0.0 ? c : min(c * k, vec3(64.0)), 1.0);
}`;
const BLUR_FS = `#version 300 es
precision highp float; in vec2 vUv; out vec4 o; uniform sampler2D uTex; uniform vec2 uDir;
void main() {
  vec3 c = texture(uTex, vUv).rgb * 0.227027;
  c += (texture(uTex, vUv + uDir * 1.3846).rgb + texture(uTex, vUv - uDir * 1.3846).rgb) * 0.3162162;
  c += (texture(uTex, vUv + uDir * 3.2308).rgb + texture(uTex, vUv - uDir * 3.2308).rgb) * 0.0702703;
  o = vec4(c, 1.0);
}`;
const COMPOSITE_FS = `#version 300 es
precision highp float; in vec2 vUv; out vec4 o;
uniform sampler2D uScene; uniform sampler2D uBloom; uniform sampler2D uBlur; uniform sampler2D uDepth;
uniform vec4 uPost; // bloom strength, dof aperture, focus, exposure
uniform vec2 uClip; // near, far
vec3 aces(vec3 x) { return clamp((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14), 0.0, 1.0); }
void main() {
  vec3 c = texture(uScene, vUv).rgb;
  if (uPost.y > 0.0) {
    float z = texture(uDepth, vUv).r * 2.0 - 1.0;
    float d = 2.0 * uClip.x * uClip.y / (uClip.y + uClip.x - z * (uClip.y - uClip.x));
    float coc = clamp(abs(d - uPost.z) * uPost.y, 0.0, 1.0);
    c = mix(c, texture(uBlur, vUv).rgb, coc);
  }
  c += texture(uBloom, vUv).rgb * uPost.x;
  c = pow(aces(c * uPost.w), vec3(1.0 / 2.2));
  o = vec4(c, dot(c, vec3(0.299, 0.587, 0.114)));
}`;
const FXAA_FS = `#version 300 es
precision highp float; in vec2 vUv; out vec4 o; uniform sampler2D uTex; uniform vec2 uTexel;
void main() {
  vec3 rgbM = texture(uTex, vUv).rgb;
  float lM = texture(uTex, vUv).a;
  float lNW = texture(uTex, vUv + vec2(-1,-1) * uTexel).a, lNE = texture(uTex, vUv + vec2(1,-1) * uTexel).a;
  float lSW = texture(uTex, vUv + vec2(-1,1) * uTexel).a, lSE = texture(uTex, vUv + vec2(1,1) * uTexel).a;
  float lMin = min(lM, min(min(lNW, lNE), min(lSW, lSE))), lMax = max(lM, max(max(lNW, lNE), max(lSW, lSE)));
  if (lMax - lMin < max(0.0312, lMax * 0.125)) { o = vec4(rgbM, 1.0); return; }
  vec2 dir = vec2(-((lNW + lNE) - (lSW + lSE)), ((lNW + lSW) - (lNE + lSE)));
  float red = max((lNW + lNE + lSW + lSE) * 0.03125, 1.0 / 128.0);
  dir = clamp(dir / (min(abs(dir.x), abs(dir.y)) + red), vec2(-8.0), vec2(8.0)) * uTexel;
  vec3 a = 0.5 * (texture(uTex, vUv + dir * (1.0/3.0 - 0.5)).rgb + texture(uTex, vUv + dir * (2.0/3.0 - 0.5)).rgb);
  vec3 b = a * 0.5 + 0.25 * (texture(uTex, vUv - dir * 0.5).rgb + texture(uTex, vUv + dir * 0.5).rgb);
  float lB = dot(b, vec3(0.299, 0.587, 0.114));
  o = vec4((lB < lMin || lB > lMax) ? a : b, 1.0);
}`;

const ENVS: Record<EnvironmentName, { id: number; top: string; hor: string; ground: string; sun: string; dir: [number, number, number]; sunI: number; envI: number; blur: number }> = {
  studio: { id: 0, top: '#3b3e45', hor: '#5d6068', ground: '#2a2b2f', sun: '#ffffff', dir: [0.35, 0.85, 0.4], sunI: 2.2, envI: 1, blur: 0.75 },
  sunset: { id: 1, top: '#2a3f7a', hor: '#ff9a5c', ground: '#3a2a2a', sun: '#ffc08a', dir: [-0.6, 0.22, -0.75], sunI: 3.2, envI: 1, blur: 0 },
  night: { id: 2, top: '#03050d', hor: '#1a2140', ground: '#05060a', sun: '#a9c0ff', dir: [0.4, 0.6, -0.65], sunI: 0.9, envI: 1.2, blur: 0 },
};

// ───────────────────────────── the scene ─────────────────────────────

interface Program { p: WebGLProgram; u: Record<string, WebGLUniformLocation | null> }
interface Target { fb: WebGLFramebuffer; tex: WebGLTexture; w: number; h: number }

let geomIds = 0;
const DRAG_PX = 4;

/**
 * A WebGL2 scene. Create it with {@link scene}.
 */
export class Scene {
  /** The canvas. */
  readonly canvas: HTMLCanvasElement;
  /** The WebGL2 context. */
  readonly gl: WebGL2RenderingContext;
  /** Camera: animate `camera.position`, `camera.target`, `fov`, `roll`. */
  readonly camera: Camera;
  /** Root objects. */
  readonly objects: Object3D[] = [];
  /** Live counters (updated every frame). */
  readonly stats: SceneStats = { drawCalls: 0, triangles: 0, culled: 0, frameMs: 16.7, dpr: 1 };
  /** Exposure before tonemapping (animatable). */
  exposure: number;
  /** Key-light intensity (animatable). */
  lightIntensity: number;
  /** Environment light intensity (animatable). */
  envIntensity: number;
  /** Key-light direction (towards the light; animatable). */
  readonly lightDir: Vector3;
  /** Bloom strength (animatable; needs `bloom` at creation). */
  bloomStrength: number;
  /** Depth-of-field focus distance (animatable; needs `dof`). */
  focus: number;

  private env: EnvironmentName;
  private envC: { top: number[]; hor: number[]; ground: number[]; sun: number[]; blur: number };
  private bg: number[] | null;
  private transparentBg: boolean;
  private shadows: boolean;
  private post: boolean;
  private bloomOn: boolean;
  private threshold: number;
  private dofOn: boolean;
  private aperture: number;
  private fxaaOn: boolean;
  private maxDpr: number;
  private dpr: number;
  private adaptive: boolean;
  private progs: Record<string, Program> = {};
  private ubo: WebGLBuffer;
  private uboData = new Float32Array(16 * 4 + 4 * 7);
  private view = m4(); private proj = m4(); private vp = m4(); private ivp = m4(); private lightVP = m4();
  private lv = m4(); private lp = m4(); private tmp = m4();
  private planes = new Float32Array(24);
  private lEye = new Vector3(); private lAt = new Vector3();
  private layerCtx: LayerContext | null = null;
  private opaque: Object3D[] = [];
  private transparent: Object3D[] = [];
  private casters: Object3D[] = [];
  private layers: SceneLayer[] = [];
  private shadowFb: WebGLFramebuffer | null = null;
  private shadowTex: WebGLTexture | null = null;
  private shadowSize = 2048;
  private msFb: WebGLFramebuffer | null = null;
  private msColor: WebGLRenderbuffer | null = null;
  private msDepth: WebGLRenderbuffer | null = null;
  private sceneT: Target | null = null;
  private depthTex: WebGLTexture | null = null;
  private depthFb: WebGLFramebuffer | null = null;
  private half: Target[] = [];
  private ldr: Target | null = null;
  private hdrFormat: number;
  private samples: number;
  private w = 0; private h = 0;
  private cssW = 1; private cssH = 1;
  private ro: ResizeObserver | null = null;
  private stopFrame: (() => void) | null = null;
  private emptyVao: WebGLVertexArrayObject;
  private slow = 0; private fast = 0; private lastT = 0;
  private listening = false;
  private hover: Object3D | null = null;
  private hoverInst = -1;
  private orbitCtl: Orbit | null = null;
  private domOff: Array<() => void> = [];
  private gpuGeoms = new Map<Geometry, GpuGeometry>();
  private maps = new WeakMap<object, { tex: WebGLTexture; version: number }>();
  private alive = true;

  constructor(canvas: HTMLCanvasElement, o: SceneInit = {}) {
    checkKeys('scene', o, SCENE_KEYS);
    if (o.camera) checkKeys('scene camera', o.camera, CAMERA_KEYS);
    if (o.environment && !ENVS[o.environment]) warn(`anikit: scene(): unknown environment "${o.environment}" (studio, sunset or night)`);
    this.canvas = canvas;
    this.env = o.environment && ENVS[o.environment] ? o.environment : 'studio';
    const E = ENVS[this.env];
    this.envC = { top: linearColor(E.top), hor: linearColor(E.hor), ground: linearColor(E.ground), sun: linearColor(E.sun), blur: E.blur };
    this.lightDir = new Vector3().copy(E.dir);
    this.lightIntensity = E.sunI;
    this.envIntensity = E.envI;
    this.exposure = o.exposure ?? 1;
    this.transparentBg = o.background === 'transparent';
    this.bg = o.background && o.background !== 'environment' && !this.transparentBg ? linearColor(o.background) : null;
    this.shadows = o.shadows ?? true;
    const bloom = o.bloom ? (o.bloom === true ? {} : o.bloom) : null;
    const dof = o.dof ? (o.dof === true ? {} : o.dof) : null;
    this.bloomOn = !!bloom;
    this.bloomStrength = bloom?.strength ?? 0.6;
    this.threshold = bloom?.threshold ?? 1;
    this.dofOn = !!dof;
    this.aperture = dof?.aperture ?? 0.15;
    this.post = this.bloomOn || this.dofOn || !!o.fxaa;
    this.fxaaOn = o.fxaa ?? this.post;
    this.maxDpr = Math.min(2, o.dpr ?? 2);
    this.adaptive = o.adaptive ?? true;
    const gl = canvas.getContext('webgl2', {
      antialias: !this.post, alpha: this.transparentBg, premultipliedAlpha: true, powerPreference: 'high-performance', preserveDrawingBuffer: false,
    });
    if (!gl) throw new Error('anikit: scene() needs WebGL2');
    this.gl = gl;
    const cf = !!gl.getExtension('EXT_color_buffer_float');
    this.hdrFormat = cf ? gl.RGBA16F : gl.RGBA8;
    this.samples = Math.min(4, gl.getParameter(gl.MAX_SAMPLES) as number);
    const c = o.camera ?? {};
    this.camera = {
      position: vec(c.position ?? [0, 2, 6], 0), target: vec(c.target ?? [0, 0, 0], 0),
      fov: c.fov ?? 45, roll: 0, near: c.near ?? 0.05, far: c.far ?? 200,
    };
    this.focus = dof?.focus ?? Math.hypot(this.camera.position.x - this.camera.target.x, this.camera.position.y - this.camera.target.y, this.camera.position.z - this.camera.target.z);

    // Pre-warm every program now so the first frame doesn't stall.
    this.progs.pbr = this.program(COMMON_VS, PBR_FS, '');
    this.progs.pbrI = this.program(COMMON_VS, PBR_FS, '#define INSTANCED\n');
    this.progs.depth = this.program(COMMON_VS, DEPTH_FS, '#define DEPTH\n');
    this.progs.depthI = this.program(COMMON_VS, DEPTH_FS, '#define DEPTH\n#define INSTANCED\n');
    this.progs.sky = this.program(FS_TRI, SKY_FS, '');
    if (this.post) {
      this.progs.bright = this.program(FS_TRI, BRIGHT_FS, '');
      this.progs.blur = this.program(FS_TRI, BLUR_FS, '');
      this.progs.comp = this.program(FS_TRI, COMPOSITE_FS, '');
      this.progs.fxaa = this.program(FS_TRI, FXAA_FS, '');
    }
    this.ubo = gl.createBuffer()!;
    gl.bindBuffer(gl.UNIFORM_BUFFER, this.ubo);
    gl.bufferData(gl.UNIFORM_BUFFER, this.uboData.byteLength, gl.DYNAMIC_DRAW);
    gl.bindBufferBase(gl.UNIFORM_BUFFER, 0, this.ubo);
    for (const k of ['pbr', 'pbrI', 'depth', 'depthI', 'sky']) {
      const p = this.progs[k].p, i = gl.getUniformBlockIndex(p, 'Frame');
      if (i !== gl.INVALID_INDEX) gl.uniformBlockBinding(p, i, 0);
    }
    this.emptyVao = gl.createVertexArray()!;
    // Without shadows a 1×1 depth map still backs the shadow sampler: a sampler2DShadow
    // over an empty or colour texture is a GL error on every draw.
    if (!this.shadows) this.shadowSize = 1;
    this.makeShadowMap();

    // Size: read layout once, then follow ResizeObserver (no per-frame layout reads).
    this.dpr = Math.min(this.maxDpr, typeof devicePixelRatio === 'number' ? devicePixelRatio : 1);
    this.cssW = canvas.clientWidth || canvas.width || 300;
    this.cssH = canvas.clientHeight || canvas.height || 150;
    this.resize();
    if (typeof ResizeObserver === 'function') {
      this.ro = new ResizeObserver((es) => {
        const r = es[0].contentRect;
        if (r.width && r.height) { this.cssW = r.width; this.cssH = r.height; this.resize(); }
      });
      this.ro.observe(canvas);
    }
    if (o.autoRender ?? true) this.stopFrame = engine.onFrame((t) => this.frame(t));
    track(this);
  }

  // ── public API ──

  /**
   * Add a mesh (a {@link Geometry} or a primitive name), a group (`null`), or
   * a custom {@link SceneLayer}. Returns the object.
   */
  add(geometry: Geometry | PrimitiveName | null, options?: ObjectOptions): Object3D;
  add(layer: SceneLayer): SceneLayer;
  add(g: Geometry | PrimitiveName | SceneLayer | null, o: ObjectOptions = {}): Object3D | SceneLayer {
    if (g && typeof g === 'object' && 'render' in g && typeof (g as SceneLayer).render === 'function') {
      this.layers.push(g as SceneLayer);
      return g as SceneLayer;
    }
    checkKeys('scene.add', o, OBJECT_KEYS);
    const geo = typeof g === 'string' ? primitive(g) : (g as Geometry | null);
    const obj = new Object3D(geo ? this.upload(geo) : null, o, toMaterial(o.material));
    return this.attach(obj, o.parent);
  }

  /**
   * `count` copies of a mesh in one draw call. `fn(i, o)` sets each
   * instance's `o.position` / `o.rotation` / `o.scale` / `o.color`.
   */
  instances(geometry: Geometry | PrimitiveName, count: number, fn?: (i: number, o: InstanceState) => void, options: ObjectOptions = {}): InstancedMesh {
    checkKeys('scene.instances', options, OBJECT_KEYS);
    const geo = typeof geometry === 'string' ? primitive(geometry) : geometry;
    const inst = new InstancedMesh(this.upload(geo), Math.max(0, count | 0), options, toMaterial(options.material));
    const gl = this.gl;
    inst.ibuf = gl.createBuffer();
    inst.ivao = this.vao(geo, inst.ibuf!, count);
    if (fn) inst.update(fn); else inst.update(() => {});
    return this.attach(inst, options.parent) as InstancedMesh;
  }

  /** Remove an object (or layer) from the scene. Its geometry stays cached for re-use. */
  remove(x: Object3D | SceneLayer): void {
    if (x instanceof Object3D) {
      if (x.parent) x.parent.children.splice(x.parent.children.indexOf(x), 1);
      else { const i = this.objects.indexOf(x); if (i >= 0) this.objects.splice(i, 1); }
      x.parent = null; x.scene = null;
      if (x instanceof InstancedMesh) { this.gl.deleteBuffer(x.ibuf); this.gl.deleteVertexArray(x.ivao); x.ibuf = null; x.ivao = null; }
    } else {
      const i = this.layers.indexOf(x);
      if (i >= 0) { this.layers.splice(i, 1); x.dispose?.(); }
    }
  }

  /** Orbit/drag camera controls with inertia (one per scene). */
  orbit(o: OrbitOptions = {}): Orbit {
    checkKeys('scene.orbit', o, ORBIT_KEYS);
    this.orbitCtl?.revert();
    this.orbitCtl = new Orbit(this, o);
    return this.orbitCtl;
  }

  /** Put the camera on a `flightPath` at arc length `s` (eye, target and roll). */
  fly(path: FlightPath, s: number): void {
    const p = path.at(s);
    this.camera.position.set(p.eye[0], p.eye[1], p.eye[2]);
    this.camera.target.set(p.target[0], p.target[1], p.target[2]);
    this.camera.roll = p.roll;
  }

  /** Cast a ray through canvas pixel `(x, y)` (CSS px from the canvas's top-left). */
  raycast(x: number, y: number): RayHit | null {
    this.updateCamera();
    const nx = (x / this.cssW) * 2 - 1, ny = 1 - (y / this.cssH) * 2;
    const ro = this.unproject(nx, ny, -1), rf = this.unproject(nx, ny, 1);
    const dx = rf[0] - ro[0], dy = rf[1] - ro[1], dz = rf[2] - ro[2], l = Math.hypot(dx, dy, dz);
    const dir = [dx / l, dy / l, dz / l];
    for (const o of this.objects) this.updateWorld(o, null);
    let best: RayHit | null = null;
    const visit = (o: Object3D) => {
      if (!o.visible) return;
      if (o.gpu) {
        const h = this.hitObject(o, ro, dir);
        if (h && (!best || h.distance < best.distance)) best = h;
      }
      for (const c of o.children) visit(c);
    };
    for (const o of this.objects) visit(o);
    return best;
  }

  /** Render one frame now (normally automatic). */
  render(): void {
    if (!this.alive) return;
    const gl = this.gl;
    this.updateCamera();
    // World matrices + render lists.
    this.opaque.length = 0; this.transparent.length = 0; this.casters.length = 0;
    this.stats.culled = 0;
    for (const o of this.objects) this.collect(o, null);
    const ce = this.camera.position;
    for (const o of this.transparent) o.depth = (o.world[12] - ce.x) ** 2 + (o.world[13] - ce.y) ** 2 + (o.world[14] - ce.z) ** 2;
    this.opaque.sort(byState);
    this.transparent.sort(this.byDepth);
    this.writeUbo();
    this.stats.drawCalls = 0; this.stats.triangles = 0;

    if (this.shadows && this.shadowFb) {
      gl.bindFramebuffer(gl.FRAMEBUFFER, this.shadowFb);
      gl.viewport(0, 0, this.shadowSize, this.shadowSize);
      gl.clear(gl.DEPTH_BUFFER_BIT);
      gl.enable(gl.DEPTH_TEST); gl.depthMask(true); gl.disable(gl.BLEND);
      gl.enable(gl.CULL_FACE); gl.cullFace(gl.FRONT);
      for (const o of this.casters) this.draw(o, true);
      gl.cullFace(gl.BACK);
    }

    gl.bindFramebuffer(gl.FRAMEBUFFER, this.post ? this.msFb : null);
    gl.viewport(0, 0, this.w, this.h);
    const bg = this.bg;
    if (bg) gl.clearColor(this.clear[0], this.clear[1], this.clear[2], 1); else gl.clearColor(0, 0, 0, 0);
    gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
    if (!bg && !this.transparentBg) {
      gl.disable(gl.DEPTH_TEST); gl.depthMask(false);
      const s = this.progs.sky;
      gl.useProgram(s.p);
      gl.uniform1f(s.u.uBlur, this.envC.blur);
      gl.bindVertexArray(this.emptyVao);
      gl.drawArrays(gl.TRIANGLES, 0, 3);
      this.stats.drawCalls++;
    }
    gl.enable(gl.DEPTH_TEST); gl.depthMask(true); gl.depthFunc(gl.LEQUAL);
    gl.enable(gl.CULL_FACE); gl.cullFace(gl.BACK);
    gl.disable(gl.BLEND);
    for (const o of this.opaque) this.draw(o, false);
    if (this.layers.length) {
      const ctx = (this.layerCtx ??= { gl, view: this.view, proj: this.proj, viewProj: this.vp, eye: this.camera.position, width: 0, height: 0, hdr: this.post });
      ctx.width = this.w; ctx.height = this.h;
      for (const l of this.layers) l.render(ctx);
      gl.bindVertexArray(null);
    }
    if (this.transparent.length) {
      gl.enable(gl.BLEND); gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA); gl.depthMask(false);
      for (const o of this.transparent) this.draw(o, false);
      gl.depthMask(true); gl.disable(gl.BLEND);
    }
    if (this.post) this.postProcess();
    gl.bindVertexArray(null);
  }

  /** Draw-buffer pixels as RGBA (for tests and screenshots). Call right after `render()`. */
  readPixels(x = 0, y = 0, w = this.w, h = this.h): Uint8Array {
    const out = new Uint8Array(w * h * 4);
    this.gl.bindFramebuffer(this.gl.FRAMEBUFFER, null);
    this.gl.readPixels(x, y, w, h, this.gl.RGBA, this.gl.UNSIGNED_BYTE, out);
    return out;
  }

  /** Stop rendering and free every GPU resource. */
  revert(): void { this.kill(); }
  /** Stop rendering and free every GPU resource. */
  kill(): void {
    if (!this.alive) return;
    this.alive = false;
    this.stopFrame?.(); this.stopFrame = null;
    this.ro?.disconnect();
    this.orbitCtl?.revert();
    for (const off of this.domOff) off();
    for (const l of this.layers) l.dispose?.();
    const gl = this.gl;
    for (const g of this.gpuGeoms.values()) { gl.deleteVertexArray(g.vao); for (const b of g.buffers) gl.deleteBuffer(b); }
    const visit = (o: Object3D) => { if (o instanceof InstancedMesh) { gl.deleteBuffer(o.ibuf); gl.deleteVertexArray(o.ivao); } o.children.forEach(visit); };
    this.objects.forEach(visit);
    for (const p of Object.values(this.progs)) gl.deleteProgram(p.p);
    this.freeTargets();
    if (this.shadowFb) { gl.deleteFramebuffer(this.shadowFb); gl.deleteTexture(this.shadowTex); }
    gl.deleteBuffer(this.ubo);
    this.objects.length = 0; this.layers.length = 0; this.gpuGeoms.clear();
  }

  // ── internals ──

  /** @internal */ get cssSize(): [number, number] { return [this.cssW, this.cssH]; }

  /** @internal */ listen(): void {
    if (this.listening) return;
    this.listening = true;
    const c = this.canvas;
    let down: [number, number] | null = null;
    const pos = (e: MouseEvent): [number, number] => [e.offsetX, e.offsetY];
    const fire = (type: string, h: RayHit | null, e: PointerEvent | MouseEvent, obj = h?.object ?? null) => {
      // Live HTML maps: forward the event to the element under the hit's UV.
      const map = h?.object.material?.map as { element?: Element } | null | undefined;
      if (h?.uv && map && map.element && type !== 'pointerenter' && type !== 'pointerleave') forwardToElement(map.element, h.uv, type, e);
      for (let o: Object3D | null = obj; o; o = o.parent) {
        const set = o.handlers?.get(type);
        if (set && set.size) {
          const ev: ScenePointerEvent = { object: h ? h.object : o, instance: h ? h.instance : -1, point: h ? h.point : [0, 0, 0], distance: h ? h.distance : 0, uv: h?.uv ?? null, event: e };
          for (const fn of set) fn(ev);
        }
      }
    };
    const onMove = (e: PointerEvent) => {
      const [x, y] = pos(e), h = this.raycast(x, y);
      const obj = h?.object ?? null, inst = h?.instance ?? -1;
      if (obj !== this.hover || inst !== this.hoverInst) {
        if (this.hover) fire('pointerleave', null, e, this.hover);
        this.hover = obj; this.hoverInst = inst;
        if (obj) fire('pointerenter', h, e);
        let clickable = false;
        for (let o: Object3D | null = obj; o; o = o.parent) if (o.handlers?.get('click')?.size) clickable = true;
        c.style.cursor = clickable ? 'pointer' : '';
      }
      if (h) fire('pointermove', h, e);
    };
    const onDown = (e: PointerEvent) => { const [x, y] = pos(e); down = [x, y]; fire('pointerdown', this.raycast(x, y), e); };
    const onUp = (e: PointerEvent) => {
      const [x, y] = pos(e), h = this.raycast(x, y);
      fire('pointerup', h, e);
      if (down && Math.hypot(x - down[0], y - down[1]) < DRAG_PX) fire('click', h, e);
      down = null;
    };
    c.addEventListener('pointermove', onMove);
    c.addEventListener('pointerdown', onDown);
    c.addEventListener('pointerup', onUp);
    this.domOff.push(() => { c.removeEventListener('pointermove', onMove); c.removeEventListener('pointerdown', onDown); c.removeEventListener('pointerup', onUp); });
  }

  private attach(obj: Object3D, parent?: Object3D): Object3D {
    obj.scene = this;
    if (parent) parent.add(obj); else this.objects.push(obj);
    if (obj.handlers || (obj.material?.map as { element?: Element } | null)?.element) this.listen();
    return obj;
  }

  /** Clear colour: linear for the HDR chain, gamma-encoded when drawing straight to the canvas. */
  private get clear(): number[] {
    const c = this.bg!;
    if (this.post) return c;
    const g = this.clearOut;
    for (let i = 0; i < 3; i++) g[i] = Math.pow(Math.min(1, Math.max(0, c[i])), 1 / 2.2);
    return g;
  }
  private clearOut = [0, 0, 0];

  private frame(t: number): void {
    const dt = this.lastT ? t - this.lastT : 16.7;
    this.lastT = t;
    if (dt > 0 && dt < 250) this.stats.frameMs += (dt - this.stats.frameMs) * 0.1;
    this.orbitCtl?.step(dt);
    if (this.adaptive) this.govern();
    this.render();
  }

  private govern(): void {
    const ms = this.stats.frameMs;
    if (ms > 22) { this.slow++; this.fast = 0; } else if (ms < 17.5) { this.fast++; this.slow = 0; } else { this.slow = this.fast = 0; }
    const base = Math.min(this.maxDpr, typeof devicePixelRatio === 'number' ? devicePixelRatio : 1);
    if (this.slow > 45 && this.dpr > 0.75) { this.dpr = Math.max(0.75, this.dpr - 0.25); this.slow = 0; this.resize(); }
    else if (this.fast > 180 && this.dpr < base) { this.dpr = Math.min(base, this.dpr + 0.25); this.fast = 0; this.resize(); }
  }

  private resize(): void {
    const w = Math.max(1, Math.round(this.cssW * this.dpr)), h = Math.max(1, Math.round(this.cssH * this.dpr));
    this.stats.dpr = this.dpr;
    if (w === this.w && h === this.h) return;
    this.w = w; this.h = h;
    this.canvas.width = w; this.canvas.height = h;
    if (this.post) this.makeTargets();
  }

  private program(vs: string, fs: string, defs: string): Program {
    const gl = this.gl;
    const sh = (type: number, src: string) => {
      const s = gl.createShader(type)!;
      gl.shaderSource(s, src.replace('#version 300 es\n', '#version 300 es\n' + defs));
      gl.compileShader(s);
      if (!gl.getShaderParameter(s, gl.COMPILE_STATUS) && !gl.isContextLost()) throw new Error('anikit: scene shader: ' + gl.getShaderInfoLog(s));
      return s;
    };
    const p = gl.createProgram()!;
    const a = sh(gl.VERTEX_SHADER, vs), b = sh(gl.FRAGMENT_SHADER, fs);
    gl.attachShader(p, a); gl.attachShader(p, b); gl.linkProgram(p);
    if (!gl.getProgramParameter(p, gl.LINK_STATUS) && !gl.isContextLost()) throw new Error('anikit: scene program: ' + gl.getProgramInfoLog(p));
    gl.deleteShader(a); gl.deleteShader(b);
    const u: Record<string, WebGLUniformLocation | null> = {};
    const n = gl.getProgramParameter(p, gl.ACTIVE_UNIFORMS) as number;
    for (let i = 0; i < n; i++) { const info = gl.getActiveUniform(p, i); if (info) u[info.name] = gl.getUniformLocation(p, info.name); }
    return { p, u };
  }

  private upload(g: Geometry): GpuGeometry {
    let gg = this.gpuGeoms.get(g);
    if (gg) return gg;
    const gl = this.gl;
    const pb = gl.createBuffer()!, nb = gl.createBuffer()!, ib = gl.createBuffer()!;
    const vao = gl.createVertexArray()!;
    gl.bindVertexArray(vao);
    gl.bindBuffer(gl.ARRAY_BUFFER, pb); gl.bufferData(gl.ARRAY_BUFFER, g.positions, gl.STATIC_DRAW);
    gl.enableVertexAttribArray(0); gl.vertexAttribPointer(0, 3, gl.FLOAT, false, 0, 0);
    gl.bindBuffer(gl.ARRAY_BUFFER, nb); gl.bufferData(gl.ARRAY_BUFFER, g.normals, gl.STATIC_DRAW);
    gl.enableVertexAttribArray(1); gl.vertexAttribPointer(1, 3, gl.FLOAT, false, 0, 0);
    let ub: WebGLBuffer | null = null;
    if (g.uvs && g.uvs.length * 3 === g.positions.length * 2) {
      ub = gl.createBuffer()!;
      gl.bindBuffer(gl.ARRAY_BUFFER, ub); gl.bufferData(gl.ARRAY_BUFFER, g.uvs, gl.STATIC_DRAW);
      gl.enableVertexAttribArray(2); gl.vertexAttribPointer(2, 2, gl.FLOAT, false, 0, 0);
    }
    const small = g.positions.length / 3 <= 65536;
    gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, ib);
    gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, small ? new Uint16Array(g.indices) : g.indices, gl.STATIC_DRAW);
    gl.bindVertexArray(null);
    // Bounding sphere (box centre + max distance).
    const p = g.positions;
    let lx = Infinity, ly = Infinity, lz = Infinity, hx = -Infinity, hy = -Infinity, hz = -Infinity;
    for (let i = 0; i < p.length; i += 3) {
      if (p[i] < lx) lx = p[i]; if (p[i] > hx) hx = p[i]; if (p[i + 1] < ly) ly = p[i + 1]; if (p[i + 1] > hy) hy = p[i + 1]; if (p[i + 2] < lz) lz = p[i + 2]; if (p[i + 2] > hz) hz = p[i + 2];
    }
    const cx = (lx + hx) / 2 || 0, cy = (ly + hy) / 2 || 0, cz = (lz + hz) / 2 || 0;
    let r = 0;
    for (let i = 0; i < p.length; i += 3) r = Math.max(r, Math.hypot(p[i] - cx, p[i + 1] - cy, p[i + 2] - cz));
    gg = { id: ++geomIds, vao, count: g.indices.length, type: small ? gl.UNSIGNED_SHORT : gl.UNSIGNED_INT, buffers: [pb, nb, ib], uvBuffer: ub, geometry: g, center: [cx, cy, cz], radius: r };
    this.gpuGeoms.set(g, gg);
    return gg;
  }

  /** A VAO sharing the geometry's buffers plus a per-instance buffer (matrix + tint). */
  private vao(g: Geometry, ibuf: WebGLBuffer, count: number): WebGLVertexArrayObject {
    const gl = this.gl, gg = this.upload(g);
    const vao = gl.createVertexArray()!;
    gl.bindVertexArray(vao);
    gl.bindBuffer(gl.ARRAY_BUFFER, gg.buffers[0]); gl.enableVertexAttribArray(0); gl.vertexAttribPointer(0, 3, gl.FLOAT, false, 0, 0);
    gl.bindBuffer(gl.ARRAY_BUFFER, gg.buffers[1]); gl.enableVertexAttribArray(1); gl.vertexAttribPointer(1, 3, gl.FLOAT, false, 0, 0);
    if (gg.uvBuffer) { gl.bindBuffer(gl.ARRAY_BUFFER, gg.uvBuffer); gl.enableVertexAttribArray(2); gl.vertexAttribPointer(2, 2, gl.FLOAT, false, 0, 0); }
    gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, gg.buffers[2]);
    gl.bindBuffer(gl.ARRAY_BUFFER, ibuf);
    gl.bufferData(gl.ARRAY_BUFFER, count * 19 * 4, gl.DYNAMIC_DRAW);
    for (let i = 0; i < 4; i++) {
      gl.enableVertexAttribArray(3 + i); gl.vertexAttribPointer(3 + i, 4, gl.FLOAT, false, 64, i * 16); gl.vertexAttribDivisor(3 + i, 1);
    }
    gl.enableVertexAttribArray(7); gl.vertexAttribPointer(7, 3, gl.FLOAT, false, 12, count * 64); gl.vertexAttribDivisor(7, 1);
    gl.bindVertexArray(null);
    return vao;
  }

  private updateWorld(o: Object3D, parent: Float32Array | null): void {
    const local = o.matrix ?? compose(this.tmp, o.position, o.rotation, o.scale);
    if (parent) mul(o.world, parent, local); else o.world.set(local);
    for (const c of o.children) this.updateWorld(c, o.world);
  }

  private collect(o: Object3D, parent: Float32Array | null): void {
    if (!o.visible) return;
    const local = o.matrix ?? compose(this.tmp, o.position, o.rotation, o.scale);
    if (parent) mul(o.world, parent, local); else o.world.set(local);
    if (o.gpu) {
      const isI = o instanceof InstancedMesh;
      if (isI && !o.count) { /* nothing */ } else {
        if (o.castShadow && this.shadows) this.casters.push(o);
        if (this.inFrustum(o)) (o.material.opacity < 1 || o.material.glass ? this.transparent : this.opaque).push(o);
        else this.stats.culled++;
      }
    }
    for (const c of o.children) this.collect(c, o.world);
  }

  private sphereOf(o: Object3D, out: number[]): void {
    const w = o.world, g = o.gpu!;
    const c = o instanceof InstancedMesh ? o.bound : g.center;
    const r = o instanceof InstancedMesh ? o.bound[3] : g.radius;
    out[0] = w[0] * c[0] + w[4] * c[1] + w[8] * c[2] + w[12];
    out[1] = w[1] * c[0] + w[5] * c[1] + w[9] * c[2] + w[13];
    out[2] = w[2] * c[0] + w[6] * c[1] + w[10] * c[2] + w[14];
    out[3] = r * Math.max(Math.hypot(w[0], w[1], w[2]), Math.hypot(w[4], w[5], w[6]), Math.hypot(w[8], w[9], w[10]));
  }
  private sp = [0, 0, 0, 0];
  private inFrustum(o: Object3D): boolean {
    const s = this.sp; this.sphereOf(o, s);
    const p = this.planes;
    for (let i = 0; i < 24; i += 4) if (p[i] * s[0] + p[i + 1] * s[1] + p[i + 2] * s[2] + p[i + 3] < -s[3]) return false;
    return true;
  }

  private updateCamera(): void {
    const c = this.camera, aspect = this.cssW / this.cssH;
    lookAt(this.view, c.position, c.target, (c.roll * Math.PI) / 180);
    const f = 1 / Math.tan((c.fov * Math.PI) / 360), n = c.near, fa = c.far, P = this.proj;
    P.fill(0); P[0] = f / aspect; P[5] = f; P[10] = (fa + n) / (n - fa); P[11] = -1; P[14] = (2 * fa * n) / (n - fa);
    mul(this.vp, this.proj, this.view);
    invert(this.ivp, this.vp);
    // Frustum planes (Gribb–Hartmann), normalised.
    const m = this.vp, pl = this.planes;
    for (let i = 0; i < 6; i++) {
      const row = i >> 1, s = i & 1 ? -1 : 1;
      let a = m[3] + s * m[row], b = m[7] + s * m[4 + row], cc = m[11] + s * m[8 + row], d = m[15] + s * m[12 + row];
      const l = Math.hypot(a, b, cc) || 1; a /= l; b /= l; cc /= l; d /= l;
      pl[i * 4] = a; pl[i * 4 + 1] = b; pl[i * 4 + 2] = cc; pl[i * 4 + 3] = d;
    }
  }

  private unproject(x: number, y: number, z: number): [number, number, number] {
    const m = this.ivp;
    const X = m[0] * x + m[4] * y + m[8] * z + m[12], Y = m[1] * x + m[5] * y + m[9] * z + m[13];
    const Z = m[2] * x + m[6] * y + m[10] * z + m[14], W = m[3] * x + m[7] * y + m[11] * z + m[15];
    return [X / W, Y / W, Z / W];
  }

  private hitObject(o: Object3D, ro: number[], rd: number[]): RayHit | null {
    const s = [0, 0, 0, 0];
    this.sphereOf(o, s);
    if (raySphere(ro, rd, s) === Infinity) return null;
    if (o instanceof InstancedMesh) {
      // Per-instance bounding spheres (approximate, fast).
      let best = Infinity, bi = -1;
      const g = o.gpu!, m = o.matrices, w = o.world;
      const inst = m4(), wm = m4();
      for (let i = 0; i < o.count; i++) {
        inst.set(m.subarray(i * 16, i * 16 + 16)); mul(wm, w, inst);
        const c = g.center;
        s[0] = wm[0] * c[0] + wm[4] * c[1] + wm[8] * c[2] + wm[12]; s[1] = wm[1] * c[0] + wm[5] * c[1] + wm[9] * c[2] + wm[13]; s[2] = wm[2] * c[0] + wm[6] * c[1] + wm[10] * c[2] + wm[14];
        s[3] = g.radius * Math.max(Math.hypot(wm[0], wm[1], wm[2]), Math.hypot(wm[4], wm[5], wm[6]), Math.hypot(wm[8], wm[9], wm[10])) * 0.8;
        const t = raySphere(ro, rd, s);
        if (t < best) { best = t; bi = i; }
      }
      return bi < 0 ? null : { object: o, instance: bi, distance: best, point: [ro[0] + rd[0] * best, ro[1] + rd[1] * best, ro[2] + rd[2] * best] };
    }
    // Exact: triangles in local space.
    const inv = m4();
    if (!invert(inv, o.world)) return null;
    const lo = xform(inv, ro, 1), ld = xform(inv, rd, 0);
    const g = o.gpu!.geometry, p = g.positions, ix = g.indices;
    let best = Infinity, bt = -1;
    for (let t = 0; t < ix.length; t += 3) {
      const d = rayTri(lo, ld, p, ix[t] * 3, ix[t + 1] * 3, ix[t + 2] * 3);
      if (d < best) { best = d; bt = t; }
    }
    if (best === Infinity) return null;
    const lp = [lo[0] + ld[0] * best, lo[1] + ld[1] * best, lo[2] + ld[2] * best];
    const wp = xform(o.world, lp, 1);
    const hit: RayHit = { object: o, instance: -1, point: [wp[0], wp[1], wp[2]], distance: Math.hypot(wp[0] - ro[0], wp[1] - ro[1], wp[2] - ro[2]) };
    const uv = g.uvs;
    if (uv && uv.length * 3 === p.length * 2) {
      // Barycentric weights of the hit point in the hit triangle → interpolated UV.
      const a = ix[bt], b = ix[bt + 1], c = ix[bt + 2];
      const v0 = [p[b * 3] - p[a * 3], p[b * 3 + 1] - p[a * 3 + 1], p[b * 3 + 2] - p[a * 3 + 2]];
      const v1 = [p[c * 3] - p[a * 3], p[c * 3 + 1] - p[a * 3 + 1], p[c * 3 + 2] - p[a * 3 + 2]];
      const v2 = [lp[0] - p[a * 3], lp[1] - p[a * 3 + 1], lp[2] - p[a * 3 + 2]];
      const d00 = v0[0] * v0[0] + v0[1] * v0[1] + v0[2] * v0[2], d01 = v0[0] * v1[0] + v0[1] * v1[1] + v0[2] * v1[2];
      const d11 = v1[0] * v1[0] + v1[1] * v1[1] + v1[2] * v1[2], d20 = v2[0] * v0[0] + v2[1] * v0[1] + v2[2] * v0[2];
      const d21 = v2[0] * v1[0] + v2[1] * v1[1] + v2[2] * v1[2], den = d00 * d11 - d01 * d01 || 1;
      const wb = (d11 * d20 - d01 * d21) / den, wc = (d00 * d21 - d01 * d20) / den, wa = 1 - wb - wc;
      hit.uv = [uv[a * 2] * wa + uv[b * 2] * wb + uv[c * 2] * wc, uv[a * 2 + 1] * wa + uv[b * 2 + 1] * wb + uv[c * 2 + 1] * wc];
    }
    return hit;
  }

  private writeUbo(): void {
    const d = this.uboData, gl = this.gl;
    // Shadow camera: orthographic box around everything that casts.
    if (this.shadows) {
      let lx = Infinity, ly = Infinity, lz = Infinity, hx = -Infinity, hy = -Infinity, hz = -Infinity;
      const s = this.sp;
      const all = this.casters;
      for (const o of all) {
        this.sphereOf(o, s);
        lx = Math.min(lx, s[0] - s[3]); ly = Math.min(ly, s[1] - s[3]); lz = Math.min(lz, s[2] - s[3]);
        hx = Math.max(hx, s[0] + s[3]); hy = Math.max(hy, s[1] + s[3]); hz = Math.max(hz, s[2] + s[3]);
      }
      if (!all.length) { lx = ly = lz = -1; hx = hy = hz = 1; }
      // Clamp to a region around the camera target so huge floors don't blur the map.
      const t = this.camera.target, lim = 25;
      lx = Math.max(lx, t.x - lim); ly = Math.max(ly, t.y - lim); lz = Math.max(lz, t.z - lim);
      hx = Math.min(hx, t.x + lim); hy = Math.min(hy, t.y + lim); hz = Math.min(hz, t.z + lim);
      const cx = (lx + hx) / 2, cy = (ly + hy) / 2, cz = (lz + hz) / 2;
      const r = Math.max(0.5, Math.hypot(hx - lx, hy - ly, hz - lz) / 2);
      const L = this.lightDir, ll = Math.hypot(L.x, L.y, L.z) || 1;
      this.lEye.set(cx + (L.x / ll) * r * 2, cy + (L.y / ll) * r * 2, cz + (L.z / ll) * r * 2);
      lookAt(this.lv, this.lEye, this.lAt.set(cx, cy, cz), 0);
      const P = this.lp; P.fill(0);
      // Orthographic: depth [0, 4r] in front of the light maps to [-1, 1].
      P[0] = 1 / r; P[5] = 1 / r; P[10] = -2 / (4 * r); P[14] = -1; P[15] = 1;
      mul(this.lightVP, this.lp, this.lv);
    }
    d.set(this.view, 0); d.set(this.proj, 16); d.set(this.lightVP, 32); d.set(this.ivp, 48);
    const c = this.camera.position, E = this.envC, L = this.lightDir, ll = Math.hypot(L.x, L.y, L.z) || 1;
    let o = 64;
    d[o++] = c.x; d[o++] = c.y; d[o++] = c.z; d[o++] = this.exposure;
    d[o++] = L.x / ll; d[o++] = L.y / ll; d[o++] = L.z / ll; d[o++] = this.lightIntensity;
    d[o++] = E.sun[0]; d[o++] = E.sun[1]; d[o++] = E.sun[2]; d[o++] = this.shadows ? 1 : 0;
    d[o++] = E.top[0]; d[o++] = E.top[1]; d[o++] = E.top[2]; d[o++] = ENVS[this.env].id;
    d[o++] = E.hor[0]; d[o++] = E.hor[1]; d[o++] = E.hor[2]; d[o++] = this.post ? 0 : 1;
    d[o++] = E.ground[0]; d[o++] = E.ground[1]; d[o++] = E.ground[2]; d[o++] = this.envIntensity;
    d[o++] = this.lastT / 1000; d[o++] = 0; d[o++] = 0; d[o++] = 0;
    gl.bindBuffer(gl.UNIFORM_BUFFER, this.ubo);
    gl.bufferSubData(gl.UNIFORM_BUFFER, 0, d);
  }

  private lastProg: Program | null = null;
  private draw(o: Object3D, depth: boolean): void {
    const gl = this.gl, g = o.gpu!, isI = o instanceof InstancedMesh;
    const prog = depth ? (isI ? this.progs.depthI : this.progs.depth) : isI ? this.progs.pbrI : this.progs.pbr;
    gl.useProgram(prog.p);
    if (isI) {
      const inst = o as InstancedMesh;
      if (inst.dirty) {
        gl.bindBuffer(gl.ARRAY_BUFFER, inst.ibuf);
        gl.bufferSubData(gl.ARRAY_BUFFER, 0, inst.matrices);
        gl.bufferSubData(gl.ARRAY_BUFFER, inst.count * 64, inst.colors);
        inst.dirty = false;
      }
      gl.bindVertexArray(inst.ivao);
    } else gl.bindVertexArray(g.vao);
    gl.uniformMatrix4fv(prog.u.uModel, false, o.world);
    if (!depth) {
      const m = o.material;
      gl.uniform4f(prog.u.uColor, m.color[0], m.color[1], m.color[2], Math.max(0, Math.min(1, m.opacity)));
      gl.uniform4f(prog.u.uPbr, m.metalness, m.roughness, m.emissiveIntensity, m.glass ? 2 : m.toon ? 1 : 0);
      gl.uniform3f(prog.u.uEmissive, m.emissive[0], m.emissive[1], m.emissive[2]);
      const tex = m.map ? this.mapTexture(m.map) : null;
      gl.uniform1f(prog.u.uHasMap, tex ? 1 : 0);
      // Always on unit 1: a 2D and a shadow sampler may not share unit 0, even unused.
      gl.uniform1i(prog.u.uMap, 1);
      if (tex) { gl.activeTexture(gl.TEXTURE1); gl.bindTexture(gl.TEXTURE_2D, tex); gl.activeTexture(gl.TEXTURE0); }
      gl.activeTexture(gl.TEXTURE0); gl.bindTexture(gl.TEXTURE_2D, this.shadowTex);
      gl.uniform1i(prog.u.uShadow, 0);
      this.stats.triangles += (g.count / 3) * (isI ? (o as InstancedMesh).count : 1);
    }
    if (isI) gl.drawElementsInstanced(gl.TRIANGLES, g.count, g.type, 0, (o as InstancedMesh).count);
    else gl.drawElements(gl.TRIANGLES, g.count, g.type, 0);
    this.stats.drawCalls++;
  }

  /** GPU texture for a map source; re-uploaded only when a live source's `version` changes. */
  private mapTexture(src: MapSource): WebGLTexture | null {
    const gl = this.gl;
    let e = this.maps.get(src);
    const live = 'version' in src && 'canvas' in src;
    const version = live ? src.version : 0;
    if (live && version === 0) return null; // not rasterised yet
    if (!e) {
      const tex = gl.createTexture()!;
      // Unit 1: unit 0 belongs to the shadow sampler, which must never see a colour texture.
      gl.activeTexture(gl.TEXTURE1);
      gl.bindTexture(gl.TEXTURE_2D, tex);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
      e = { tex, version: -1 };
      this.maps.set(src, e);
    }
    if (e.version !== version) {
      gl.activeTexture(gl.TEXTURE1);
      gl.bindTexture(gl.TEXTURE_2D, e.tex);
      gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, false);
      gl.pixelStorei(gl.UNPACK_PREMULTIPLY_ALPHA_WEBGL, false);
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, live ? src.canvas : (src as TexImageSource));
      gl.activeTexture(gl.TEXTURE0);
      e.version = version;
    }
    return e.tex;
  }

  private byDepth = (a: Object3D, b: Object3D) => b.depth - a.depth;

  private makeShadowMap(): void {
    const gl = this.gl, n = this.shadowSize;
    const tex = gl.createTexture()!;
    gl.bindTexture(gl.TEXTURE_2D, tex);
    gl.texStorage2D(gl.TEXTURE_2D, 1, gl.DEPTH_COMPONENT24, n, n);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_COMPARE_MODE, gl.COMPARE_REF_TO_TEXTURE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_COMPARE_FUNC, gl.LEQUAL);
    const fb = gl.createFramebuffer()!;
    gl.bindFramebuffer(gl.FRAMEBUFFER, fb);
    gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.DEPTH_ATTACHMENT, gl.TEXTURE_2D, tex, 0);
    gl.drawBuffers([gl.NONE]); gl.readBuffer(gl.NONE);
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
    this.shadowFb = fb; this.shadowTex = tex;
  }

  private target(w: number, h: number, fmt: number): Target {
    const gl = this.gl, tex = gl.createTexture()!;
    gl.bindTexture(gl.TEXTURE_2D, tex);
    gl.texStorage2D(gl.TEXTURE_2D, 1, fmt, w, h);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    const fb = gl.createFramebuffer()!;
    gl.bindFramebuffer(gl.FRAMEBUFFER, fb);
    gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, tex, 0);
    return { fb, tex, w, h };
  }

  private freeTargets(): void {
    const gl = this.gl;
    for (const t of [this.sceneT, this.ldr, ...this.half]) if (t) { gl.deleteFramebuffer(t.fb); gl.deleteTexture(t.tex); }
    if (this.msFb) { gl.deleteFramebuffer(this.msFb); gl.deleteRenderbuffer(this.msColor); gl.deleteRenderbuffer(this.msDepth); }
    if (this.depthFb) { gl.deleteFramebuffer(this.depthFb); gl.deleteTexture(this.depthTex); }
    this.sceneT = this.ldr = null; this.half = []; this.msFb = null; this.depthFb = null;
  }

  private makeTargets(): void {
    const gl = this.gl, w = this.w, h = this.h, fmt = this.hdrFormat;
    this.freeTargets();
    this.msColor = gl.createRenderbuffer()!;
    gl.bindRenderbuffer(gl.RENDERBUFFER, this.msColor);
    gl.renderbufferStorageMultisample(gl.RENDERBUFFER, this.samples, fmt, w, h);
    this.msDepth = gl.createRenderbuffer()!;
    gl.bindRenderbuffer(gl.RENDERBUFFER, this.msDepth);
    gl.renderbufferStorageMultisample(gl.RENDERBUFFER, this.samples, gl.DEPTH_COMPONENT24, w, h);
    this.msFb = gl.createFramebuffer()!;
    gl.bindFramebuffer(gl.FRAMEBUFFER, this.msFb);
    gl.framebufferRenderbuffer(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.RENDERBUFFER, this.msColor);
    gl.framebufferRenderbuffer(gl.FRAMEBUFFER, gl.DEPTH_ATTACHMENT, gl.RENDERBUFFER, this.msDepth);
    this.sceneT = this.target(w, h, fmt);
    if (this.dofOn) {
      this.depthTex = gl.createTexture()!;
      gl.bindTexture(gl.TEXTURE_2D, this.depthTex);
      gl.texStorage2D(gl.TEXTURE_2D, 1, gl.DEPTH_COMPONENT24, w, h);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
      this.depthFb = gl.createFramebuffer()!;
      gl.bindFramebuffer(gl.FRAMEBUFFER, this.depthFb);
      gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.DEPTH_ATTACHMENT, gl.TEXTURE_2D, this.depthTex, 0);
    }
    const hw = Math.max(1, w >> 1), hh = Math.max(1, h >> 1);
    // half[0..1] bloom ping-pong, half[2..3] dof blur ping-pong
    this.half = [this.target(hw, hh, fmt), this.target(hw, hh, fmt)];
    if (this.dofOn) this.half.push(this.target(hw, hh, fmt), this.target(hw, hh, fmt));
    if (this.fxaaOn) this.ldr = this.target(w, h, gl.RGBA8);
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
  }

  private pass(p: Program, to: Target | null): void {
    const gl = this.gl;
    gl.bindFramebuffer(gl.FRAMEBUFFER, to ? to.fb : null);
    gl.viewport(0, 0, to ? to.w : this.w, to ? to.h : this.h);
    gl.useProgram(p.p);
    gl.drawArrays(gl.TRIANGLES, 0, 3);
    this.stats.drawCalls++;
  }
  private tex(unit: number, t: WebGLTexture | null): void { const gl = this.gl; gl.activeTexture(gl.TEXTURE0 + unit); gl.bindTexture(gl.TEXTURE_2D, t); }

  private postProcess(): void {
    const gl = this.gl, w = this.w, h = this.h, S = this.sceneT!, H = this.half;
    gl.bindFramebuffer(gl.READ_FRAMEBUFFER, this.msFb);
    gl.bindFramebuffer(gl.DRAW_FRAMEBUFFER, S.fb);
    gl.blitFramebuffer(0, 0, w, h, 0, 0, w, h, gl.COLOR_BUFFER_BIT, gl.NEAREST);
    if (this.dofOn && this.depthFb) {
      gl.bindFramebuffer(gl.DRAW_FRAMEBUFFER, this.depthFb);
      gl.blitFramebuffer(0, 0, w, h, 0, 0, w, h, gl.DEPTH_BUFFER_BIT, gl.NEAREST);
    }
    gl.disable(gl.DEPTH_TEST); gl.disable(gl.BLEND); gl.disable(gl.CULL_FACE);
    gl.bindVertexArray(this.emptyVao);
    const br = this.progs.bright, bl = this.progs.blur;
    const blurPair = (a: Target, b: Target, iters: number) => {
      gl.useProgram(bl.p); gl.uniform1i(bl.u.uTex, 0);
      for (let i = 0; i < iters; i++) {
        const k = 1 + i;
        this.tex(0, a.tex); gl.useProgram(bl.p); gl.uniform2f(bl.u.uDir, k / a.w, 0); this.pass(bl, b);
        this.tex(0, b.tex); gl.useProgram(bl.p); gl.uniform2f(bl.u.uDir, 0, k / a.h); this.pass(bl, a);
      }
    };
    if (this.bloomOn) {
      gl.useProgram(br.p); gl.uniform1i(br.u.uTex, 0); gl.uniform2f(br.u.uTexel, 0.5 / w, 0.5 / h); gl.uniform1f(br.u.uThreshold, this.threshold);
      this.tex(0, S.tex); this.pass(br, H[0]);
      blurPair(H[0], H[1], 3);
    }
    if (this.dofOn) {
      gl.useProgram(br.p); gl.uniform1i(br.u.uTex, 0); gl.uniform2f(br.u.uTexel, 0.5 / w, 0.5 / h); gl.uniform1f(br.u.uThreshold, -1);
      this.tex(0, S.tex); this.pass(br, H[2]);
      blurPair(H[2], H[3], 2);
    }
    const cp = this.progs.comp;
    gl.useProgram(cp.p);
    gl.uniform1i(cp.u.uScene, 0); gl.uniform1i(cp.u.uBloom, 1); gl.uniform1i(cp.u.uBlur, 2); gl.uniform1i(cp.u.uDepth, 3);
    gl.uniform4f(cp.u.uPost, this.bloomOn ? this.bloomStrength : 0, this.dofOn ? this.aperture : 0, this.focus, this.exposure);
    gl.uniform2f(cp.u.uClip, this.camera.near, this.camera.far);
    this.tex(0, S.tex); this.tex(1, this.bloomOn ? H[0].tex : null); this.tex(2, this.dofOn ? H[2].tex : null); this.tex(3, this.dofOn ? this.depthTex : null);
    this.pass(cp, this.fxaaOn ? this.ldr : null);
    if (this.fxaaOn) {
      const fx = this.progs.fxaa;
      gl.useProgram(fx.p); gl.uniform1i(fx.u.uTex, 0); gl.uniform2f(fx.u.uTexel, 1 / w, 1 / h);
      this.tex(0, this.ldr!.tex); this.pass(fx, null);
    }
    for (let i = 3; i >= 0; i--) this.tex(i, null);
  }
}

const byState = (a: Object3D, b: Object3D) =>
  (a instanceof InstancedMesh ? 1 : 0) - (b instanceof InstancedMesh ? 1 : 0) || a.gpu!.id - b.gpu!.id || a.material.id - b.material.id;

function xform(m: ArrayLike<number>, v: ArrayLike<number>, w: number): number[] {
  return [m[0] * v[0] + m[4] * v[1] + m[8] * v[2] + m[12] * w, m[1] * v[0] + m[5] * v[1] + m[9] * v[2] + m[13] * w, m[2] * v[0] + m[6] * v[1] + m[10] * v[2] + m[14] * w];
}
function raySphere(o: number[], d: number[], s: number[]): number {
  const ox = o[0] - s[0], oy = o[1] - s[1], oz = o[2] - s[2];
  const b = ox * d[0] + oy * d[1] + oz * d[2], c = ox * ox + oy * oy + oz * oz - s[3] * s[3];
  const disc = b * b - c;
  if (disc < 0) return Infinity;
  const t = -b - Math.sqrt(disc);
  return t >= 0 ? t : -b + Math.sqrt(disc) >= 0 ? 0 : Infinity;
}
function rayTri(o: number[], d: number[], p: Float32Array, a: number, b: number, c: number): number {
  const e1x = p[b] - p[a], e1y = p[b + 1] - p[a + 1], e1z = p[b + 2] - p[a + 2];
  const e2x = p[c] - p[a], e2y = p[c + 1] - p[a + 1], e2z = p[c + 2] - p[a + 2];
  const px = d[1] * e2z - d[2] * e2y, py = d[2] * e2x - d[0] * e2z, pz = d[0] * e2y - d[1] * e2x;
  const det = e1x * px + e1y * py + e1z * pz;
  if (Math.abs(det) < 1e-12) return Infinity;
  const inv = 1 / det, tx = o[0] - p[a], ty = o[1] - p[a + 1], tz = o[2] - p[a + 2];
  const u = (tx * px + ty * py + tz * pz) * inv;
  if (u < 0 || u > 1) return Infinity;
  const qx = ty * e1z - tz * e1y, qy = tz * e1x - tx * e1z, qz = tx * e1y - ty * e1x;
  const v = (d[0] * qx + d[1] * qy + d[2] * qz) * inv;
  if (v < 0 || u + v > 1) return Infinity;
  const t = (e2x * qx + e2y * qy + e2z * qz) * inv;
  return t > 1e-6 ? t : Infinity;
}

function toMaterial(m: ObjectOptions['material']): Material {
  return m instanceof Material ? m : new Material(m ?? {});
}

/** Primitive names accepted by `scene.add('sphere')`. */
export type PrimitiveName = 'box' | 'sphere' | 'cylinder' | 'torus' | 'plane' | 'capsule' | 'roundedBox';
const prims = new Map<string, Geometry>();
function primitive(n: PrimitiveName): Geometry {
  let g = prims.get(n);
  if (g) return g;
  switch (n) {
    case 'box': g = G.box(); break;
    case 'sphere': g = G.sphere(0.5, 48); break;
    case 'cylinder': g = G.cylinder(); break;
    case 'torus': g = G.torus(0.5, 0.18); break;
    case 'plane': g = G.plane(); break;
    case 'capsule': g = G.capsule(0.3, 1); break;
    case 'roundedBox': g = G.roundedBox(1, 1, 1, 0.12); break;
    default: warn(`anikit: scene.add(): unknown primitive "${n}" (box, sphere, cylinder, torus, plane, capsule, roundedBox)`); g = G.box();
  }
  prims.set(n, g);
  return g;
}

// ───────────────────────────── orbit controls ─────────────────────────────

/** Orbit / drag camera controls with inertia. Created by `scene.orbit()`. */
export class Orbit {
  /** Auto-rotation, degrees per second (animatable). */
  autoRotate: number;
  /** Fraction of velocity kept per 1/60 s. */
  damping: number;
  private vYaw = 0; private vPitch = 0; private vZoom = 0;
  private off: Array<() => void> = [];
  private dragging = false;
  private o: OrbitOptions;
  constructor(private s: Scene, o: OrbitOptions) {
    this.o = o;
    this.autoRotate = o.autoRotate ?? 0;
    this.damping = o.damping ?? 0.92;
    const c = s.canvas;
    let px = 0, py = 0, id = -1;
    const down = (e: PointerEvent) => {
      if (id !== -1) return;
      id = e.pointerId; px = e.clientX; py = e.clientY; this.dragging = true;
      c.setPointerCapture?.(e.pointerId);
    };
    const move = (e: PointerEvent) => {
      if (e.pointerId !== id) return;
      const [, h] = s.cssSize;
      const k = (Math.PI * 1.5) / h;
      this.vYaw = -(e.clientX - px) * k; this.vPitch = (e.clientY - py) * k;
      this.apply(this.vYaw, this.vPitch, 0);
      px = e.clientX; py = e.clientY;
    };
    const up = (e: PointerEvent) => { if (e.pointerId === id) { id = -1; this.dragging = false; } };
    const wheel = (e: WheelEvent) => {
      if (o.zoom === false) return;
      e.preventDefault();
      this.vZoom += Math.sign(e.deltaY) * Math.min(0.08, Math.abs(e.deltaY) / 1000);
    };
    c.addEventListener('pointerdown', down);
    c.addEventListener('pointermove', move);
    c.addEventListener('pointerup', up);
    c.addEventListener('pointercancel', up);
    c.addEventListener('wheel', wheel, { passive: false });
    c.style.touchAction = 'none';
    this.off.push(() => {
      c.removeEventListener('pointerdown', down); c.removeEventListener('pointermove', move);
      c.removeEventListener('pointerup', up); c.removeEventListener('pointercancel', up); c.removeEventListener('wheel', wheel);
    });
    track(this);
  }
  /** @internal per-frame inertia */
  step(dtMs: number): void {
    const k = Math.pow(this.damping, dtMs / 16.67);
    let yaw = 0, pitch = 0;
    if (!this.dragging) {
      this.vYaw *= k; this.vPitch *= k;
      if (Math.abs(this.vYaw) < 1e-5) this.vYaw = 0;
      if (Math.abs(this.vPitch) < 1e-5) this.vPitch = 0;
      yaw = this.vYaw; pitch = this.vPitch;
    }
    yaw += (this.autoRotate * Math.PI) / 180 * (dtMs / 1000);
    const z = this.vZoom; this.vZoom *= k * 0.8; if (Math.abs(this.vZoom) < 1e-5) this.vZoom = 0;
    if (yaw || pitch || z) this.apply(yaw, pitch, z);
  }
  private apply(dyaw: number, dpitch: number, dzoom: number): void {
    const c = this.s.camera, p = c.position, t = c.target;
    const x = p.x - t.x, y = p.y - t.y, z = p.z - t.z;
    let r = Math.hypot(x, y, z) || 1;
    let yaw = Math.atan2(x, z), pitch = Math.asin(Math.max(-1, Math.min(1, y / r)));
    const pl = this.o.pitch?.[0] ?? -85, ph = this.o.pitch?.[1] ?? 85;
    const dl = this.o.distance?.[0] ?? 0.5, dh = this.o.distance?.[1] ?? 100;
    yaw += dyaw;
    pitch = Math.max((pl * Math.PI) / 180, Math.min((ph * Math.PI) / 180, pitch + dpitch));
    r = Math.max(dl, Math.min(dh, r * (1 + dzoom)));
    p.set(t.x + r * Math.cos(pitch) * Math.sin(yaw), t.y + r * Math.sin(pitch), t.z + r * Math.cos(pitch) * Math.cos(yaw));
  }
  /** Remove the controls. */
  revert(): void { for (const f of this.off) f(); this.off.length = 0; }
  /** Remove the controls. */
  kill(): void { this.revert(); }
}

/**
 * Create a real-time WebGL2 scene on a canvas (element or selector).
 * See the module docs for a tour.
 */
export function scene(canvas: HTMLCanvasElement | string, options?: SceneInit): Scene {
  const el = typeof canvas === 'string' ? document.querySelector<HTMLCanvasElement>(canvas) : canvas;
  if (!el) throw new Error(`anikit: scene(): no canvas matches "${canvas}"`);
  return new Scene(el, options);
}

/** Dispatch `type` on the deepest descendant of `root` under element-relative UV `uv` ((0, 0) = top-left). */
function forwardToElement(root: Element, uv: [number, number], type: string, src: PointerEvent | MouseEvent): void {
  const r = root.getBoundingClientRect();
  const x = r.left + uv[0] * r.width, y = r.top + uv[1] * r.height;
  let target: Element = root;
  for (let descended = true; descended;) {
    descended = false;
    for (const c of Array.from(target.children)) {
      const cr = c.getBoundingClientRect();
      if (x >= cr.left && x <= cr.right && y >= cr.top && y <= cr.bottom) { target = c; descended = true; break; }
    }
  }
  const init = { bubbles: true, cancelable: true, composed: true, clientX: x, clientY: y, button: src.button, buttons: src.buttons, shiftKey: src.shiftKey, ctrlKey: src.ctrlKey, altKey: src.altKey, metaKey: src.metaKey };
  if (type === 'click') {
    target.dispatchEvent(new MouseEvent('click', init));
    if (typeof (target as HTMLElement).focus === 'function' && target.matches('input, textarea, select, button, a[href], [tabindex], [contenteditable]')) (target as HTMLElement).focus({ preventScroll: true });
  } else if (typeof PointerEvent !== 'undefined') {
    target.dispatchEvent(new PointerEvent(type, { ...init, pointerId: (src as PointerEvent).pointerId ?? 1, pointerType: (src as PointerEvent).pointerType ?? 'mouse' }));
  }
}
