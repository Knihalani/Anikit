/**
 * Point clouds and glow rendering.
 *
 * `pointCloud()` keeps up to millions of particles in WebAssembly, each one
 * spring-following a blend between target shapes with its own timing and a
 * turbulence field — organic, interruptible morphs you can scrub from scroll.
 * `glow()` renders projected points as additive light straight into pixels
 * (sub-pixel splats, bokeh discs, fog, pointer lens, motion trails, tone
 * mapping), all in the core: JavaScript does one `putImageData` per frame.
 *
 * ```ts
 * const cloud = pointCloud(150_000, {
 *   shapes: [sphere, (i, n) => [x, y, z], textPoints('anikit', 150_000)],
 * });
 * const scene = scene3d({ fov: 40, camera: { z: 900 } });
 * const g = glow('#bg', { resolution: 0.75, trail: 0.85 });
 * g.colors(cloud.count, (i) => [0.5, 0.55, 1]);
 * engine.onFrame(() => g.render(scene, cloud, { size: 1.4, lens: pointer }));
 * cloud.morph(2, { duration: 2.5 });
 * ```
 */
import { core, F64, f64Buf, W } from './wasm';
import { engine } from './core';
import { animate } from './animate';
import { track } from './context';
import type { Scene3D } from './scene';
import type { EasingInput } from './core';

export type ShapeInput = ArrayLike<number> | ((i: number, count: number) => [number, number, number] | number[]);

export interface CloudOptions {
  /** Target shapes (index 0 is the starting shape). */
  shapes?: ShapeInput[];
  /** Spring stiffness toward the target (default 60). */
  stiffness?: number;
  /** Velocity damping (default 9). */
  damping?: number;
  /** 0 = every particle moves at once; higher = a rippling transition (default 0.6). */
  spread?: number;
  /** Resting turbulence in world units (default 2). */
  noise?: number;
  /** Extra turbulence mid-transition (default 40). */
  swirl?: number;
  /** Turbulence spatial frequency (default 0.01). */
  frequency?: number;
  /** Step automatically every frame (default true). */
  autoStep?: boolean;
  seed?: number;
}

export class PointCloud {
  readonly count: number;
  /** Blend state: particles follow `mix(shape[from], shape[to], mix)`. */
  from = 0;
  to = 0;
  mix = 0;
  private id: number;
  private off: (() => void) | null = null;
  private tween: { kill(): void } | null = null;
  private params: Required<Omit<CloudOptions, 'shapes' | 'autoStep' | 'seed'>>;
  shapes = 0;

  constructor(count: number, o: CloudOptions = {}) {
    this.count = count;
    this.id = core().ak_cloud_create(count, 0, o.seed ?? 1);
    this.params = {
      stiffness: o.stiffness ?? 60,
      damping: o.damping ?? 9,
      spread: o.spread ?? 0.6,
      noise: o.noise ?? 2,
      swirl: o.swirl ?? 40,
      frequency: o.frequency ?? 0.01,
    };
    this.set({});
    (o.shapes ?? []).forEach((s, k) => this.shape(k, s));
    if (this.shapes) core().ak_cloud_snap(this.id, 0);
    if (o.autoStep !== false) this.off = engine.onFrame((_t, dt) => this.step(dt / 1000));
    track(this);
  }

  /** Update tunables. */
  set(o: Partial<Omit<CloudOptions, 'shapes' | 'autoStep' | 'seed'>>): this {
    Object.assign(this.params, o);
    const p = this.params;
    const at = f64Buf(6);
    F64.set([p.stiffness, p.damping, p.spread, p.noise, p.swirl, p.frequency], at);
    core().ak_cloud_params(this.id);
    return this;
  }

