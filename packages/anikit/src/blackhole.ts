/**
 * A physically based, animated Schwarzschild black hole rendered to pixels.
 *
 * Exact null geodesics (one Binet-equation table per observer distance),
 * a Page–Thorne accretion disk coloured by the shifted blackbody at g·T,
 * g⁴ relativistic beaming, the lensed secondary image and photon ring, a
 * lensed star catalogue and Milky Way, orbiting hot spots, an infalling
 * (raindrop) observer that can cross the horizon, bloom and filmic tone
 * mapping — all in WebAssembly. JavaScript does one `putImageData` a frame.
 *
 * ```ts
 * const bh = blackHole({ width: 1280, height: 800, canvas: '#hero' });
 * engine.onFrame((t) => bh.render({ time: t / 1000, distance: 30, inclination: 84 }));
 * ```
 */
import { core, F64, f64Buf, W } from './wasm';
import { track } from './context';
import { checkKeys, warn, type OptionKeys } from './debug';

/** Construction options for {@link blackHole}. */
export interface BlackHoleOptions {
  /** Output width in pixels (default: the canvas width, else 640). */
  width?: number;
  /** Output height in pixels (default: the canvas height, else 400). */
  height?: number;
  /** Canvas (or selector) to draw each frame into. Optional: `render()` always returns the pixels. */
  canvas?: HTMLCanvasElement | string;
  /** 'physical': Doppler colour and g⁴ beaming (EHT / Luminet). 'cinematic': Interstellar-style, no Doppler colour, softened beaming. Default 'physical'. */
  mode?: 'physical' | 'cinematic';
  /** Internal resolution relative to `width`/`height` (default 1). The canvas is drawn at this size; CSS scales it up. */
  scale?: number;
  /** Seed for the disk turbulence, stars and nebula (default 1). */
  seed?: number;
}

/** Per-frame parameters for {@link BlackHole.render}. Every field is optional. */
export interface BlackHoleFrame {
  /** Observer distance from the centre in black-hole masses M (horizon at 2, photon sphere at 3, ISCO at 6). Default 30. */
  distance?: number;
  /** Angle between the disk's axis and the line of sight, degrees (90 = edge-on). Default 80. */
  inclination?: number;
  /** Vertical field of view, degrees. Default 40. */
  fov?: number;
  /** Time in seconds: drives the disk flow, hot spots and jet. Default 0. */
  time?: number;
  /** Exposure multiplier. Default 1. */
  exposure?: number;
  /** Bloom strength (0 = off). Default 1. */
  bloom?: number;
  /** Film grain strength (0 = off). Default 1. */
  grain?: number;
  /** Strength of orbiting hot-spot flares (0 = none). Default 1. */
  hotSpots?: number;
  /** Strength of the faint bipolar jet glow (0 = none). Default 0. */
  jet?: number;
  /** Camera orbit around the disk axis, degrees (cheap: no geometry rebuild). Default 0. */
  azimuth?: number;
  /** Look away from the hole, degrees (0 = straight at it, 180 = straight back out). Default 0. */
  look?: number;
  /** Camera roll, degrees. Default 0. */
  roll?: number;
  /** Observer infall: 0 = hovering (static), 1 = free fall from rest at infinity (raindrop). Inside the horizon the observer always falls. Default 0. */
  fall?: number;
  /** Peak disk temperature in kelvin. Default 9000 (physical) / 6500 (cinematic). */
  temperature?: number;
  /** Outer disk radius in M. Default 22. */
  diskOuter?: number;
  /** Star brightness (0 = none). Default 1. */
  stars?: number;
  /** Disk flow speed multiplier. Default 1. */
  speed?: number;
  /** Milky Way / nebula brightness (0 = none). Default 1. */
  nebula?: number;
}

