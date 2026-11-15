/**
 * 3D: browser-exact `transform` interpolation (computed in the core), a bulk
 * projector for point clouds (perspective projection + radix depth sort in
 * WebAssembly), orbit controls, and `tilt()` hover cards.
 *
 * ```ts
 * animate(card, { transform: ['rotateY(0deg)', 'rotateY(180deg) translateZ(80px)'] });
 *
 * const cloud = batch(20000, { x: (i) => [...], y: …, z: … }, { duration: 2 });
 * const scene = scene3d({ canvas: '#c', fov: 50 });
 * scene.orbit({ autoRotate: 12 });
 * engine.onFrame(() => scene.draw(cloud, { size: 1.6, color: '#9cf' }));
 * ```
 */
import { core, writeStr, F64, f64Buf, views, W } from './wasm';
import { engine } from './core';
import { Batch } from './batch';
import { PointCloud } from './cloud';
import { Emitter } from './emitter';
import { spring, inertia } from './spring';
import { Targets, toTargets, stateOf, markDirty } from './props';
import { VelocityTracker } from './utils';
import { track } from './context';

export type Matrix = number[];

/** Parse a CSS transform list into a 4×4 matrix (column-major, `matrix3d` order). */
export function parseTransform(css: string): Matrix {
  const w = core();
  if (!w.ak_transform_parse(writeStr(css))) throw new Error(`anikit: cannot parse transform "${css}"`);
  views();
  const at = w.ak_f64_buf(0) >>> 3;
  return Array.from(F64.subarray(at, at + 16));
}

const r4 = (v: number) => Math.round(v * 1e4) / 1e4;
/** Format a matrix as CSS `matrix3d(...)`. */
export function toMatrix3d(m: ArrayLike<number>, at = 0): string {
  let s = 'matrix3d(';
  for (let i = 0; i < 16; i++) s += (i ? ',' : '') + r4(m[at + i]);
  return s + ')';
}

/** Interpolate two CSS transforms exactly like browsers (decompose → slerp → recompose). */
export function mixTransforms(a: string, b: string, t: number): string {
  const ma = parseTransform(a);
  const mb = parseTransform(b);
  const at = f64Buf(32);
  F64.set(ma, at);
  F64.set(mb, at + 16);
  const p = core().ak_matrix_mix(t) >>> 3;
  views();
  return toMatrix3d(F64, p);
}

// ---------------------------------------------------------------------------
// Scene / projector
// ---------------------------------------------------------------------------

export interface Vec3 {
  x: number;
  y: number;
  z: number;
}

export interface SceneOptions {
  /** Canvas to size against / draw into. */
  canvas?: HTMLCanvasElement | string;
  width?: number;
  height?: number;
  /** Vertical field of view in degrees (default 50). */
  fov?: number;
  near?: number;
  far?: number;
  camera?: Partial<Vec3>;
  target?: Partial<Vec3>;
}

export interface Projection {
  /** `[screenX, screenY, depth, scale] * count`. Culled points have scale 0. */
  data: Float32Array;
  /** Indices far → near (draw in this order). */
  order: Uint32Array;
  count: number;
}

export interface DrawOptions {
  /** Point radius in px at scale 1 (default 2). */
  size?: number;
  color?: string | ((i: number, depth: number, scale: number) => string);
  /** Fade with distance (0 = off, default 0.6). */
  fog?: number;
  clear?: boolean | string;
  /** Channel names (batch sources) for x/y/z (default 'x','y','z'). */
  channels?: [string, string, string];
  /** Draw squares (faster) instead of circles. */
  squares?: boolean;
  /** Overall opacity multiplier (default 1). */
  opacity?: number;
}

export class Scene3D {
  /** Animatable camera position. */
  camera: Vec3;
  /** Animatable look-at target. */
  target: Vec3;
  /** Animatable model rotation in degrees (applied X, then Y, then Z). */
  rotation: Vec3 = { x: 0, y: 0, z: 0 };
  fov: number;
  near: number;
  far: number;
  canvas: HTMLCanvasElement | null;
  width: number;
  height: number;
  private pid: number;
  /** @internal */
  get _pid(): number {
    return this.pid;
  }
  private ownLen = 0;
  private offs: Array<() => void> = [];

  constructor(o: SceneOptions = {}) {
    const c = typeof o.canvas === 'string' ? document.querySelector<HTMLCanvasElement>(o.canvas) : o.canvas ?? null;
    this.canvas = c;
    this.width = o.width ?? c?.width ?? 800;
    this.height = o.height ?? c?.height ?? 600;
    this.fov = o.fov ?? 50;
    this.near = o.near ?? 1;
    this.far = o.far ?? 20000;
    this.camera = { x: 0, y: 0, z: 800, ...o.camera };
    this.target = { x: 0, y: 0, z: 0, ...o.target };
    this.pid = core().ak_projector_create();
    track(this);
  }

