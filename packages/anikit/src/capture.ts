/**
 * Capture: screenshots and frame-exact recordings of anything animated with
 * Anikit — canvases, DOM elements or the whole page.
 *
 * Recordings are deterministic by default: the engine is switched to manual
 * time and advanced exactly `1 / fps` per frame, so every frame is perfect no
 * matter how slow the machine is. GIFs are encoded by the WebAssembly core
 * (median-cut palettes, changed-region frames, LZW); WebM uses MediaRecorder.
 *
 * ```ts
 * const png = await screenshot('#card');                       // Blob
 * const rec = await record('#hero', { duration: 2, fps: 30 });  // GIF by default
 * rec.download('hero.gif');
 * const clip = await record(canvas, { duration: 3, format: 'webm' });
 * ```
 *
 * Headless / CI: see the `anikit-record` CLI (Playwright) for recording any
 * page frame-exactly to GIF, MP4, WebM or PNG frames.
 */
import { core, W, ready } from './wasm';
import { engine, isManual } from './core';
import { Playable } from './playable';

export type CaptureTarget = Element | HTMLCanvasElement | string | null | undefined;

export interface RasterOptions {
  /** Pixel ratio (default devicePixelRatio, capped at 2). */
  scale?: number;
  /** Background fill (default: transparent for PNG, white for GIF/JPEG). */
  background?: string;
}

function resolveTarget(t: CaptureTarget): Element {
  if (typeof t === 'string') {
    const el = document.querySelector(t);
    if (!el) throw new Error(`anikit capture: "${t}" matched no element`);
    return el;
  }
  return t ?? document.documentElement;
}

/** Inline computed styles into a deep clone so it renders standalone. */
function cloneWithStyles(src: Element): Element {
  const clone = src.cloneNode(false) as Element;
  if (src instanceof HTMLCanvasElement) {
    const img = document.createElement('img');
    try {
      img.src = src.toDataURL();
    } catch {
      /* tainted canvas */
    }
    const cs = getComputedStyle(src);
    img.setAttribute('style', cs.cssText || styleText(cs));
    return img;
  }
  if (src.nodeType === 1) {
    const cs = getComputedStyle(src);
    (clone as HTMLElement).setAttribute('style', cs.cssText || styleText(cs));
  }
  for (const child of Array.from(src.childNodes)) {
    if (child.nodeType === 1) clone.appendChild(cloneWithStyles(child as Element));
    else if (child.nodeType === 3) clone.appendChild(child.cloneNode());
  }
  return clone;
}

function styleText(cs: CSSStyleDeclaration): string {
  let s = '';
  for (let i = 0; i < cs.length; i++) {
    const k = cs[i];
    s += `${k}:${cs.getPropertyValue(k)};`;
  }
  return s;
}

function loadImage(url: string): Promise<HTMLImageElement> {
  return new Promise((res, rej) => {
    const img = new Image();
    img.onload = () => res(img);
    img.onerror = () => rej(new Error('anikit capture: could not rasterise the element (cross-origin images or fonts?)'));
    img.src = url;
  });
}

/** Render a canvas or DOM element (via SVG foreignObject) into a new canvas. */
export async function rasterize(target?: CaptureTarget, o: RasterOptions = {}): Promise<HTMLCanvasElement> {
  const el = resolveTarget(target);
  const scale = o.scale ?? Math.min(2, typeof devicePixelRatio === 'number' ? devicePixelRatio : 1);
  const r = el.getBoundingClientRect();
  const w = Math.max(1, Math.round((el === document.documentElement ? innerWidth : r.width) * scale));
  const h = Math.max(1, Math.round((el === document.documentElement ? innerHeight : r.height) * scale));
  const out = document.createElement('canvas');
  out.width = w;
  out.height = h;
  const ctx = out.getContext('2d')!;
  if (o.background) {
    ctx.fillStyle = o.background;
    ctx.fillRect(0, 0, w, h);
  }
  if (el instanceof HTMLCanvasElement) {
    ctx.drawImage(el, 0, 0, w, h);
    return out;
  }
  const clone = cloneWithStyles(el) as HTMLElement;
  clone.style.margin = '0';
  clone.style.position = 'relative';
  clone.style.left = clone.style.top = '0';
  clone.setAttribute('xmlns', 'http://www.w3.org/1999/xhtml');
  const cw = w / scale;
  const ch = h / scale;
  const xml = new XMLSerializer().serializeToString(clone);
  const svg =
    `<svg xmlns="http://www.w3.org/2000/svg" width="${cw}" height="${ch}">` +
    `<foreignObject x="0" y="0" width="100%" height="100%">${xml}</foreignObject></svg>`;
  const img = await loadImage('data:image/svg+xml;charset=utf-8,' + encodeURIComponent(svg));
  ctx.drawImage(img, 0, 0, w, h);
  return out;
}