/** Counters from the last rendered frame. */
export interface BlackHoleStats {
  /** Disk samples shaded per image order: [direct, secondary (over/under arc), photon ring]. */
  samples: [number, number, number];
  /** Lensed star images drawn. */
  stars: number;
  /** True if the geodesic table was re-integrated this frame (distance, fall or field of view changed). */
  tableBuilt: boolean;
  /** True if the per-pixel lensing geometry was rebuilt this frame (camera changed). */
  geometryBuilt: boolean;
  /** Angular radius of the shadow in the observer's frame, radians (-1 if none). */
  shadowRadius: number;
  /** Milliseconds the last `render()` took (WebAssembly + canvas upload). */
  ms: number;
  /** Output size in pixels after `scale`. */
  width: number;
  /** Output height in pixels after `scale`. */
  height: number;
}

const OPTION_KEYS: OptionKeys<BlackHoleOptions> = { width: 0, height: 0, canvas: 0, mode: 0, scale: 0, seed: 0 };
const FRAME_KEYS: OptionKeys<BlackHoleFrame> = {
  distance: 0, inclination: 0, fov: 0, time: 0, exposure: 0, bloom: 0, grain: 0, hotSpots: 0, jet: 0,
  azimuth: 0, look: 0, roll: 0, fall: 0, temperature: 0, diskOuter: 0, stars: 0, speed: 0, nebula: 0,
};

/** A black-hole renderer; create with {@link blackHole}. */
export class BlackHole {
  /** The canvas frames are drawn into, if any. */
  readonly canvas: HTMLCanvasElement | null = null;
  /** Rendering mode chosen at construction. */
  readonly mode: 'physical' | 'cinematic';
  private id: number;
  private w = 0;
  private h = 0;
  private cssW: number;
  private cssH: number;
  private scale: number;
  private ctx: CanvasRenderingContext2D | null = null;
  private img: ImageData | null = null;
  private checked = false;
  private last: BlackHoleStats = {
    samples: [0, 0, 0], stars: 0, tableBuilt: false, geometryBuilt: false, shadowRadius: -1, ms: 0, width: 0, height: 0,
  };

  constructor(o: BlackHoleOptions = {}) {
    checkKeys('blackHole()', o, OPTION_KEYS);
    if (o.mode !== undefined && o.mode !== 'physical' && o.mode !== 'cinematic') {
      warn(`blackHole(): mode must be 'physical' or 'cinematic', got ${JSON.stringify(o.mode)}`);
    }
    this.mode = o.mode === 'cinematic' ? 'cinematic' : 'physical';
    if (o.canvas !== undefined && typeof document !== 'undefined') {
      const c = typeof o.canvas === 'string' ? document.querySelector<HTMLCanvasElement>(o.canvas) : o.canvas;
      if (!c) warn(`blackHole(): canvas ${JSON.stringify(o.canvas)} not found`);
      else {
        this.canvas = c;
        this.ctx = c.getContext('2d', { alpha: false });
      }
    }
    this.cssW = o.width ?? this.canvas?.width ?? 640;
    this.cssH = o.height ?? this.canvas?.height ?? 400;
    this.scale = o.scale ?? 1;
    if (!(this.scale > 0)) {
      warn(`blackHole(): scale must be > 0, got ${o.scale}`);
      this.scale = 1;
    }
    const [w, h] = this.size();
    this.id = core().ak_bh_create(w, h, o.seed ?? 1);
    this.setSize(w, h);
    track(this);
  }

  private size(): [number, number] {
    return [Math.max(4, Math.round(this.cssW * this.scale)), Math.max(4, Math.round(this.cssH * this.scale))];
  }

  private setSize(w: number, h: number): void {
    this.w = w;
    this.h = h;
    this.img = null;
    if (this.canvas) {
      this.canvas.width = w;
      this.canvas.height = h;
    }
  }

  /** Change the output size (and optionally the internal `scale`). */
  resize(width: number, height: number, scale = this.scale): this {
    this.cssW = width;
    this.cssH = height;
    this.scale = scale > 0 ? scale : this.scale;
    const [w, h] = this.size();
    if (w !== this.w || h !== this.h) {
      if (this.id >= 0) core().ak_bh_resize(this.id, w, h);
      this.setSize(w, h);
    }
    return this;
  }

