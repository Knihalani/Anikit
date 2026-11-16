/**
 * Video creation: compose scenes in code, render them frame-exactly, export.
 *
 * ```ts
 * const film = composition({ width: 1920, height: 1080, fps: 60, duration: 10, motionBlur: 3 });
 * film.scene('intro', 0, 4, {
 *   enter: () => { cloud.morph(1, { duration: 2 }); },
 *   draw: ({ ctx, t, width, height }) => {
 *     g.render(scene, cloud); ctx.drawImage(g.canvas, 0, 0);
 *     typeFx(ctx, 'Motion, compiled.', 80, height - 80, { size: 120, start: 1, t });
 *   },
 * });
 * const webm = await exportWebM(film);          // browser: WebCodecs, frame-exact
 * // Node: `anikit-render film.mjs --out film.mp4 --workers 4`
 * ```
 *
 * Rendering is deterministic: anikit runs in manual time and is ticked
 * exactly 1/fps per frame (or per motion-blur sub-frame), timelines start on
 * schedule, and nothing depends on wall-clock speed. Motion blur averages
 * sub-frames across a shutter in WebAssembly; frames convert to I420 in
 * WebAssembly too, so encoders get 1.5 bytes per pixel and no conversion work.
 */
import { core, W, ready, F64, f64Buf } from './wasm';
import { engine, isManual } from './core';
import { ease } from './utils';
import { passTimes, created } from './profile';
import { warn } from './debug';
import type { EasingInput } from './core';

type Ctx2D = CanvasRenderingContext2D;
interface CanvasLike {
  width: number;
  height: number;
  getContext(type: '2d', o?: unknown): Ctx2D | null;
}

export interface FrameInfo {
  ctx: Ctx2D;
  /** Composition time (seconds). */
  t: number;
  frame: number;
  /** Time since the scene started, and 0…1 through the scene. */
  local: number;
  progress: number;
  width: number;
  height: number;
  scene: SceneDef;
  /**
   * Engine seconds advanced by this tick (scaled by the scene's `timeScale`;
   * one motion-blur sample's worth when blurring). Step simulations with it.
   */
  dt: number;
  /**
   * The scene's own clock: engine seconds since it started, slowed by its
   * `timeScale` (the sum of every `dt` so far). Choreograph bullet-time shots
   * on it instead of keeping a counter in `step()`; `composition.timeAt()`
   * maps a clock value back to composition time (for impacts and cues).
   */
  clock: number;
}
export interface SceneHooks {
  /** Once, when the scene starts (start tweens, timelines, simulations here). */
  enter?(f: FrameInfo): void;
  /** Every frame (and every motion-blur sample), even when not drawing. */
  step?(f: FrameInfo): void;
  /** Draw the frame. */
  draw(f: FrameInfo): void;
  /**
   * Transition in from the previous scene. The previous scene keeps running
   * and drawing for `duration` seconds after this one starts; both render to
   * their own offscreen canvases and are composited.
   */
  transition?: Transition;
  /** Speed ramp: engine time scale as a function of scene-local seconds (1 = real time, 0 = frozen). */
  timeScale?: (local: number) => number;
  /** Motion-blur samples for this scene (overrides the composition; 1 = off for static shots). */
  motionBlur?: number;
}

/** Composite `from` and `to` at progress `p` (0…1, already eased). */
export type TransitionFn = (ctx: Ctx2D, from: CanvasLike, to: CanvasLike, p: number, width: number, height: number) => void;
export interface Transition {
  duration: number;
  /** Built-in transition or your own compositing function (default 'fade'). */
  type?: 'fade' | 'iris' | 'push' | 'whip' | 'slice' | 'zoom' | 'flash' | 'mask' | TransitionFn;
  /** Easing of the progress (default 'inOutCubic'; 'inOutQuart' for push/whip). */
  ease?: EasingInput;
  /** Iris / zoom / mask centre, 0…1 of the frame (default centre). */
  origin?: [number, number];
  /** Push / whip / slice direction (default 'left': the new scene enters from the right). */
  direction?: 'left' | 'right' | 'up' | 'down';
  /** Slice: number of bands (default 7). */
  slices?: number;
  /** Mask: the letterform the next scene is revealed through (default '●'), and its font. */
  text?: string;
  font?: string;
  /** Flash colour (default '#fff'). */
  color?: string;
}
export interface SceneDef extends SceneHooks {
  name: string;
  start: number;
  end: number;
}

export interface CompositionOptions {
  width: number;
  height: number;
  fps?: number;
  duration: number;
  /** Background painted before every frame (default none). */
  background?: string;
  /** Motion blur: sub-frames per frame, or `{ samples, shutter }` (shutter 0…1, default 0.5 = 180°). */
  motionBlur?: number | { samples: number; shutter?: number };
  /**
   * Film-look grade applied in WebAssembly to every finished frame: bloom
   * (`bloom`, `bloomThreshold`, `bloomRadius`), radial chromatic `aberration`
   * (px at the corners; impacts add to it), `vignette` and per-frame `grain`.
   */
  post?: PostOptions;
}

/** A post value: a number, or a function of composition time in seconds (animate the grade). */
export type PostValue = number | ((t: number) => number);
export type PostColor = [number, number, number] | ((t: number) => [number, number, number]);

