/**
 * `emitter()` — a 3D particle emitter simulated in WebAssembly.
 *
 * Continuous emission and bursts from a point, sphere, box or ring; launch
 * cone, gravity, drag, turbulence, an attractor; size and colour over life.
 * Render it as light or ink with `glow().render(scene, emitter)`, project it
 * with `scene3d().project(emitter)`, or read `positions` yourself.
 *
 * ```ts
 * const sparks = emitter({ rate: 3000, life: 1.2, speed: 600, spread: 0.5,
 *   gravity: [0, -900, 0], colors: ['#fff', '#ffd27a', '#ff3b14', '#200'] });
 * engine.onFrame(() => g.render(scene, sparks, { size: 2 }));
 * sparks.burst(2000);                  // a one-shot explosion
 * ```
 */
import { core, F64, f64Buf, W } from './wasm';
import { engine } from './core';
import { track } from './context';
import { parseColor } from './values';

export type SpawnShape = 'point' | 'sphere' | 'box' | 'ring';
type V3 = [number, number, number];

export interface EmitterOptions {
  /** Maximum live particles (default 20,000). */
  capacity?: number;
  /** Particles per second (default 1000; 0 for bursts only). */
  rate?: number;
  /** Lifetime in seconds (default 2) and its ± variation fraction (default 0.3). */
  life?: number;
  lifeVariation?: number;
  origin?: V3;
  shape?: SpawnShape;
  /** Sphere/ring radius, or box half-extents. */
  size?: number | V3;
  /** Launch direction (default up), speed (default 200 units/s) and its variation. */
  direction?: V3;
  speed?: number;
  speedVariation?: number;
  /** Cone half-angle in radians (default 0.4; Math.PI = every direction). */
  spread?: number;
  gravity?: V3;
  /** Velocity damping per second (default 0.4). */
  drag?: number;
  /** Turbulence strength (units/s²) and spatial frequency. */
  turbulence?: number;
  frequency?: number;
  /** Pull toward (or, negative, push from) a point. */
  attractor?: V3;
  attract?: number;
  /** Size multiplier at birth and death (default 1 → 0). */
  sizeStart?: number;
  sizeEnd?: number;
  /** Colour over life: up to 4 CSS colours (birth → death). */
  colors?: string[];
  /** Step automatically each frame (default true). */
  autoStep?: boolean;
  seed?: number;
}

const SHAPES: Record<SpawnShape, number> = { point: 0, sphere: 1, box: 2, ring: 3 };

export class Emitter {
  readonly capacity: number;
  /** Live particle count. */
  count = 0;
  private id: number;
  private o: Required<Omit<EmitterOptions, 'capacity' | 'autoStep' | 'seed' | 'colors' | 'size'>> & { size: V3; colors: string[] };
  private gradient: number[] = [];
  private off: (() => void) | null = null;
  paused = false;

  constructor(o: EmitterOptions = {}) {
    this.capacity = o.capacity ?? 20_000;
    this.id = core().ak_emitter_create(this.capacity, o.seed ?? 1);
    const s = o.size ?? 0;
    this.o = {
      rate: o.rate ?? 1000, life: o.life ?? 2, lifeVariation: o.lifeVariation ?? 0.3, origin: o.origin ?? [0, 0, 0],
      shape: o.shape ?? 'point', size: typeof s === 'number' ? [s, s, s] : s, direction: o.direction ?? [0, 1, 0],
      speed: o.speed ?? 200, speedVariation: o.speedVariation ?? 0.4, spread: o.spread ?? 0.4, gravity: o.gravity ?? [0, -300, 0],
      drag: o.drag ?? 0.4, turbulence: o.turbulence ?? 0, frequency: o.frequency ?? 0.01, attractor: o.attractor ?? [0, 0, 0],
      attract: o.attract ?? 0, sizeStart: o.sizeStart ?? 1, sizeEnd: o.sizeEnd ?? 0, colors: o.colors ?? ['#ffffff', '#ffffff', '#ffffff', '#ffffff'],
    };
    this.set({});
    if (o.autoStep !== false) this.off = engine.onFrame((_t, dt) => !this.paused && this.step(dt / 1000));
    track(this);
  }