  /**
   * Render one frame. Returns the RGBA pixels (a view into WebAssembly
   * memory, valid until the next call) and draws them into the canvas if
   * one was given. Changing only `time`, `azimuth`, `exposure`, `bloom`,
   * `grain`, `hotSpots`, `jet`, `stars`, `speed` or `nebula` reuses the
   * lensing geometry and is several times cheaper than moving the camera.
   */
  render(f: BlackHoleFrame = {}): Uint8ClampedArray {
    if (!this.checked) {
      checkKeys('blackHole().render()', f, FRAME_KEYS);
      this.checked = true;
    }
    const t0 = typeof performance !== 'undefined' ? performance.now() : Date.now();
    const cine = this.mode === 'cinematic';
    const a = f64Buf(19);
    F64[a] = f.distance ?? 30;
    F64[a + 1] = f.inclination ?? 80;
    F64[a + 2] = f.fov ?? 40;
    F64[a + 3] = f.time ?? 0;
    F64[a + 4] = f.exposure ?? 1;
    F64[a + 5] = f.bloom ?? 1;
    F64[a + 6] = f.grain ?? 1;
    F64[a + 7] = f.hotSpots ?? 1;
    F64[a + 8] = f.jet ?? 0;
    F64[a + 9] = f.azimuth ?? 0;
    F64[a + 10] = f.look ?? 0;
    F64[a + 11] = f.roll ?? 0;
    F64[a + 12] = f.fall ?? 0;
    F64[a + 13] = cine ? 1 : 0;
    F64[a + 14] = f.temperature ?? (cine ? 6500 : 9000);
    F64[a + 15] = f.diskOuter ?? 22;
    F64[a + 16] = f.stars ?? 1;
    F64[a + 17] = f.speed ?? 1;
    F64[a + 18] = f.nebula ?? 1;
    if (this.id < 0) return new Uint8ClampedArray(0);
    const ptr = core().ak_bh_render(this.id);
    const n = this.w * this.h * 4;
    const px = new Uint8ClampedArray(W!.memory.buffer, ptr, n);
    if (this.ctx) {
      if (!this.img || this.img.data.buffer !== W!.memory.buffer || this.img.data.byteOffset !== ptr) {
        this.img = new ImageData(px, this.w, this.h);
      }
      this.ctx.putImageData(this.img, 0, 0);
    }
    this.last.ms = (typeof performance !== 'undefined' ? performance.now() : Date.now()) - t0;
    return px;
  }

  /** Counters and timing from the last frame. */
  get stats(): BlackHoleStats {
    if (this.id >= 0) {
      core().ak_bh_stats(this.id);
      const a = f64Buf(7);
      this.last.samples = [F64[a], F64[a + 1], F64[a + 2]];
      this.last.stars = F64[a + 3];
      this.last.tableBuilt = F64[a + 4] === 1;
      this.last.geometryBuilt = F64[a + 5] === 1;
      this.last.shadowRadius = F64[a + 6];
    }
    this.last.width = this.w;
    this.last.height = this.h;
    return { ...this.last, samples: [...this.last.samples] as [number, number, number] };
  }

  /** Free the WebAssembly memory. */
  destroy(): void {
    if (this.id >= 0) core().ak_bh_free(this.id);
    this.id = -1;
    this.img = null;
  }
  /** Same as {@link destroy}. */
  kill(): void {
    this.destroy();
  }
  /** Same as {@link destroy} (for `context()` cleanup). */
  revert(): void {
    this.destroy();
  }
}

/**
 * Create a black-hole renderer. Call `render()` every frame with the camera
 * and time; see {@link BlackHoleFrame}.
 */
export function blackHole(options?: BlackHoleOptions): BlackHole {
  return new BlackHole(options);
}