export interface PostOptions {
  /** Glow around bright pixels: strength (0–2), luminance threshold (0–1, default 0.7), radius px (default 24). */
  bloom?: PostValue;
  bloomThreshold?: PostValue;
  bloomRadius?: PostValue;
  /** Radial chromatic aberration in px at the corners. */
  aberration?: PostValue;
  /** Darkened corners (0–1). */
  vignette?: PostValue;
  /** Film grain amplitude in 8-bit levels. */
  grain?: PostValue;
  /** Anamorphic streaks: horizontal flares from highlights (strength 0–2), length px (default 160), tint (default cool blue). */
  streaks?: PostValue;
  streakLength?: PostValue;
  streakTint?: PostColor;
  /**
   * Radial zoom blur (speed lines) towards `zoomCenter` ([0.5, 0.5] = middle):
   * 0.1 fast, 0.3 warp. Usually a function of time: `zoomBlur: (t) => t > 3 && t < 4 ? 0.25 : 0`.
   */
  zoomBlur?: PostValue;
  zoomCenter?: [number, number];
  /** Colour grade: exposure (1 = none), contrast (1 = none), saturation (1 = none). */
  exposure?: PostValue;
  contrast?: PostValue;
  saturation?: PostValue;
  /** Split toning: colour added to shadows, and multiplied into highlights (0–1 each, e.g. teal shadows `[0, 0.04, 0.06]`, warm highlights `[0.08, 0.02, -0.04]`). */
  shadows?: PostColor;
  highlights?: PostColor;
  /**
   * God rays: highlights streamed radially from `raysCenter` ([x, y] 0–1 of the frame,
   * e.g. a light behind the city) over `raysLength` (0–1, default 0.6) of the way. Strength 0–2.
   */
  rays?: PostValue;
  raysCenter?: [number, number] | ((t: number) => [number, number]);
  raysLength?: PostValue;
  /** Edge antialiasing (FXAA-style) for 3D geometry, 0–1 (1 recommended with `renderer3d`). */
  aa?: PostValue;
  /**
   * Lens flares (0–2): ghosts of the bright lights mirrored through the frame centre
   * at several scales, tinted like lens coatings, plus a halo ring. Point a sun or a
   * hot light at the lens and the flares follow it.
   */
  flares?: PostValue;
}

/** A hit: camera shake, a flash frame and an aberration kick, all decaying. */
export interface ImpactOptions {
  /** Shake amplitude in px (default 24). */
  shake?: number;
  /** Flash colour (default none) and peak opacity (default 0.85). */
  flash?: string;
  flashOpacity?: number;
  /** Extra chromatic aberration in px at the hit (default 0). */
  aberration?: number;
  /** Decay rate per second (default 10; higher = shorter). */
  decay?: number;
  /**
   * A shockwave: a refracting ring that expands from `origin` (0–1 of the frame,
   * default centre) at `shockSpeed` frame diagonals per second (default 1.1),
   * displacing the picture by up to `shockwave` px (try 20–40) as it passes.
   */
  shockwave?: number;
  origin?: [number, number];
  shockSpeed?: number;
}

/** Where one delivered frame's time went (milliseconds). */
export interface FrameTiming {
  /** Scene `step()` hooks and engine ticks (all sub-frames). */
  step: number;
  /** Scene `draw()` hooks, transitions and impacts (all motion-blur sub-frames). */
  draw: number;
  /** Reading pixels back from the canvas and accumulating motion blur. */
  read: number;
  /** The film grade (`post`). */
  post: number;
  /** `renderer3d` passes inside `draw`, summed over its renders (see `Renderer3D.stats`). */
  passes: Record<string, number>;
}
export interface RenderedFrame {
  index: number;
  t: number;
  ctx: Ctx2D;
  /** Where this frame's time went: see `anikit-render --profile`. */
  timing: FrameTiming;
  /** RGBA pixels of the (motion-blurred) frame. */
  rgba(): Uint8Array;
  /** Planar YUV 4:2:0 (BT.709), converted in WebAssembly. */
  i420(): Uint8Array;
}

export interface RenderOptions {
  canvas: CanvasLike;
  /** First and last frame time (seconds; default whole composition). */
  from?: number;
  to?: number;
  /** Seconds rendered unseen before `from` so trails and buffers are warm (default 0.5). */
  preroll?: number;
  /** Only deliver every Nth frame (previews). Simulation still runs every frame. */
  every?: number;
  /** Read RGBA (or BGRA, see `order`) pixels from the canvas. Default: getImageData. */
  readPixels?: (ctx: Ctx2D, width: number, height: number) => ArrayLike<number>;
  /** Pixel byte order returned by `readPixels`. */
  order?: 'rgba' | 'bgra';
  /** Create offscreen canvases for transitions (Node: pass your canvas library's). */
  createCanvas?: (width: number, height: number) => CanvasLike;
  onFrame(frame: RenderedFrame): void | Promise<void>;
}

/** A WebAssembly frame buffer: motion-blur accumulation + RGBA → I420. */
export class FrameBuffer {
  private id: number;
  constructor(readonly width: number, readonly height: number) {
    this.id = core().ak_video_create(width, height);
  }
  /** Copy pixels in as the current frame. */
  write(px: ArrayLike<number>): this {
    new Uint8Array(W!.memory.buffer, core().ak_video_input(this.id), this.width * this.height * 4).set(px as ArrayLike<number>);
    return this;
  }
  /** Add the current frame to the motion-blur accumulator. */
  accumulate(): number {
    return core().ak_video_add(this.id);
  }
  /** Replace the current frame with the average of the accumulated ones. */
  resolve(): this {
    core().ak_video_resolve(this.id);
    return this;
  }
  /** The current frame (RGBA view, valid until the next call). */
  rgba(): Uint8Array {
    return new Uint8Array(W!.memory.buffer, core().ak_video_input(this.id), this.width * this.height * 4);
  }
  /** Film grade (reads its parameters from the f64 scratch buffer; see Composition). */
  /** Apply the grade from the f64 scratch buffer (`nf` values, see `ak_video_post`). */
  post(seed: number, nf = 6): this {
    core().ak_video_post(this.id, seed >>> 0, nf);
    return this;
  }
  /** I420 of the current frame (view, valid until the next call). */
  i420(order: 'rgba' | 'bgra' = 'rgba'): Uint8Array {
    const p = core().ak_video_i420(this.id, order === 'bgra' ? 1 : 0);
    return new Uint8Array(W!.memory.buffer, p, this.width * this.height * 1.5);
  }
  free(): void {
    if (this.id >= 0) core().ak_video_free(this.id);
    this.id = -1;
  }
}