  /** Define shape `k` from a flat xyz array or `(i, count) => [x, y, z]`. */
  shape(k: number, s: ShapeInput): this {
    const n = this.count * 3;
    let src: ArrayLike<number>;
    if (typeof s === 'function') {
      // Resolve user code before touching wasm memory.
      const a = new Float32Array(n);
      for (let i = 0; i < this.count; i++) {
        const p = s(i, this.count);
        a[i * 3] = p[0];
        a[i * 3 + 1] = p[1];
        a[i * 3 + 2] = p[2];
      }
      src = a;
    } else src = s.length > n ? Array.prototype.slice.call(s, 0, n) : s;
    const ptr = core().ak_cloud_shape(this.id, k);
    new Float32Array(W!.memory.buffer, ptr, n).set(src);
    this.shapes = Math.max(this.shapes, k + 1);
    return this;
  }

  /** Jump every particle onto shape `k`. */
  snap(k: number): this {
    this.tween?.kill();
    this.from = this.to = k;
    this.mix = 0;
    core().ak_cloud_snap(this.id, k);
    return this;
  }

  /**
   * Flow to shape `k`. Without options the blend is left for you to drive
   * (e.g. `cloud.mix = scrollProgress`); with a duration it is tweened.
   */
  morph(k: number, o: { duration?: number; ease?: EasingInput; from?: number } = {}): this {
    this.tween?.kill();
    this.from = o.from ?? (this.mix >= 0.5 ? this.to : this.from);
    this.to = k;
    this.mix = 0;
    if (o.duration !== undefined) this.tween = animate(this as any, { mix: 1 }, { duration: o.duration, ease: o.ease ?? 'linear' });
    return this;
  }

  /** Advance the simulation (automatic unless `autoStep: false`). */
  step(dt: number): void {
    core().ak_cloud_step(this.id, dt, this.from, this.to, this.mix);
  }

  /** Radial velocity kick from a world-space point (negative pulls in). */
  explode(x = 0, y = 0, z = 0, strength = 400): this {
    core().ak_cloud_explode(this.id, x, y, z, strength);
    return this;
  }

  /** Live positions `[x, y, z] * count` (zero-copy; valid until memory grows). */
  get positions(): Float32Array {
    return new Float32Array(W!.memory.buffer, core().ak_cloud_ptr(this.id), this.count * 3);
  }

  /** Mean particle speed (world units / s). */
  get speed(): number {
    return core().ak_cloud_speed(this.id);
  }

  /** @internal projector source handle */
  get _h(): number {
    return this.id;
  }

  destroy(): void {
    this.off?.();
    this.off = null;
    this.tween?.kill();
    if (this.id >= 0) core().ak_cloud_free(this.id);
    this.id = -1;
  }
  kill(): void {
    this.destroy();
  }
  revert(): void {
    this.destroy();
  }
}

/** A particle cloud that flows between shapes. */
export function pointCloud(count: number, options?: CloudOptions): PointCloud {
  return new PointCloud(count, options);
}

// ---------------------------------------------------------------------------
// Shape helpers
// ---------------------------------------------------------------------------

/**
 * Sample `count` points from rendered text (or any canvas-drawable glyphs),
 * centred at the origin, `width` world units wide, `depth` units thick.
 */
export function textPoints(
  text: string,
  count: number,
  o: { font?: string; width?: number; depth?: number; weight?: number | string; lineHeight?: number } = {},
): Float32Array {
  const out = new Float32Array(count * 3);
  if (typeof document === 'undefined') return out;
  const lines = text.split('\n');
  const fontPx = 200;
  const c = document.createElement('canvas');
  const ctx = c.getContext('2d', { willReadFrequently: true } as CanvasRenderingContext2DSettings)!;
  const font = `${o.weight ?? 800} ${fontPx}px ${o.font ?? 'Inter, system-ui, sans-serif'}`;
  ctx.font = font;
  const lh = fontPx * (o.lineHeight ?? 1.05);
  const tw = Math.max(...lines.map((l) => ctx.measureText(l).width), 1);
  c.width = Math.ceil(tw + 40);
  c.height = Math.ceil(lh * lines.length + 40);
  ctx.font = font;
  ctx.fillStyle = '#fff';
  ctx.textBaseline = 'middle';
  ctx.textAlign = 'center';
  lines.forEach((l, i) => ctx.fillText(l, c.width / 2, 20 + lh * (i + 0.5)));
  const data = ctx.getImageData(0, 0, c.width, c.height).data;
  const filled: number[] = [];
  for (let y = 0; y < c.height; y += 2) for (let x = 0; x < c.width; x += 2) if (data[(y * c.width + x) * 4 + 3] > 128) filled.push(x, y);
  const m = filled.length / 2;
  if (!m) return out;
  const s = (o.width ?? 800) / tw;
  const depth = o.depth ?? 40;
  let seed = 12345;
  const rnd = () => ((seed = (seed * 1664525 + 1013904223) >>> 0) / 4294967296);
  for (let i = 0; i < count; i++) {
    const k = Math.floor(rnd() * m);
    out[i * 3] = (filled[k * 2] + rnd() * 2 - c.width / 2) * s;
    out[i * 3 + 1] = -(filled[k * 2 + 1] + rnd() * 2 - c.height / 2) * s;
    out[i * 3 + 2] = (rnd() - 0.5) * depth;
  }
  return out;
}

