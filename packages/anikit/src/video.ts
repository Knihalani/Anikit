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
import { core, W, ready } from './wasm';
import { engine, isManual } from './core';
import { ease } from './utils';
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
}
export interface SceneHooks {
  /** Once, when the scene starts (start tweens, timelines, simulations here). */
  enter?(f: FrameInfo): void;
  /** Every frame (and every motion-blur sample), even when not drawing. */
  step?(f: FrameInfo): void;
  /** Draw the frame. */
  draw(f: FrameInfo): void;
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
}

export interface RenderedFrame {
  index: number;
  t: number;
  ctx: Ctx2D;
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

  constructor(readonly options: CompositionOptions) {
    this.width = options.width;
    this.height = options.height;
    this.fps = options.fps ?? 60;
    this.duration = options.duration;
  }

  /** Add a scene from `start` to `end` seconds. */
  scene(name: string, start: number, end: number, hooks: SceneHooks): this {
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

  get frames(): number {
    return Math.round(this.duration * this.fps);
  }

  /** The scene active at `t` (scenes may not overlap; gaps draw nothing). */
  sceneAt(t: number): SceneDef | undefined {
    return this.scenes.find((s) => t >= s.start && t < s.end) ?? (t >= this.duration - 1e-9 ? this.scenes[this.scenes.length - 1] : undefined);
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
    let current: SceneDef | undefined;
    let cue = 0;
    const info = (t: number, frame: number, s: SceneDef): FrameInfo => ({
      ctx, t, frame, width: w, height: h, scene: s, local: t - s.start, progress: Math.min(1, Math.max(0, (t - s.start) / (s.end - s.start))),
    });
    const paint = (fi: FrameInfo) => {
      if (this.options.background) {
        ctx.fillStyle = this.options.background;
        ctx.fillRect(0, 0, w, h);
      }
      fi.scene.draw(fi);
    };
    try {
      for (let f = 0; f < last; f++) {
        const t = f / fps;
        while (cue < this.cues.length && this.cues[cue].t <= t + 1e-9) this.cues[cue++].fn();
        const s = this.sceneAt(t);
        if (s && s !== current) {
          current = s;
          s.enter?.(info(t, f, s));
        }
        // Timelines created this frame autoplay on a microtask: let them start.
        await null;
        const visible = f >= first && (f - first) % every === 0;
        const warm = f >= pre && f < first;
        if (!s || (!visible && !warm)) {
          engine.tick(dt);
          if (s) s.step?.(info(t, f, s));
          continue;
        }
        if (samples === 1 || warm) {
          engine.tick(dt);
          const fi = info(t, f, s);
          s.step?.(fi);
          paint(fi);
          if (warm) continue;
          let rgba: Uint8Array | null = null;
          await o.onFrame({
            index: f, t, ctx,
            rgba: () => rgba ?? (rgba = fb.write(read(ctx, w, h)).rgba()),
            i420: () => (rgba ? fb : fb.write(read(ctx, w, h))).i420(order),
          });
          continue;
        }
        // Motion blur: sub-frames across the shutter, averaged in WebAssembly.
        const sub = (dt * shutter) / samples;
        for (let k = 0; k < samples; k++) {
          engine.tick(sub);
          const fi = info(t + (k * sub) / 1000, f, s);
          s.step?.(fi);
          paint(fi);
          fb.write(read(ctx, w, h));
          fb.accumulate();
        }
        engine.tick(dt - sub * samples);
        fb.resolve();
        await o.onFrame({ index: f, t, ctx, rgba: () => fb.rgba(), i420: () => fb.i420(order) });
      }
    } finally {
      fb.free();
      if (!wasManual) engine.manual(false);
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
  effect?: 'rise' | 'wave' | 'type' | 'scramble' | 'fade';
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
  const st = o.stagger ?? 0.03, dur = o.duration ?? 0.7, start = o.start ?? 0, e = o.ease ?? 'outExpo';
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
    if (alpha > 0 && ch) {
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