export class Composition {
  readonly width: number;
  readonly height: number;
  readonly fps: number;
  readonly duration: number;
  readonly scenes: SceneDef[] = [];
  private cues: Array<{ t: number; fn: () => void }> = [];
  private impacts: Array<Required<ImpactOptions> & { t: number }> = [];

  constructor(readonly options: CompositionOptions) {
    this.width = options.width;
    this.height = options.height;
    this.fps = options.fps ?? 60;
    this.duration = options.duration;
  }

  /** Add a scene from `start` to `end` seconds. */
  scene(name: string, start: number, end: number, hooks: SceneHooks): this {
    const where = `anikit composition: scene "${name}"`;
    if (!(end > start)) throw new Error(`${where}: end (${end}) must be after start (${start})`);
    if (typeof hooks?.draw !== 'function') throw new Error(`${where} needs a draw({ ctx, t, local, dt }) function`);
    if (start < 0 || start >= this.duration) throw new Error(`${where} starts at ${start}s, outside the composition (0–${this.duration}s)`);
    const clash = this.scenes.find((s) => start < s.end - 1e-9 && end > s.start + 1e-9 && !(hooks.transition && start >= s.start));
    if (clash && !hooks.transition) {
      throw new Error(`${where} (${start}–${end}s) overlaps "${clash.name}" (${clash.start}–${clash.end}s). Make them adjacent, or give "${name}" a transition: { duration, type } to overlap on purpose.`);
    }
    if (hooks.transition && !(hooks.transition.duration > 0)) throw new Error(`${where}: transition.duration must be > 0`);
    this.scenes.push({ name, start, end, ...hooks });
    this.scenes.sort((a, b) => a.start - b.start);
    return this;
  }
  /**
   * Add a scene of `length` seconds after the last one. (Not called `then`:
   * that would make compositions thenables, which `await` would swallow.)
   */
  append(name: string, length: number, hooks: SceneHooks): this {
    const start = this.scenes.length ? this.scenes[this.scenes.length - 1].end : 0;
    return this.scene(name, start, start + length, hooks);
  }
  /** Run `fn` when the composition reaches `t` seconds. */
  cue(t: number, fn: () => void): this {
    this.cues.push({ t, fn });
    this.cues.sort((a, b) => a.t - b.t);
    return this;
  }

  /**
   * An impact at `t`: decaying camera shake, an optional flash frame and an
   * aberration kick, applied to the finished frame (after transitions).
   */
  impact(t: number, o: ImpactOptions = {}): this {
    this.impacts.push({ t, shake: o.shake ?? 24, flash: o.flash ?? '', flashOpacity: o.flashOpacity ?? 0.85, aberration: o.aberration ?? 0, decay: o.decay ?? 10,
      shockwave: o.shockwave ?? 0, origin: o.origin ?? [0.5, 0.5], shockSpeed: o.shockSpeed ?? 1.1 });
    return this;
  }

  /**
   * The scene clock (`FrameInfo.clock`) at composition time `t`: seconds since
   * scene `name` started, slowed by its `timeScale`. A pure function of time.
   *
   * ```ts
   * film.impact(film.timeAt('knot', 4.2), { shake: 30 }); // when the knot's slowed clock reads 4.2 s
   * ```
   */
  clockAt(name: string, t: number): number {
    const s = this.sceneNamed(name), fps = this.fps;
    let c = 0;
    for (let f = Math.ceil(s.start * fps - 1e-9); f / fps <= t + 1e-9 && f / fps < s.end; f++) {
      const local = f / fps - s.start;
      c += Math.max(0, s.timeScale ? s.timeScale(local) : 1) / fps;
    }
    return c;
  }
  /** The composition time (seconds, on a frame) when scene `name`'s clock reaches `clock`; see `clockAt()`. */
  timeAt(name: string, clock: number): number {
    const s = this.sceneNamed(name), fps = this.fps;
    let c = 0, f = Math.ceil(s.start * fps - 1e-9);
    for (; f / fps < s.end; f++) {
      c += Math.max(0, s.timeScale ? s.timeScale(f / fps - s.start) : 1) / fps;
      if (c >= clock - 1e-9) break;
    }
    return Math.min(f, Math.round(s.end * fps)) / fps;
  }
  private sceneNamed(name: string): SceneDef {
    const s = this.scenes.find((x) => x.name === name);
    if (!s) throw new Error(`anikit composition: no scene named "${name}" (scenes: ${this.scenes.map((x) => x.name).join(', ') || 'none'})`);
    return s;
  }