  /** Resize the viewport (and canvas, if any, honouring devicePixelRatio). */
  resize(width: number, height: number, dpr = typeof devicePixelRatio === 'number' ? devicePixelRatio : 1): this {
    this.width = width;
    this.height = height;
    if (this.canvas) {
      this.canvas.width = Math.round(width * dpr);
      this.canvas.height = Math.round(height * dpr);
      this.canvas.style.width = width + 'px';
      this.canvas.style.height = height + 'px';
      this.canvas.getContext('2d')?.setTransform(dpr, 0, 0, dpr, 0, 0);
    }
    return this;
  }

  /**
   * The scene's combined (view·projection·rotation) matrix and the camera
   * position in model space — what renderers need to draw into this view.
   */
  viewProjection(): { matrix: number[]; eye: [number, number, number] } {
    const m = this.matrices();
    const model = m.slice(0, 16), vp = m.slice(16);
    const out = new Array<number>(16).fill(0);
    for (let c = 0; c < 4; c++) for (let r = 0; r < 4; r++) for (let k = 0; k < 4; k++) out[c * 4 + r] += vp[k * 4 + r] * model[c * 4 + k];
    // Model is a pure rotation: its inverse is its transpose.
    const e = [this.camera.x, this.camera.y, this.camera.z];
    const eye: [number, number, number] = [0, 1, 2].map((i) => model[i * 4] * e[0] + model[i * 4 + 1] * e[1] + model[i * 4 + 2] * e[2]) as [number, number, number];
    return { matrix: out, eye };
  }

  private matrices(): number[] {
    const w = core();
    const r = this.rotation;
    let p = w.ak_rotation(r.x, r.y, r.z) >>> 3;
    views();
    const model = Array.from(F64.subarray(p, p + 16));
    const c = this.camera;
    const t = this.target;
    p = w.ak_camera(c.x, c.y, c.z, t.x, t.y, t.z, this.fov, this.width / this.height, this.near, this.far) >>> 3;
    views();
    return model.concat(Array.from(F64.subarray(p, p + 16)));
  }

  /**
   * Project a batch (channels x/y/z) or a flat `[x, y, z, …]` array. Runs
   * entirely in the core; returns zero-copy views valid until the next call.
   */
  project(source: Batch | PointCloud | Emitter | ArrayLike<number>, o: { channels?: [string, string, string]; sort?: boolean } = {}): Projection {
    const w = core();
    const mats = this.matrices();
    let n: number;
    let src: number;
    let handle = 0;
    let stride = 3;
    let offs = [0, 1, 2];
    if (source instanceof Batch) {
      const [cx, cy, cz] = o.channels ?? ['x', 'y', 'z'];
      n = source.count;
      src = 0;
      handle = source._h;
      stride = source.stride;
      offs = [source.channels[cx] ?? 0, source.channels[cy] ?? 1, source.channels[cz] ?? 2];
    } else if (source instanceof PointCloud) {
      n = source.count;
      src = 2;
      handle = source._h;
    } else if (source instanceof Emitter) {
      n = source.count;
      src = 3;
      handle = source._h;
    } else {
      n = Math.floor(source.length / 3);
      src = 1;
      if (this.ownLen !== n * 3) this.ownLen = n * 3;
      const ptr = w.ak_projector_points(this.pid, n * 3);
      new Float32Array(W!.memory.buffer, ptr, n * 3).set(source as ArrayLike<number>);
    }
    const at = f64Buf(32);
    F64.set(mats, at);
    const p = w.ak_project(this.pid, src, handle, stride, offs[0], offs[1], offs[2], n, this.width, this.height, o.sort === false ? 0 : 1);
    const buf = W!.memory.buffer;
    return {
      data: new Float32Array(buf, p, n * 4),
      order: new Uint32Array(buf, w.ak_projector_order(this.pid), n),
      count: n,
    };
  }

  /** Project and draw points on the scene's canvas (sorted, size by depth, fog). */
  draw(source: Batch | PointCloud | Emitter | ArrayLike<number>, o: DrawOptions = {}): Projection {
    const proj = this.project(source, { channels: o.channels });
    const ctx = this.canvas?.getContext('2d');
    if (!ctx) return proj;
    if (o.clear !== false) {
      if (typeof o.clear === 'string') {
        ctx.fillStyle = o.clear;
        ctx.fillRect(0, 0, this.width, this.height);
      } else ctx.clearRect(0, 0, this.width, this.height);
    }
    const { data, order, count } = proj;
    const size = o.size ?? 2;
    const fog = o.fog ?? 0.6;
    const op = o.opacity ?? 1;
    const camDist = Math.hypot(this.camera.x - this.target.x, this.camera.y - this.target.y, this.camera.z - this.target.z) || 1;
    const fixed = typeof o.color === 'string' || o.color === undefined ? (o.color as string | undefined) ?? '#ffffff' : null;
    if (fixed) ctx.fillStyle = fixed;
    for (let k = 0; k < count; k++) {
      const i = order[k];
      const sc = data[i * 4 + 3];
      if (sc <= 0) continue;
      const d = data[i * 4 + 2];
      const r = size * sc * camDist * 0.002;
      if (r < 0.15) continue;
      ctx.globalAlpha = op * (fog > 0 ? Math.max(0.05, Math.min(1, 1 - fog * ((d - camDist) / camDist))) : 1);
      if (!fixed) ctx.fillStyle = (o.color as (i: number, d: number, s: number) => string)(i, d, sc);
      const x = data[i * 4];
      const y = data[i * 4 + 1];
      if (o.squares || r < 1.2) ctx.fillRect(x - r, y - r, r * 2, r * 2);
      else {
        ctx.beginPath();
        ctx.arc(x, y, r, 0, Math.PI * 2);
        ctx.fill();
      }
    }
    ctx.globalAlpha = 1;
    return proj;
  }