/** A PNG (or WebP/JPEG) screenshot of a canvas, element or the page. */
export async function screenshot(
  target?: CaptureTarget,
  o: RasterOptions & { type?: 'image/png' | 'image/webp' | 'image/jpeg'; quality?: number } = {},
): Promise<Blob> {
  const c = await rasterize(target, o);
  return new Promise((res, rej) => c.toBlob((b) => (b ? res(b) : rej(new Error('anikit capture: toBlob failed'))), o.type ?? 'image/png', o.quality));
}

// ---------------------------------------------------------------------------
// GIF encoding (works in browsers and Node)
// ---------------------------------------------------------------------------

/** Incremental GIF encoder backed by the WebAssembly core. */
export class GifWriter {
  private id: number;
  private carry = 0;
  frames = 0;
  constructor(readonly width: number, readonly height: number, loop = 0) {
    this.id = core().ak_gif_new(width, height, loop);
  }
  /** Add an RGBA frame shown for `seconds` (fractional centiseconds are carried over). */
  add(rgba: ArrayLike<number> | Uint8ClampedArray, seconds: number): void {
    const w = core();
    if (rgba.length !== this.width * this.height * 4) {
      throw new Error(`anikit GifWriter: frame has ${rgba.length} bytes, expected ${this.width}×${this.height}×4 = ${this.width * this.height * 4}`);
    }
    const ptr = w.ak_gif_frame_ptr(this.id);
    new Uint8Array(W!.memory.buffer, ptr, this.width * this.height * 4).set(rgba as ArrayLike<number>);
    const cs = seconds * 100 + this.carry;
    const d = Math.max(1, Math.round(cs));
    this.carry = cs - d;
    w.ak_gif_add(this.id, d);
    this.frames++;
  }
  /** Finish and return the file bytes. */
  finish(): Uint8Array {
    const w = core();
    const len = w.ak_gif_finish(this.id);
    const bytes = new Uint8Array(W!.memory.buffer, w.ak_gif_ptr(this.id), len).slice();
    w.ak_gif_free(this.id);
    return bytes;
  }
}

/** Encode RGBA frames into an animated GIF in one call. */
export function encodeGif(frames: ArrayLike<ArrayLike<number>>, width: number, height: number, fps = 30, loop = 0): Uint8Array {
  const g = new GifWriter(width, height, loop);
  for (let i = 0; i < frames.length; i++) g.add(frames[i], 1 / fps);
  return g.finish();
}

// ---------------------------------------------------------------------------
// Recording
// ---------------------------------------------------------------------------

export interface RecordOptions extends RasterOptions {
  /** Seconds to record (default 2). */
  duration?: number;
  /** Frames per second (default 30). */
  fps?: number;
  /** 'gif' (default), 'webm', or 'frames' (PNG blobs). */
  format?: 'gif' | 'webm' | 'frames';
  /** Seek this animation/timeline per frame instead of advancing global time. */
  animation?: Playable;
  /** Record in real time (non-deterministic) instead of stepping engine time. */
  realtime?: boolean;
  /** GIF loop count (0 = forever). */
  loop?: number;
  /** Called with 0…1 as frames are captured. */
  onProgress?: (fraction: number) => void;
}