  /** Shake offset, flash and aberration at `t` (a pure function of time). */
  impactAt(t: number): { x: number; y: number; flash: string; flashAlpha: number; aberration: number; shock: [number, number, number, number, number] } {
    let x = 0, y = 0, fa = 0, fl = '', ab = 0;
    // The strongest live shockwave: [origin x, y, radius px, half-width px, strength px].
    const shock: [number, number, number, number, number] = [0.5, 0.5, 0, 0, 0];
    const diag = Math.hypot(this.width, this.height);
    this.impacts.forEach((s, i) => {
      if (t < s.t) return;
      if (s.shockwave) {
        const age = t - s.t, r = age * s.shockSpeed * diag;
        const amt = s.shockwave * Math.exp(-age * 1.6) * Math.min(1, age * 30);
        if (r < diag * 1.2 && Math.abs(amt) > 0.3 && Math.abs(amt) > Math.abs(shock[4])) {
          shock[0] = s.origin[0]; shock[1] = s.origin[1]; shock[2] = r; shock[3] = diag * 0.02 + r * 0.12; shock[4] = amt;
        }
      }
      const k = Math.exp(-(t - s.t) * s.decay);
      x += s.shake * k * Math.sin(t * 97 + i * 7.1);
      y += s.shake * k * Math.cos(t * 83 + i * 3.3);
      ab += s.aberration * k;
      if (s.flash) {
        const a = s.flashOpacity * Math.exp(-(t - s.t) * s.decay * 3);
        if (a > fa) { fa = a; fl = s.flash; }
      }
    });
    return { x, y, flash: fl, flashAlpha: fa, aberration: ab, shock };
  }

  get frames(): number {
    return Math.round(this.duration * this.fps);
  }

  /** When a scene stops running: its end, extended over the next scene's transition. */
  private runsUntil(i: number): number {
    const s = this.scenes[i], n = this.scenes[i + 1];
    return n?.transition && n.start < s.end + 1e-9 ? Math.max(s.end, n.start + n.transition.duration) : s.end;
  }
  /** Scenes running at `t` (two during a transition), oldest first. */
  scenesAt(t: number): SceneDef[] {
    const out = this.scenes.filter((s, i) => t >= s.start && t < this.runsUntil(i));
    if (!out.length && t >= this.duration - 1e-9 && this.scenes.length) out.push(this.scenes[this.scenes.length - 1]);
    return out.slice(-2);
  }
  /** The (newest) scene active at `t`. */
  sceneAt(t: number): SceneDef | undefined {
    const a = this.scenesAt(t);
    return a[a.length - 1];
  }