  /** Drag to rotate with momentum; optional idle auto-rotation (deg/s). */
  orbit(o: { element?: Element; autoRotate?: number; sensitivity?: number; inertia?: boolean; clampX?: number } = {}): this {
    const el = (o.element ?? this.canvas ?? document.body) as HTMLElement;
    const sens = o.sensitivity ?? 0.3;
    const clampX = o.clampX ?? 80;
    let dragging = false;
    let lx = 0;
    let ly = 0;
    const vx = new VelocityTracker();
    const vy = new VelocityTracker();
    const down = (e: PointerEvent) => {
      dragging = true;
      lx = e.clientX;
      ly = e.clientY;
      vx.reset();
      vy.reset();
      el.setPointerCapture?.(e.pointerId);
    };
    const move = (e: PointerEvent) => {
      if (!dragging) return;
      this.rotation.y += (e.clientX - lx) * sens;
      this.rotation.x = Math.max(-clampX, Math.min(clampX, this.rotation.x - (e.clientY - ly) * sens));
      lx = e.clientX;
      ly = e.clientY;
      vx.add(this.rotation.y);
      vy.add(this.rotation.x);
    };
    const up = () => {
      if (!dragging) return;
      dragging = false;
      if (o.inertia !== false) {
        inertia(this.rotation, { y: vx.velocity, x: { velocity: vy.velocity, min: -clampX, max: clampX } }, { timeConstant: 0.5 });
      }
    };
    el.addEventListener('pointerdown', down);
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', up);
    el.style.touchAction = 'none';
    const auto = o.autoRotate ?? 0;
    const off = auto ? engine.onFrame((_t, dt) => !dragging && (this.rotation.y += (auto * dt) / 1000)) : () => {};
    this.offs.push(() => {
      el.removeEventListener('pointerdown', down);
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', up);
      off();
    });
    return this;
  }

  destroy(): void {
    for (const f of this.offs.splice(0)) f();
    core().ak_projector_free(this.pid);
  }
  revert(): void {
    this.destroy();
  }
}

/** Create a 3D scene / projector. */
export function scene3d(options?: SceneOptions): Scene3D {
  return new Scene3D(options);
}

// ---------------------------------------------------------------------------
// tilt()
// ---------------------------------------------------------------------------

export interface TiltOptions {
  /** Max rotation in degrees (default 12). */
  max?: number;
  /** Perspective distance in px (default 800). */
  perspective?: number;
  /** Scale while hovered (default 1.03). */
  scale?: number;
  stiffness?: number;
  damping?: number;
  /** Optional callback with normalised pointer position (-1…1) for glare effects. */
  onMove?: (el: Element, x: number, y: number) => void;
}

/** 3D hover tilt that follows the pointer with a spring. */
export function tilt(targets: Targets, o: TiltOptions = {}) {
  const max = o.max ?? 12;
  const sp = { stiffness: o.stiffness ?? 260, damping: o.damping ?? 20 };
  const offs: Array<() => void> = [];
  for (const el of toTargets(targets) as HTMLElement[]) {
    const s = stateOf(el);
    s.t[11] = o.perspective ?? 800;
    markDirty(s);
    const move = (e: PointerEvent) => {
      const r = el.getBoundingClientRect();
      const nx = ((e.clientX - r.left) / r.width) * 2 - 1;
      const ny = ((e.clientY - r.top) / r.height) * 2 - 1;
      spring(el, { rotateY: nx * max, rotateX: -ny * max, scale: o.scale ?? 1.03 }, sp);
      o.onMove?.(el, nx, ny);
    };
    const leave = () => {
      spring(el, { rotateY: 0, rotateX: 0, scale: 1 }, sp);
      o.onMove?.(el, 0, 0);
    };
    el.addEventListener('pointermove', move);
    el.addEventListener('pointerleave', leave);
    offs.push(() => {
      el.removeEventListener('pointermove', move);
      el.removeEventListener('pointerleave', leave);
    });
  }
  const api = { kill: () => offs.splice(0).forEach((f) => f()), revert: () => api.kill() };
  track(api);
  return api;
}