export interface Recording {
  /** The encoded file (GIF or WebM). */
  blob: Blob;
  /** Object URL of `blob` (revoke when done). */
  url: string;
  /** Number of frames captured. */
  frames: number;
  /** Frame width in pixels. */
  width: number;
  /** Frame height in pixels. */
  height: number;
  /** PNG blobs when `format: 'frames'`. */
  images?: Blob[];
  /** Trigger a download in the browser. */
  download(filename?: string): void;
}

const nextFrame = () => new Promise<void>((r) => requestAnimationFrame(() => r()));
const sleep = (ms: number) => new Promise<void>((r) => setTimeout(r, ms));

/** Record a canvas, element or the page into a GIF / WebM / PNG frames. */
export async function record(target?: CaptureTarget, o: RecordOptions = {}): Promise<Recording> {
  await ready;
  const fps = o.fps ?? 30;
  const n = Math.max(1, Math.round((o.duration ?? 2) * fps));
  const format = o.format ?? 'gif';
  const deterministic = !o.realtime;
  const wasManual = isManual();
  const bg = o.background ?? (format === 'frames' ? undefined : '#ffffff');
  if (deterministic) engine.manual(true);
  let gif: GifWriter | null = null;
  const images: Blob[] = [];
  let stream: { canvas: HTMLCanvasElement; ctx: CanvasRenderingContext2D; rec: MediaRecorder; chunks: Blob[]; track: any } | null = null;
  let w = 0;
  let h = 0;
  try {
    for (let i = 0; i < n; i++) {
      if (deterministic) {
        if (o.animation) o.animation.seek(i / fps);
        else engine.tick(i === 0 ? 0 : 1000 / fps);
      } else if (i > 0) {
        await sleep(1000 / fps);
        await nextFrame();
      }
      const frame = await rasterize(target, { ...o, background: bg });
      w = frame.width;
      h = frame.height;
      if (format === 'gif') {
        gif ??= new GifWriter(w, h, o.loop ?? 0);
        const data = frame.getContext('2d')!.getImageData(0, 0, w, h).data;
        gif.add(data, 1 / fps);
      } else if (format === 'frames') {
        images.push(await new Promise<Blob>((r) => frame.toBlob((b) => r(b!), 'image/png')));
      } else {
        if (!stream) {
          const canvas = document.createElement('canvas');
          canvas.width = w;
          canvas.height = h;
          const ctx = canvas.getContext('2d')!;
          const ms = (canvas as any).captureStream(0) as MediaStream;
          const track = ms.getVideoTracks()[0] as any;
          const rec = new MediaRecorder(ms, { mimeType: 'video/webm' });
          const chunks: Blob[] = [];
          rec.ondataavailable = (e) => e.data.size && chunks.push(e.data);
          rec.start();
          stream = { canvas, ctx, rec, chunks, track };
        }
        stream.ctx.drawImage(frame, 0, 0);
        stream.track.requestFrame?.();
        // MediaRecorder timestamps are wall-clock: pace frames.
        await sleep(1000 / fps);
      }
      o.onProgress?.((i + 1) / n);
    }
  } finally {
    if (deterministic && !wasManual) engine.manual(false);
  }
  let blob: Blob;
  if (format === 'gif') blob = new Blob([gif!.finish() as BlobPart], { type: 'image/gif' });
  else if (format === 'frames') blob = new Blob(images, { type: 'application/octet-stream' });
  else {
    const s = stream!;
    await new Promise<void>((r) => {
      s.rec.onstop = () => r();
      s.rec.stop();
    });
    blob = new Blob(s.chunks, { type: 'video/webm' });
  }
  const url = URL.createObjectURL(blob);
  return {
    blob,
    url,
    frames: n,
    width: w,
    height: h,
    images: format === 'frames' ? images : undefined,
    download(filename = format === 'gif' ? 'anikit.gif' : format === 'webm' ? 'anikit.webm' : 'frame.png') {
      const a = document.createElement('a');
      a.href = url;
      a.download = filename;
      document.body.appendChild(a);
      a.click();
      a.remove();
    },
  };
}