  /** Render frames deterministically, calling `onFrame` for each delivered frame. */
  async render(o: RenderOptions): Promise<void> {
    await ready;
    const wasManual = isManual();
    engine.manual(true);
    const { width: w, height: h, fps } = this;
    const ctx = o.canvas.getContext('2d') as Ctx2D;
    const mb = this.options.motionBlur;
    const samples = typeof mb === 'number' ? Math.max(1, mb | 0) : mb ? Math.max(1, mb.samples | 0) : 1;
    const shutter = typeof mb === 'object' ? mb.shutter ?? 0.5 : 0.5;
    const first = Math.round((o.from ?? 0) * fps);
    const last = Math.min(this.frames, Math.round((o.to ?? this.duration) * fps));
    const pre = first - Math.round((o.preroll ?? 0.5) * fps);
    const every = Math.max(1, o.every ?? 1);
    const fb = new FrameBuffer(w, h);
    const read = o.readPixels ?? ((c: Ctx2D) => c.getImageData(0, 0, w, h).data);
    const order = o.order ?? 'rgba';
    const dt = 1000 / fps;
    const entered = new Set<SceneDef>();
    let cue = 0;
    const make = o.createCanvas ?? defaultCanvas;
    const bufs: CanvasLike[] = [];
    const buf = (k: number) => (bufs[k] ??= make(w, h));
    const clocks = new Map<SceneDef, number>();
    const info = (t: number, frame: number, s: SceneDef, c: Ctx2D = ctx, step = 0): FrameInfo => ({
      ctx: c, t, frame, width: w, height: h, scene: s, local: t - s.start, progress: Math.min(1, Math.max(0, (t - s.start) / (s.end - s.start))), dt: step,
      clock: clocks.get(s) ?? 0,
    });
    const post = this.options.post;
    const postOn = !!post && Object.values(post).some((v) => v);
    /** Shake + flash on the finished frame (after any transition compositing). */
    const finish = (t: number) => {
      if (!this.impacts.length) return;
      const im = this.impactAt(t);
      if (Math.abs(im.x) + Math.abs(im.y) > 0.05) {
        const c = buf(3), x = c.getContext('2d') as Ctx2D;
        x.setTransform(1, 0, 0, 1, 0, 0);
        x.drawImage(o.canvas as unknown as CanvasImageSource, 0, 0, w, h);
        ctx.save();
        ctx.setTransform(1, 0, 0, 1, 0, 0);
        const sc = 1 + (Math.max(Math.abs(im.x), Math.abs(im.y)) * 2.2) / Math.min(w, h);
        ctx.translate(w / 2 + im.x, h / 2 + im.y);
        ctx.scale(sc, sc);
        ctx.drawImage(c as unknown as CanvasImageSource, -w / 2, -h / 2, w, h);
        ctx.restore();
      }
      if (im.flashAlpha > 0.005) {
        ctx.save();
        ctx.setTransform(1, 0, 0, 1, 0, 0);
        ctx.globalAlpha = im.flashAlpha;
        ctx.fillStyle = im.flash;
        ctx.fillRect(0, 0, w, h);
        ctx.restore();
      }
    };
    /** The film grade, once per output frame (not per motion-blur sample). */
    const num = (v: PostValue | undefined, t: number, d: number) => (v === undefined ? d : typeof v === 'function' ? v(t) : v);
    const col = (v: PostColor | undefined, t: number, d: [number, number, number]) => (v === undefined ? d : typeof v === 'function' ? v(t) : v);
    const grade = (f: number, t: number) => {
      const im = this.impacts.length ? this.impactAt(t) : null;
      const ab = num(post?.aberration, t, 0) + (im ? im.aberration : 0);
      const shock = im?.shock ?? [0.5, 0.5, 0, 0, 0];
      if (!postOn && ab <= 0 && !shock[4]) return;
      const p = post ?? {};
      const zc = p.zoomCenter ?? [0.5, 0.5];
      const vals = [num(p.bloomThreshold, t, 0.7), num(p.bloom, t, 0), num(p.bloomRadius, t, 24), ab, num(p.vignette, t, 0), num(p.grain, t, 0),
        num(p.streaks, t, 0), num(p.streakLength, t, 160), ...col(p.streakTint, t, [0.6, 0.75, 1]), num(p.zoomBlur, t, 0), zc[0], zc[1],
        num(p.exposure, t, 1), num(p.contrast, t, 1), num(p.saturation, t, 1), ...col(p.shadows, t, [0, 0, 0]), ...col(p.highlights, t, [0, 0, 0]),
        num(p.rays, t, 0), ...(typeof p.raysCenter === 'function' ? p.raysCenter(t) : p.raysCenter ?? [0.5, 0.4]), num(p.raysLength, t, 0.6), num(p.aa, t, 0), num(p.flares, t, 0), ...shock];
      const at = f64Buf(vals.length);
      F64.set(vals, at);
      fb.post(f, vals.length);
    };
    const bg = (c: Ctx2D) => {
      if (!this.options.background) return;
      c.fillStyle = this.options.background;
      c.fillRect(0, 0, w, h);
    };
    // Guard: meshes / instance buffers created inside draw() pile up in WebAssembly memory.
    let churn = 0, churnWarned = false;
    const paint = (t: number, f: number, act: SceneDef[]) => {
      const before = created.meshes + created.instances;
      paintFrame(t, f, act);
      const made = created.meshes + created.instances - before;
      churn = made > 0 ? churn + 1 : 0;
      if (churn >= 3 && !churnWarned) {
        churnWarned = true;
        warn(`composition: scene "${act.map((s) => s.name).join('" / "')}" creates ${made} mesh${made === 1 ? '' : 'es'} or instance buffers in every draw(). Create them once, outside draw(), and update them in place (instances.set(i, …)): each one stays in memory for the whole render.`);
      }
    };
    const paintFrame = (t: number, f: number, act: SceneDef[]) => {
      const [a, b] = act;
      if (!b || !b.transition) {
        bg(ctx);
        (b ?? a).draw(info(t, f, b ?? a));
        finish(t);
        return;
      }
      const ca = buf(0), cb = buf(1);
      const xa = ca.getContext('2d') as Ctx2D, xb = cb.getContext('2d') as Ctx2D;
      for (const x of [xa, xb]) { x.setTransform(1, 0, 0, 1, 0, 0); x.globalAlpha = 1; x.globalCompositeOperation = 'source-over'; x.clearRect(0, 0, w, h); bg(x); }
      a.draw(info(t, f, a, xa));
      b.draw(info(t, f, b, xb));
      const tr = b.transition;
      const p = Math.min(1, Math.max(0, (t - b.start) / tr.duration));
      ctx.save();
      ctx.setTransform(1, 0, 0, 1, 0, 0);
      bg(ctx);
      composite(ctx, ca, cb, p, w, h, tr, () => buf(2));
      ctx.restore();
      finish(t);
    };
    const enterAll = (t: number, f: number, act: SceneDef[]) => {
      for (const s of act) if (!entered.has(s)) { entered.add(s); s.enter?.(info(t, f, s)); }
    };
    const stepAll = (fi: (s: SceneDef) => FrameInfo, act: SceneDef[], d: number) => {
      for (const s of act) { clocks.set(s, (clocks.get(s) ?? 0) + d); s.step?.(fi(s)); }
    };
    const defSamples = samples;
    try {
      for (let f = 0; f < last; f++) {
        const t = f / fps;
        while (cue < this.cues.length && this.cues[cue].t <= t + 1e-9) this.cues[cue++].fn();
        const act = this.scenesAt(t);
        enterAll(t, f, act);
        // Timelines created this frame autoplay on a microtask: let them start.
        await null;
        const visible = f >= first && (f - first) % every === 0;
        const warm = f >= pre && f < first;
        // Everything below is a pure function of the frame index, so ticks —
        // and therefore simulations — are identical in every worker/slice.
        const lead = act[act.length - 1];
        const n = Math.max(1, (lead?.motionBlur ?? defSamples) | 0);
        const scale = lead?.timeScale ? Math.max(0, lead.timeScale(t - lead.start)) : 1;
        const sub = n > 1 ? (dt * shutter) / n : dt;
        const blur = n > 1 && visible;
        const tm: FrameTiming = { step: 0, draw: 0, read: 0, post: 0, passes: {} };
        for (const k in passTimes) delete passTimes[k];
        let c0 = performance.now(), c1 = c0;
        for (let k = 0; k < n; k++) {
          engine.tick(sub * scale);
          const ts = t + (k * sub) / 1000;
          stepAll((s) => info(ts, f, s, ctx, (sub * scale) / 1000), act, (sub * scale) / 1000);
          c1 = performance.now(); tm.step += c1 - c0; c0 = c1;
          if (!act.length) continue;
          if (blur) {
            paint(ts, f, act);
            c1 = performance.now(); tm.draw += c1 - c0; c0 = c1;
            fb.write(read(ctx, w, h));
            fb.accumulate();
            c1 = performance.now(); tm.read += c1 - c0; c0 = c1;
          } else if (warm || (visible && k === n - 1)) {
            paint(ts, f, act); // warm: same renders as drawn frames, so trails match
            c1 = performance.now(); tm.draw += c1 - c0; c0 = c1;
          }
        }
        if (n > 1) {
          const rest = (dt - sub * n) * scale;
          engine.tick(rest);
          // The closed part of the shutter still advances simulations.
          stepAll((s) => info(t + (sub * n) / 1000, f, s, ctx, rest / 1000), act, rest / 1000);
          c1 = performance.now(); tm.step += c1 - c0; c0 = c1;
        }
        if (!visible || !act.length) continue;
        if (blur) fb.resolve();
        else fb.write(read(ctx, w, h));
        c1 = performance.now(); tm.read += c1 - c0; c0 = c1;
        grade(f, t);
        tm.post = performance.now() - c0;
        Object.assign(tm.passes, passTimes);
        await o.onFrame({ index: f, t, ctx, timing: tm, rgba: () => fb.rgba(), i420: () => fb.i420(order) });
      }
    } finally {
      fb.free();
      if (!wasManual) engine.manual(false);
    }
  }
}