// ---------------------------------------------------------------------------
// glow(): additive splat renderer
// ---------------------------------------------------------------------------

export interface GlowOptions {
  /** Pixel buffer resolution relative to CSS pixels (default min(dpr, 1) * 0.75). */
  resolution?: number;
  /** Background colour (default '#000'). */
  background?: string;
  /** Exposure / brightness (default 1). */
  exposure?: number;
  /** Fraction of light kept between frames: 0 = no trails (default), 0.9 = long trails. */
  trail?: number;
  /**
   * Frame budget in ms (default 7). When rendering is consistently slower,
   * the pixel resolution steps down (to at most `minResolution`) so slow
   * machines keep their frame rate. 0 disables.
   */
  budget?: number;
  /** Lowest resolution adaptive quality may use (default 0.4). */
  minResolution?: number;
}

export interface GlowRenderOptions {
  /** Point radius in world units (default 1). */
  size?: number;
  /** Light per point (default 0.25). */
  intensity?: number;
  /** Depth fade `[near, far]` measured from the camera. */
  fog?: [number, number];
  /** Push points away from a screen point (CSS px) — e.g. the pointer. */
  lens?: { x: number; y: number; radius?: number; strength?: number } | null;
  /** Largest bokeh radius in buffer pixels (default 6). */
  maxRadius?: number;
  /** Override exposure for this frame. */
  exposure?: number;
}

function parseHex(c: string): number {
  const m = /^#?([0-9a-f]{3}|[0-9a-f]{6})$/i.exec(c.trim());
  if (!m) return 0;
  let h = m[1];
  if (h.length === 3) h = h.replace(/./g, (x) => x + x);
  return parseInt(h, 16);
}

export class Glow {
  canvas: HTMLCanvasElement;
  readonly options: Required<GlowOptions>;
  width = 0;
  height = 0;
  private id: number;
  private ctx: CanvasRenderingContext2D;
  private img: ImageData | null = null;
  private imgPtr = 0;
  private imgBuf: ArrayBuffer | null = null;
  private bw = 0;
  private bh = 0;
  /** Milliseconds the last `render()` spent in WebAssembly + putImageData. */
  lastMs = 0;
  /** Smoothed render cost (ms). */
  avgMs = 0;
  private slow = 0;

  constructor(canvas: HTMLCanvasElement | string, o: GlowOptions = {}) {
    const c = typeof canvas === 'string' ? document.querySelector<HTMLCanvasElement>(canvas) : canvas;
    if (!c) throw new Error(`anikit glow: canvas "${canvas}" not found`);
    this.canvas = c;
    const dpr = typeof devicePixelRatio === 'number' ? devicePixelRatio : 1;
    this.options = {
      resolution: o.resolution ?? Math.min(dpr, 1) * 0.75,
      background: o.background ?? '#000000',
      exposure: o.exposure ?? 1,
      trail: o.trail ?? 0,
      budget: o.budget ?? 7,
      minResolution: o.minResolution ?? 0.4,
    };
    this.ctx = c.getContext('2d', { alpha: false })!;
    this.id = core().ak_splat_create(1, 1);
    this.resize(c.clientWidth || c.width, c.clientHeight || c.height);
    track(this);
  }