  /** Update any option live (e.g. move the origin with the pointer). */
  set(o: Omit<EmitterOptions, 'capacity' | 'autoStep' | 'seed'>): this {
    const size = o.size;
    Object.assign(this.o, o, size === undefined ? {} : { size: typeof size === 'number' ? [size, size, size] : size });
    const p = this.o;
    const at = f64Buf(28);
    F64.set([p.rate, p.life, p.lifeVariation, ...p.origin, SHAPES[p.shape] ?? 0, ...p.size, ...p.direction, p.speed, p.speedVariation, p.spread,
      ...p.gravity, p.drag, p.turbulence, p.frequency, ...p.attractor, p.attract, p.sizeStart, p.sizeEnd], at);
    core().ak_emitter_params(this.id);
    if (o.colors) this.gradient = [];
    return this;
  }

  /** Spawn `n` particles immediately. */
  burst(n: number): this {
    core().ak_emitter_burst(this.id, n);
    this.count = core().ak_emitter_len(this.id);
    return this;
  }

  /** Advance `dt` seconds (automatic unless `autoStep: false`). */
  step(dt: number): void {
    this.count = core().ak_emitter_step(this.id, dt);
  }

  /** Compute colours and sizes over life (renderers call this). */
  shade(): void {
    if (!this.gradient.length) {
      const cs = this.o.colors.length ? this.o.colors : ['#fff'];
      const stops = [0, 1, 2, 3].map((k) => cs[Math.min(cs.length - 1, Math.round((k / 3) * (cs.length - 1)))]);
      this.gradient = stops.flatMap((c) => (parseColor(c) ?? [255, 255, 255, 1]).slice(0, 3).map((v) => v / 255));
    }
    const at = f64Buf(12);
    F64.set(this.gradient, at);
    core().ak_emitter_shade(this.id);
  }

  /** Live positions `[x, y, z] * count` (zero-copy view). */
  get positions(): Float32Array {
    return new Float32Array(W!.memory.buffer, core().ak_emitter_ptr(this.id), this.count * 3);
  }
  /** Per-particle linear RGB after `shade()`. */
  get colors(): Float32Array {
    return new Float32Array(W!.memory.buffer, core().ak_emitter_colors(this.id), this.count * 3);
  }
  /** Per-particle size multiplier after `shade()`. */
  get sizes(): Float32Array {
    return new Float32Array(W!.memory.buffer, core().ak_emitter_sizes(this.id), this.count);
  }

  /**
   * Draw as 2D dots on a canvas (x, y in canvas pixels; z ignored). For 3D,
   * use `glow().render(scene, emitter)`.
   */
  draw(ctx: CanvasRenderingContext2D, o: { size?: number; clear?: boolean } = {}): void {
    this.shade();
    if (o.clear) ctx.clearRect(0, 0, ctx.canvas.width, ctx.canvas.height);
    const p = this.positions, c = this.colors, s = this.sizes, base = o.size ?? 3;
    for (let i = 0; i < this.count; i++) {
      const r = base * s[i];
      if (r <= 0.05) continue;
      ctx.fillStyle = `rgb(${(c[i * 3] * 255) | 0},${(c[i * 3 + 1] * 255) | 0},${(c[i * 3 + 2] * 255) | 0})`;
      ctx.fillRect(p[i * 3] - r, p[i * 3 + 1] - r, r * 2, r * 2);
    }
  }

  clear(): this {
    core().ak_emitter_clear(this.id);
    this.count = 0;
    return this;
  }
  pause(): this {
    this.paused = true;
    return this;
  }
  play(): this {
    this.paused = false;
    return this;
  }
  /** @internal projector source handle */
  get _h(): number {
    return this.id;
  }
  destroy(): void {
    this.off?.();
    this.off = null;
    if (this.id >= 0) core().ak_emitter_free(this.id);
    this.id = -1;
  }
  kill(): void {
    this.destroy();
  }
  revert(): void {
    this.destroy();
  }
}

/** Create a particle emitter. */
export function emitter(options?: EmitterOptions): Emitter {
  return new Emitter(options);
}