function defaultCanvas(w: number, h: number): CanvasLike {
  if (typeof OffscreenCanvas !== 'undefined') return new OffscreenCanvas(w, h) as unknown as CanvasLike;
  return Object.assign(document.createElement('canvas'), { width: w, height: h }) as unknown as CanvasLike;
}

const DEFAULT_EASE: Record<string, EasingInput> = { push: 'inOutQuart', whip: 'inOutExpo', slice: 'inOutQuart', zoom: 'inOutCubic', iris: 'inOutCubic', mask: 'inExpo', flash: 'linear', fade: 'inOutSine' };

/** Composite two scene buffers for a transition at raw progress `p`. */
function composite(ctx: Ctx2D, from: CanvasLike, to: CanvasLike, raw: number, w: number, h: number, tr: Transition, temp: () => CanvasLike): void {
  const type = tr.type ?? 'fade';
  const p = ease(tr.ease ?? (typeof type === 'string' ? DEFAULT_EASE[type] : 'inOutCubic') ?? 'inOutCubic', raw);
  const img = (c: CanvasLike, x = 0, y = 0, s = 1, a = 1) => {
    if (a <= 0) return;
    ctx.globalAlpha = a;
    if (s === 1) ctx.drawImage(c as unknown as CanvasImageSource, x, y, w, h);
    else ctx.drawImage(c as unknown as CanvasImageSource, x + (w - w * s) / 2, y + (h - h * s) / 2, w * s, h * s);
    ctx.globalAlpha = 1;
  };
  const [ox, oy] = tr.origin ?? [0.5, 0.5];
  const dir = tr.direction ?? 'left';
  const dx = dir === 'left' ? -1 : dir === 'right' ? 1 : 0, dy = dir === 'up' ? -1 : dir === 'down' ? 1 : 0;
  if (typeof type === 'function') return type(ctx, from, to, p, w, h);
  switch (type) {
    case 'fade':
      img(from); img(to, 0, 0, 1, p); return;
    case 'iris': {
      img(from);
      const cx = ox * w, cy = oy * h, r = p * Math.hypot(Math.max(cx, w - cx), Math.max(cy, h - cy));
      ctx.save(); ctx.beginPath(); ctx.arc(cx, cy, Math.max(0.01, r), 0, Math.PI * 2); ctx.clip(); img(to); ctx.restore();
      return;
    }
    case 'push': {
      const ux = dx * w * p, uy = dy * h * p;
      img(from, ux, uy); img(to, ux - dx * w, uy - dy * h); return;
    }
    case 'whip': {
      // A fast push with a directional smear proportional to speed.
      const speed = Math.sin(Math.PI * raw);
      const ux = dx * w * p, uy = dy * h * p, taps = 6;
      for (const [c, bx, by] of [[from, ux, uy], [to, ux - dx * w, uy - dy * h]] as Array<[CanvasLike, number, number]>) {
        img(c, bx, by);
        for (let k = 1; k <= taps; k++) {
          const o = (k / taps) * speed * 0.09;
          img(c, bx - dx * w * o, by - dy * h * o, 1, 0.5 / k);
        }
      }
      return;
    }
    case 'slice': {
      img(from);
      const n = tr.slices ?? 7, st = 0.5 / n, vertical = dy !== 0;
      for (let i = 0; i < n; i++) {
        const q = ease('outExpo', Math.min(1, Math.max(0, (raw * (1 + st * (n - 1)) - i * st))));
        if (q <= 0) continue;
        const sign = i % 2 ? 1 : -1;
        ctx.save(); ctx.beginPath();
        if (vertical) ctx.rect((i * w) / n, 0, w / n + 1, h); else ctx.rect(0, (i * h) / n, w, h / n + 1);
        ctx.clip();
        if (vertical) img(to, 0, sign * h * (1 - q)); else img(to, sign * w * (1 - q), 0);
        ctx.restore();
      }
      return;
    }
    case 'zoom':
      img(to, 0, 0, 0.7 + 0.3 * p, Math.min(1, p * 1.6));
      img(from, 0, 0, 1 + 2.5 * p * p, 1 - p);
      return;
    case 'flash': {
      img(raw < 0.5 ? from : to);
      ctx.globalAlpha = Math.pow(1 - Math.abs(2 * raw - 1), 1.5);
      ctx.fillStyle = tr.color ?? '#fff';
      ctx.fillRect(0, 0, w, h);
      ctx.globalAlpha = 1;
      return;
    }
    case 'mask': {
      // Reveal the next scene through a letterform that grows to fill the frame.
      img(from);
      if (raw >= 0.999) { img(to); return; }
      const t = temp(), x = t.getContext('2d') as Ctx2D;
      x.setTransform(1, 0, 0, 1, 0, 0);
      x.globalCompositeOperation = 'source-over';
      x.clearRect(0, 0, w, h);
      const size = h * 0.6 * Math.pow(60, p);
      x.font = tr.font ?? `900 ${size}px Display, Inter, system-ui`;
      x.textAlign = 'center';
      x.textBaseline = 'middle';
      x.fillStyle = '#000';
      x.fillText(tr.text ?? '●', ox * w, oy * h);
      x.globalCompositeOperation = 'source-in';
      x.drawImage(to as unknown as CanvasImageSource, 0, 0, w, h);
      x.globalCompositeOperation = 'source-over';
      ctx.drawImage(t as unknown as CanvasImageSource, 0, 0, w, h);
      return;
    }
  }
}

