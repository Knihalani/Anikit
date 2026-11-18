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

const HTML_NS = 'http://www.w3.org/1999/xhtml';
const ALWAYS = new Set([
  'width',
  'height',
  'inline-size',
  'block-size',
  'transform-origin',
  'perspective-origin',
  'border-top-width',
  'border-right-width',
  'border-bottom-width',
  'border-left-width',
  'border-block-start-width',
  'border-block-end-width',
  'border-inline-start-width',
  'border-inline-end-width',
  'outline-width',
  'column-rule-width',
]);
const FULL_ELEMENTS = new Set([
  'script',
  'style',
  'link',
  'meta',
  'base',
  'head',
  'title',
  'template',
  'noscript',
  'iframe',
  'frame',
  'object',
  'embed',
]);
const PROBE_EXCLUDED = new Set([
  'id',
  'class',
  'style',
  'src',
  'srcset',
  'srcdoc',
  'data',
  'poster',
  'background',
  'action',
  'formaction',
  'ping',
  'title',
  'alt',
  'placeholder',
  'value',
  'name',
  'for',
]);
type StyleCache = WeakMap<Element, { vals: string[]; text: string; key: string }>;
type StylePathCache = WeakMap<Element, boolean>;
type ProbeAttribute = { name: string; namespace: string | null; value: string };
type ProbeInfo = { key: string; namespace: string; localName: string; attributes: ProbeAttribute[] };
type DeferredStyle = {
  clone: Element;
  src: Element;
  names: string[];
  vals: string[];
  parentVals?: string[];
  key: string;
  cache?: StyleCache;
  pathCache?: StylePathCache;
};

const defaultStyles = new Map<string, (string | null)[]>();

function styleText(names: string[], vals: string[], include?: boolean[]): string {
  let s = '';
  for (let i = 0; i < names.length; i++) if (!include || include[i]) s += `${names[i]}:${vals[i]};`;
  return s;
}

function probeInfo(src: Element): ProbeInfo {
  const attributes = Array.from(src.attributes)
    .filter((attr) => {
      const name = attr.name.toLowerCase();
      return !PROBE_EXCLUDED.has(name) && !name.startsWith('on') && !name.startsWith('aria-') && !name.startsWith('data-');
    })
    .map((attr) => ({
      name: attr.name,
      namespace: attr.namespaceURI,
      value: attr.name.toLowerCase() === 'href' ? '' : attr.value,
    }));
  const keyAttributes = attributes
    .map(({ name, value }) => (name.toLowerCase() === 'href' ? name : `${name}=${value}`))
    .sort();
  const namespace = src.namespaceURI ?? '';
  return { key: `${namespace} ${src.localName} ${keyAttributes.join(' ')}`, namespace, localName: src.localName, attributes };
}

function probeDefaults(misses: Map<string, ProbeInfo>, names: string[]): { available: boolean; results: Map<string, (string | null)[]> } {
  const results = new Map<string, (string | null)[]>();
  if (!misses.size) return { available: true, results };
  let frame: HTMLIFrameElement | null = null;
  try {
    if (!document.documentElement) throw new Error('probe iframe has no document element');
    frame = document.createElement('iframe');
    frame.style.cssText =
      'position:fixed;left:-10000px;top:0;width:400px;height:400px;visibility:hidden;border:0;pointer-events:none';
    frame.setAttribute('aria-hidden', 'true');
    document.documentElement.appendChild(frame);
    const doc = frame.contentDocument;
    if (!doc) throw new Error('probe iframe has no document');
    doc.open();
    doc.write('<!DOCTYPE html><html><head></head><body></body></html>');
    doc.close();
    const a = doc.createElement('div');
    a.style.cssText =
      'direction:ltr;writing-mode:horizontal-tb;color:rgb(0,0,0);font-size:16px;font-weight:400;line-height:normal;width:300px;height:200px';
    const b = doc.createElement('div');
    b.style.cssText =
      'direction:rtl;writing-mode:vertical-rl;color:rgb(1,2,3);font-size:17px;font-weight:300;line-height:1.7;width:357px;height:211px';
    if (!doc.body) throw new Error('probe iframe has no body');
    doc.body.append(a, b);
    for (const [key, info] of misses) {
      const values: string[][] = [];
      for (const context of [a, b]) {
        const probe = doc.createElementNS(info.namespace || HTML_NS, info.localName);
        for (const attr of info.attributes) {
          if (attr.name.toLowerCase() === 'href') probe.setAttributeNS(attr.namespace, attr.name, '');
          else probe.setAttributeNS(attr.namespace, attr.name, attr.value);
        }
        context.appendChild(probe);
        const cs = doc.defaultView!.getComputedStyle(probe);
        const vals = new Array<string>(names.length);
        for (let i = 0; i < names.length; i++) vals[i] = cs.getPropertyValue(names[i]);
        values.push(vals);
      }
      const defaults = names.map((_, i) => (values[0][i] === values[1][i] ? values[0][i] : null));
      results.set(key, defaults);
      defaultStyles.set(key, defaults);
      if (defaultStyles.size > 512) defaultStyles.clear();
    }
    return { available: true, results };
  } catch {
    return { available: false, results };
  } finally {
    frame?.remove();
  }
}