  /** Resize to CSS size `w × h` (call your scene's `resize(w, h)` with the same numbers). */
  resize(w: number, h: number): this {
    this.width = w;
    this.height = h;
    this.bw = Math.max(1, Math.round(w * this.options.resolution));
    this.bh = Math.max(1, Math.round(h * this.options.resolution));
    this.canvas.width = this.bw;
    this.canvas.height = this.bh;
    this.canvas.style.width = w + 'px';
    this.canvas.style.height = h + 'px';
    core().ak_splat_resize(this.id, this.bw, this.bh);
    this.img = null;
    return this;
  }

  /** Per-point colours: linear RGB in 0…1, as `(i) => [r, g, b]` or a flat array. */
  colors(count: number, c: ArrayLike<number> | ((i: number) => ArrayLike<number>)): this {
    let src: ArrayLike<number> = c as ArrayLike<number>;
    if (typeof c === 'function') {
      const a = new Float32Array(count * 3);
      for (let i = 0; i < count; i++) {
        const v = c(i);
        a[i * 3] = v[0];
        a[i * 3 + 1] = v[1];
        a[i * 3 + 2] = v[2];
      }
      src = a;
    }
    const ptr = core().ak_splat_colors(this.id, count);
    new Float32Array(W!.memory.buffer, ptr, count * 3).set(src);
    return this;
  }

  /** Project `source` through `scene` and paint it as light. */
  render(scene: Scene3D, source: Parameters<Scene3D['project']>[0], o: GlowRenderOptions = {}): void {
    const t0 = performance.now();
    const w = core();
    // Project in CSS pixels; the splat scales to the buffer resolution.
    scene.width = this.width;
    scene.height = this.height;
    const proj = scene.project(source, { sort: false });
    const at = f64Buf(10);
    const l = o.lens;
    const fog = o.fog ?? [0, 0];
    F64.set(
      [o.size ?? 1, o.intensity ?? 0.25, fog[0], fog[1], this.options.resolution, l ? l.x : 0, l ? l.y : 0, l ? l.radius ?? 140 : 0, l ? l.strength ?? 40 : 0, o.maxRadius ?? 6],
      at,
    );
    w.ak_splat_draw(this.id, scene._pid, proj.count);
    // Tone-map and decay the light for the next frame in one pass.
    const ptr = w.ak_splat_resolve(this.id, o.exposure ?? this.options.exposure, parseHex(this.options.background), this.options.trail);
    const buf = W!.memory.buffer;
    if (!this.img || this.imgPtr !== ptr || this.imgBuf !== buf) {
      this.img = new ImageData(new Uint8ClampedArray(buf, ptr, this.bw * this.bh * 4), this.bw, this.bh);
      this.imgPtr = ptr;
      this.imgBuf = buf;
    }
    this.ctx.putImageData(this.img, 0, 0);
    const ms = performance.now() - t0;
    this.lastMs = ms;
    this.avgMs = this.avgMs ? this.avgMs * 0.95 + ms * 0.05 : ms;
    const b = this.options.budget;
    if (b > 0 && this.options.resolution > this.options.minResolution) {
      this.slow = this.avgMs > b ? this.slow + 1 : 0;
      if (this.slow > 45) {
        // Adaptive quality: fewer pixels, same frame rate.
        this.options.resolution = Math.max(this.options.minResolution, this.options.resolution * 0.85);
        this.resize(this.width, this.height);
        this.slow = 0;
        this.avgMs *= 0.75;
      }
    }
  }

  destroy(): void {
    if (this.id >= 0) core().ak_splat_free(this.id);
    this.id = -1;
  }
  kill(): void {
    this.destroy();
  }
  revert(): void {
    this.destroy();
  }
}

/** An additive-light point renderer on `canvas`. */
export function glow(canvas: HTMLCanvasElement | string, options?: GlowOptions): Glow {
  return new Glow(canvas, options);
}