/** Create a composition. */
export function composition(options: CompositionOptions): Composition {
  return new Composition(options);
}

// ---------------------------------------------------------------------------
// Kinetic type for canvases
// ---------------------------------------------------------------------------

export interface TypeFxOptions {
  /** Font size in px (default 96). */
  size?: number;
  /** Font family (default "Display, Inter, system-ui"). */
  font?: string;
  weight?: number | string;
  italic?: boolean;
  color?: string;
  /** 'rise' (masked), 'wave', 'type' (typewriter), 'scramble', 'fade' (default 'rise'). */
  effect?: 'rise' | 'wave' | 'type' | 'scramble' | 'fade' | 'slam';
  /** When the effect starts (seconds, in the same clock as `t`). */
  start?: number;
  /** Current time. */
  t: number;
  /** Seconds between characters (default 0.03) and per character (default 0.7). */
  stagger?: number;
  duration?: number;
  ease?: EasingInput;
  align?: 'left' | 'center' | 'right';
  letterSpacing?: number;
}

const GLYPHS = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789#%&*+=?';

/** Draw text with a per-character animation (deterministic in `t`). */
export function typeFx(ctx: Ctx2D, text: string, x: number, y: number, o: TypeFxOptions): void {
  const size = o.size ?? 96;
  const fam = o.font ?? 'Display, Inter, system-ui';
  ctx.font = `${o.italic ? 'italic ' : ''}${o.weight ?? 900} ${size}px ${fam}`;
  ctx.textBaseline = 'alphabetic';
  ctx.textAlign = 'left';
  ctx.fillStyle = o.color ?? '#000';
  const slam = o.effect === 'slam';
  const st = o.stagger ?? (slam ? 0.035 : 0.03), dur = o.duration ?? (slam ? 0.32 : 0.7), start = o.start ?? 0, e = o.ease ?? 'outExpo';
  const ls = o.letterSpacing ?? 0;
  const widths = Array.from(text, (c) => ctx.measureText(c).width + ls);
  const total = widths.reduce((a, b) => a + b, 0);
  let cx = o.align === 'center' ? x - total / 2 : o.align === 'right' ? x - total : x;
  const effect = o.effect ?? 'rise';
  ctx.save();
  if (effect === 'rise') {
    ctx.beginPath();
    ctx.rect(cx - size, y - size * 1.02, total + size * 2, size * 1.3);
    ctx.clip();
  }
  let i = 0;
  for (const c of text) {
    const p = Math.min(1, Math.max(0, (o.t - start - i * st) / dur));
    const v = ease(e, p);
    let ch = c, dy = 0, alpha = 1;
    if (effect === 'rise') dy = (1 - v) * size * 1.1;
    else if (effect === 'wave') { dy = (1 - v) * size * 0.6 + Math.sin((o.t - start) * 6 - i * 0.5) * size * 0.04 * v; alpha = v; }
    else if (effect === 'fade') alpha = v;
    else if (effect === 'type') alpha = p > 0 ? 1 : 0;
    else if (effect === 'scramble' && p < 1 && c !== ' ') ch = p <= 0 ? '' : GLYPHS[(Math.floor(o.t * 30) + i * 7) % GLYPHS.length];
    if (effect === 'slam') {
      // Characters punch in from 3× scale.
      if (p > 0) {
        const sc = 1 + (1 - v) * 2.2;
        ctx.globalAlpha = Math.min(1, p * 3);
        ctx.save();
        ctx.translate(cx + widths[i] / 2, y - size * 0.36);
        ctx.scale(sc, sc);
        ctx.fillText(ch, -widths[i] / 2, size * 0.36);
        ctx.restore();
      }
    } else if (alpha > 0 && ch) {
      ctx.globalAlpha = alpha;
      ctx.fillText(ch, cx, y + dy);
    }
    cx += widths[i];
    i++;
  }
  ctx.restore();
}

// ---------------------------------------------------------------------------
// Browser export: WebCodecs + a built-in WebM muxer
// ---------------------------------------------------------------------------