function sameValues(a: string[], b: string[] | undefined): boolean {
  if (!b || a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return false;
  return true;
}

function textWithDefaults(names: string[], vals: string[], parentVals: string[] | undefined, defaults: (string | null)[]): string {
  const include = new Array<boolean>(names.length);
  for (let i = 0; i < names.length; i++) {
    include[i] =
      ALWAYS.has(names[i]) ||
      vals[i] !== parentVals?.[i] ||
      defaults[i] === null ||
      vals[i] !== defaults[i];
  }
  return styleText(names, vals, include);
}

/** Inline computed styles into a deep clone so it renders standalone. */
function cloneWithStyles(
  src: Element,
  names: string[],
  deferred: DeferredStyle[],
  misses: Map<string, ProbeInfo>,
  parentVals?: string[],
  parentChanged = false,
  isRoot = false,
  forceFullSubtree = false,
  forceFull = false,
  cache?: StyleCache,
  pathCache?: StylePathCache,
): Element {
  const clone = src.cloneNode(false) as Element;
  const cs = getComputedStyle(src);
  const vals = new Array<string>(names.length);
  for (let i = 0; i < names.length; i++) vals[i] = cs.getPropertyValue(names[i]);
  const cssText = cs.cssText;
  const namespaceFull = src.namespaceURI !== HTML_NS;
  const full =
    forceFull ||
    isRoot ||
    forceFullSubtree ||
    namespaceFull ||
    FULL_ELEMENTS.has(src.localName) ||
    src instanceof HTMLCanvasElement ||
    !!cssText;
  const info = full ? null : probeInfo(src);
  const key = info ? info.key : '';
  const previous = cache?.get(src);
  const same = sameValues(vals, previous?.vals) && previous?.key === key;
  const changed = parentChanged || !same || pathCache?.get(src) !== full;

  if (src instanceof HTMLCanvasElement) {
    const img = document.createElement('img');
    try {
      img.src = src.toDataURL();
    } catch {
      /* tainted canvas */
    }
    img.setAttribute('style', cssText || styleText(names, vals));
    return img;
  }

  let text: string | undefined;
  if (previous && same && !parentChanged && pathCache?.get(src) === full && (!cssText || previous.text === cssText)) text = previous.text;
  else if (cssText) text = cssText;
  else if (full) text = styleText(names, vals);
  else if (info) {
    const defaults = defaultStyles.get(info.key);
    if (defaults) text = textWithDefaults(names, vals, parentVals, defaults);
    else {
      misses.set(info.key, info);
      deferred.push({ clone, src, names, vals, parentVals, key: info.key, cache, pathCache });
    }
  }
  if (text !== undefined) {
    clone.setAttribute('style', text);
    cache?.set(src, { vals, text, key });
    pathCache?.set(src, full);
  }

  for (const child of Array.from(src.childNodes)) {
    if (child.nodeType === 1) {
      clone.appendChild(
        cloneWithStyles(
          child as Element,
          names,
          deferred,
          misses,
          vals,
          changed,
          false,
          forceFullSubtree || namespaceFull,
          forceFull,
          cache,
          pathCache,
        ),
      );
    } else if (child.nodeType === 3) clone.appendChild(child.cloneNode());
  }
  return clone;
}

function finishDeferred(deferred: DeferredStyle[], results: Map<string, (string | null)[]>, forceFull: boolean): void {
  for (const item of deferred) {
    const defaults = results.get(item.key) ?? defaultStyles.get(item.key) ?? item.names.map(() => null);
    const text = forceFull ? styleText(item.names, item.vals) : textWithDefaults(item.names, item.vals, item.parentVals, defaults);
    item.clone.setAttribute('style', text);
    item.cache?.set(item.src, { vals: item.vals, text, key: forceFull ? '' : item.key });
    item.pathCache?.set(item.src, forceFull);
  }
}

function loadImage(url: string): Promise<HTMLImageElement> {
  return new Promise((res, rej) => {
    const img = new Image();
    img.onload = () => res(img);
    img.onerror = () => rej(new Error('anikit capture: could not rasterise the element (cross-origin images or fonts?)'));
    img.src = url;
  });
}

interface RasterizeInternalOptions {
  names?: string[];
  cache?: StyleCache;
  pathCache?: StylePathCache;
  canvas?: HTMLCanvasElement;
  contextOptions?: CanvasRenderingContext2DSettings;
}

async function rasterizeInternal(
  target: CaptureTarget | undefined,
  o: RasterOptions,
  internal: RasterizeInternalOptions = {},
): Promise<HTMLCanvasElement> {
  const el = resolveTarget(target);
  const names = internal.names ?? Array.from(getComputedStyle(el));
  const scale = o.scale ?? Math.min(2, typeof devicePixelRatio === 'number' ? devicePixelRatio : 1);
  const r = el.getBoundingClientRect();
  const w = Math.max(1, Math.round((el === document.documentElement ? innerWidth : r.width) * scale));
  const h = Math.max(1, Math.round((el === document.documentElement ? innerHeight : r.height) * scale));
  const out = internal.canvas ?? document.createElement('canvas');
  const resized = out.width !== w || out.height !== h;
  if (resized) {
    out.width = w;
    out.height = h;
  }
  const ctx = out.getContext('2d', internal.contextOptions)!;
  if (!resized) ctx.clearRect(0, 0, w, h);
  if (o.background) {
    ctx.fillStyle = o.background;
    ctx.fillRect(0, 0, w, h);
  }
  if (el instanceof HTMLCanvasElement) {
    ctx.drawImage(el, 0, 0, w, h);
    return out;
  }
  const hasStyles = el.matches('style, link') || !!el.querySelector('style, link');
  const deferred: DeferredStyle[] = [];
  const misses = new Map<string, ProbeInfo>();
  const clone = cloneWithStyles(
    el,
    names,
    deferred,
    misses,
    undefined,
    false,
    true,
    false,
    hasStyles,
    internal.cache,
    internal.pathCache,
  ) as HTMLElement;
  if (misses.size) {
    const probe = probeDefaults(misses, names);
    finishDeferred(deferred, probe.results, !probe.available);
  }
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

/** Render a canvas or DOM element (via SVG foreignObject) into a new canvas. */
export async function rasterize(target?: CaptureTarget, o: RasterOptions = {}): Promise<HTMLCanvasElement> {
  return rasterizeInternal(target, o);
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
  const names = Array.from(getComputedStyle(resolveTarget(target)));
  const cache: StyleCache = new WeakMap();
  const pathCache: StylePathCache = new WeakMap();
  const frameCanvas = document.createElement('canvas');
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
      const frame = await rasterizeInternal(target, { ...o, background: bg }, {
        names,
        cache,
        pathCache,
        canvas: frameCanvas,
        contextOptions: format === 'gif' ? { willReadFrequently: true } : undefined,
      });
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