const enc = new TextEncoder();
function sizeBytes(n: number): Uint8Array {
  // 8-byte EBML size (0x01 + 56-bit length).
  const b = new Uint8Array(8);
  b[0] = 0x01;
  let v = n;
  for (let i = 7; i >= 1; i--) { b[i] = v & 0xff; v = Math.floor(v / 256); }
  return b;
}
function uintBytes(n: number): Uint8Array {
  const out: number[] = [];
  let v = n;
  do { out.unshift(v & 0xff); v = Math.floor(v / 256); } while (v > 0);
  return Uint8Array.from(out);
}
function cat(parts: Uint8Array[]): Uint8Array {
  const n = parts.reduce((a, p) => a + p.length, 0), o = new Uint8Array(n);
  let at = 0;
  for (const p of parts) { o.set(p, at); at += p.length; }
  return o;
}
function el(id: number[], ...data: Uint8Array[]): Uint8Array {
  const body = cat(data);
  return cat([Uint8Array.from(id), sizeBytes(body.length), body]);
}
const u = (id: number[], n: number) => el(id, uintBytes(n));
const s = (id: number[], v: string) => el(id, enc.encode(v));
const f64 = (id: number[], v: number) => { const b = new Uint8Array(8); new DataView(b.buffer).setFloat64(0, v); return el(id, b); };

/** Mux encoded VP8/VP9 chunks into a WebM file. */
export function muxWebM(chunks: Array<{ data: Uint8Array; timestamp: number; key: boolean }>, o: { width: number; height: number; codec: 'V_VP9' | 'V_VP8'; duration: number }): Uint8Array {
  const header = el([0x1a, 0x45, 0xdf, 0xa3], u([0x42, 0x86], 1), u([0x42, 0xf7], 1), u([0x42, 0xf2], 4), u([0x42, 0xf3], 8), s([0x42, 0x82], 'webm'), u([0x42, 0x87], 2), u([0x42, 0x85], 2));
  const info = el([0x15, 0x49, 0xa9, 0x66], u([0x2a, 0xd7, 0xb1], 1_000_000), s([0x4d, 0x80], 'anikit'), s([0x57, 0x41], 'anikit'), f64([0x44, 0x89], o.duration * 1000));
  const tracks = el([0x16, 0x54, 0xae, 0x6b], el([0xae], u([0xd7], 1), u([0x73, 0xc5], 1), u([0x83], 1), s([0x86], o.codec), el([0xe0], u([0xb0], o.width), u([0xba], o.height))));
  const clusters: Uint8Array[] = [];
  let block: Uint8Array[] = [];
  let base = 0;
  const flush = () => { if (block.length) clusters.push(el([0x1f, 0x43, 0xb6, 0x75], u([0xe7], base), ...block)); block = []; };
  for (const c of chunks) {
    const ms = Math.round(c.timestamp / 1000);
    if (c.key || ms - base > 30000 || !block.length) { flush(); base = ms; }
    const head = new Uint8Array(4);
    head[0] = 0x81;
    new DataView(head.buffer).setInt16(1, ms - base);
    head[3] = c.key ? 0x80 : 0;
    block.push(el([0xa3], head, c.data));
  }
  flush();
  return cat([header, el([0x18, 0x53, 0x80, 0x67], info, tracks, ...clusters)]);
}

/**
 * Render a composition to a WebM (VP9) in the browser with WebCodecs.
 * Frame-exact and faster than real time on most machines.
 */
export async function exportWebM(comp: Composition, o: { canvas?: CanvasLike; bitrate?: number; codec?: 'vp9' | 'vp8'; from?: number; to?: number; onProgress?: (p: number) => void } = {}): Promise<Blob> {
  const VE = (globalThis as any).VideoEncoder, VF = (globalThis as any).VideoFrame;
  if (!VE || !VF) throw new Error('anikit exportWebM: WebCodecs is not available in this browser');
  const { width: w, height: h, fps } = comp;
  const canvas = o.canvas ?? (typeof OffscreenCanvas !== 'undefined' ? (new OffscreenCanvas(w, h) as unknown as CanvasLike) : Object.assign(document.createElement('canvas'), { width: w, height: h }));
  const chunks: Array<{ data: Uint8Array; timestamp: number; key: boolean }> = [];
  let error: unknown = null;
  const encoder = new VE({
    output: (chunk: any) => { const d = new Uint8Array(chunk.byteLength); chunk.copyTo(d); chunks.push({ data: d, timestamp: chunk.timestamp, key: chunk.type === 'key' }); },
    error: (e: unknown) => (error = e),
  });
  const vp9 = (o.codec ?? 'vp9') === 'vp9';
  encoder.configure({ codec: vp9 ? 'vp09.00.10.08' : 'vp8', width: w, height: h, bitrate: o.bitrate ?? 12_000_000, framerate: fps });
  const first = Math.round((o.from ?? 0) * fps), last = Math.round((o.to ?? comp.duration) * fps);
  await comp.render({
    canvas, from: o.from, to: o.to,
    onFrame: async (fr) => {
      if (error) throw error;
      const ts = Math.round(((fr.index - first) * 1e6) / fps);
      const frame = new VF(fr.i420(), { format: 'I420', codedWidth: w, codedHeight: h, timestamp: ts, duration: Math.round(1e6 / fps) });
      encoder.encode(frame, { keyFrame: (fr.index - first) % (fps * 2) === 0 });
      frame.close();
      if (encoder.encodeQueueSize > 8) await new Promise((r) => setTimeout(r, 0));
      o.onProgress?.((fr.index - first + 1) / (last - first));
    },
  });
  await encoder.flush();
  encoder.close();
  if (error) throw error;
  const bytes = muxWebM(chunks, { width: w, height: h, codec: vp9 ? 'V_VP9' : 'V_VP8', duration: (last - first) / fps });
  return new Blob([bytes as BlobPart], { type: 'video/webm' });
}
