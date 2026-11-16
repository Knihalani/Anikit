/**
 * HTML in canvas: draw live DOM elements into a 2D canvas or a WebGL texture,
 * and run one-line shader effects (ripple, shatter, dissolve, curl, pixelate,
 * wave, glass) on real page content.
 *
 * Two paths:
 * - **native** — the WICG HTML-in-Canvas proposal (`<canvas layoutsubtree>` /
 *   `content="drawable"`, `drawElementImage`, `texElementImage2D` /
 *   `texElementSubImage2D`, the `paint` event, `requestPaint()`). Used when
 *   the browser has it and the element is a direct child of such a canvas:
 *   pixels are exact and the element stays live, interactive and accessible.
 * - **fallback** — works in every browser today: the subtree is cloned with
 *   computed styles inlined, same-origin images, canvases, video frames and
 *   web fonts embedded as data URLs, serialised into SVG `<foreignObject>`,
 *   decoded as an image and drawn. `live: true` re-rasterises (throttled)
 *   on DOM mutations and resizes; cached resources make re-rasters cheap.
 *
 * ```ts
 * const hc = htmlCanvas('#card', { target: gl, live: true });
 * await hc.ready;               // first raster done; hc.texture is filled
 * // in your render loop: bind hc.texture; it is re-uploaded only on change
 * htmlEffect('#hero', 'shatter', { duration: 1.2 });
 * ```
 */
import { animate, type Animation } from './animate';
import type { EasingInput } from './core';
import { track } from './context';
import { checkKeys, checkSeconds, warn, type OptionKeys } from './debug';

type GL = WebGLRenderingContext | WebGL2RenderingContext;

/** Options for {@link htmlCanvas}. */
export interface HtmlCanvasOptions {
  /** Where pixels go: a 2D canvas (drawn at 0,0, sized to the element), or a WebGL context (a texture is created and filled). Default: a new offscreen 2D canvas. */
  target?: HTMLCanvasElement | GL;
  /** Keep the image in sync with the element (mutations, resizes, font loads). Default true. */
  live?: boolean;
  /** Pixel scale of the raster (default `devicePixelRatio`, capped at 2). */
  scale?: number;
  /** Minimum ms between live re-rasters on the fallback path (default 50). */
  throttle?: number;
  /** Use the native HTML-in-Canvas API when available (default true). */
  native?: boolean;
  /** Called after every new image (and texture upload). */
  onUpdate?: (hc: HtmlCanvas) => void;
}

const HTML_CANVAS_KEYS: OptionKeys<HtmlCanvasOptions> = { target: 0, live: 0, scale: 0, throttle: 0, native: 0, onUpdate: 0 };

/** Handle returned by {@link htmlCanvas}. */
export interface HtmlCanvas {
  /** The element being drawn. */
  readonly element: Element;
  /** The 2D canvas holding the latest image (the native canvas on the native path). */
  readonly canvas: HTMLCanvasElement;
  /** The WebGL texture (when `target` is a WebGL context), else null. Re-uploaded only when the content changed. */
  readonly texture: WebGLTexture | null;
  /** True when the native HTML-in-Canvas API is drawing. */
  readonly native: boolean;
  /** Element size in CSS pixels at the last raster. */
  readonly width: number;
  /** Element height in CSS pixels at the last raster. */
  readonly height: number;
  /** Increments on every new image. Compare it to skip work when nothing changed. */
  readonly version: number;
  /** Resolves after the first image is ready. */
  readonly ready: Promise<HtmlCanvas>;
  /** Re-rasterise now (resolves when the new image is drawn). */
  refresh(): Promise<void>;
  /** Stop observing, delete the texture. */
  dispose(): void;
  /** Same as `dispose()` (for contexts). */
  revert(): void;
  /** Same as `dispose()`. */
  kill(): void;
}

/** True when the browser implements the native HTML-in-Canvas API (`drawElementImage`). */
export function htmlCanvasSupported(): boolean {
  if (typeof document === 'undefined' || typeof CanvasRenderingContext2D === 'undefined') return false;
  return typeof (CanvasRenderingContext2D.prototype as unknown as Record<string, unknown>).drawElementImage === 'function';
}

const isGL = (t: unknown): t is GL =>
  !!t && typeof (t as GL).texImage2D === 'function' && typeof (t as GL).createTexture === 'function';

const toEl = (e: Element | string): Element | null => (typeof e === 'string' ? document.querySelector(e) : e);

// ---------------------------------------------------------------------------
// Fallback rasteriser: DOM → SVG foreignObject → Image.

const dataCache = new Map<string, Promise<string | null>>();
let warnedCross = new Set<string>();

function sameOrigin(url: string): boolean {
  if (url.startsWith('data:') || url.startsWith('blob:')) return true;
  try {
    return new URL(url, location.href).origin === location.origin;
  } catch {
    return false;
  }
}

/** Fetch a same-origin URL as a data URL (cached). Cross-origin: null + one warning. */
function toDataURL(url: string): Promise<string | null> {
  if (url.startsWith('data:')) return Promise.resolve(url);
  const abs = new URL(url, location.href).href;
  let p = dataCache.get(abs);
  if (p) return p;
  if (!sameOrigin(abs)) {
    if (!warnedCross.has(abs)) {
      warnedCross.add(abs);
      warn(`htmlCanvas: cross-origin resource ${abs} was skipped (it would taint the canvas). Serve it from the same origin or as a data URL.`);
    }
    return Promise.resolve(null);
  }
  p = fetch(abs)
    .then((r) => (r.ok ? r.blob() : null))
    .then((b) => b && new Promise<string>((res, rej) => {
      const fr = new FileReader();
      fr.onload = () => res(fr.result as string);
      fr.onerror = rej;
      fr.readAsDataURL(b);
    }))
    .catch(() => null);
  dataCache.set(abs, p);
  return p;
}

const URL_RE = /url\((['"]?)([^'")]+)\1\)/g;

async function inlineUrls(css: string, base?: string): Promise<string> {
  const urls = new Set<string>();
  for (const m of css.matchAll(URL_RE)) if (!m[2].startsWith('data:') && !m[2].startsWith('#')) urls.add(m[2]);
  if (!urls.size) return css;
  const map = new Map<string, string | null>();
  await Promise.all([...urls].map(async (u) => map.set(u, await toDataURL(base ? new URL(u, base).href : u))));
  return css.replace(URL_RE, (all, _q, u: string) => {
    if (!map.has(u)) return all;
    const d = map.get(u);
    return d ? `url("${d}")` : 'none';
  });
}

let fontCss: Promise<string> | null = null;
let fontSheetCount = -1;

/** Every same-origin `@font-face` rule, with its sources embedded (cached until stylesheets change). */
function fontFaces(): Promise<string> {
  const n = document.styleSheets.length;
  if (fontCss && n === fontSheetCount) return fontCss;
  fontSheetCount = n;
  const jobs: Array<Promise<string>> = [];
  for (const sheet of Array.from(document.styleSheets)) {
    let rules: CSSRuleList;
    try {
      rules = sheet.cssRules;
    } catch {
      continue; // cross-origin stylesheet
    }
    for (const r of Array.from(rules)) {
      if (r.constructor.name === 'CSSFontFaceRule' || r.cssText.startsWith('@font-face')) jobs.push(inlineUrls(r.cssText, sheet.href ?? location.href));
    }
  }
  fontCss = Promise.all(jobs).then((a) => a.join('\n'));
  return fontCss;
}

const SKIP_PROPS = new Set(['transition', 'animation', 'will-change']);

function copyStyle(src: Element, dst: HTMLElement | SVGElement, root: boolean, jobs: Array<Promise<void>>): void {
  const cs = getComputedStyle(src);
  let css = '';
  for (let i = 0; i < cs.length; i++) {
    const p = cs[i];
    if (SKIP_PROPS.has(p) || p.startsWith('transition') || p.startsWith('animation')) continue;
    const v = cs.getPropertyValue(p);
    if (v) css += `${p}:${v};`;
  }
  if (root) css += 'margin:0;transform:none;translate:none;rotate:none;scale:none;position:relative;left:auto;top:auto;right:auto;bottom:auto;inset:auto;opacity:1;visibility:visible;';
  if (css.includes('url(')) {
    jobs.push(inlineUrls(css).then((c) => dst.setAttribute('style', c)));
  }
  dst.setAttribute('style', css);
}

function cloneNode(src: Node, root: boolean, jobs: Array<Promise<void>>): Node | null {
  if (src.nodeType === 3) return document.createTextNode(src.nodeValue ?? '');
  if (src.nodeType !== 1) return null;
  const el = src as Element;
  const tag = el.tagName.toLowerCase();
  if (tag === 'script' || tag === 'style' || tag === 'template' || tag === 'noscript') return null;
  if (tag === 'iframe') {
    const box = document.createElement('div');
    copyStyle(el, box, root, jobs);
    return box;
  }
  if (tag === 'canvas' || tag === 'video') {
    const img = document.createElement('img');
    copyStyle(el, img, root, jobs);
    try {
      const c = el as HTMLCanvasElement | HTMLVideoElement;
      let url = '';
      if (tag === 'canvas') url = (c as HTMLCanvasElement).toDataURL();
      else {
        const v = c as HTMLVideoElement;
        if (v.videoWidth) {
          const t = document.createElement('canvas');
          t.width = v.videoWidth;
          t.height = v.videoHeight;
          t.getContext('2d')!.drawImage(v, 0, 0);
          url = t.toDataURL();
        }
      }
      if (url) img.setAttribute('src', url);
    } catch {
      warn(`htmlCanvas: a cross-origin <${tag}> was skipped (it would taint the canvas).`);
    }
    return img;
  }
  const out = el.cloneNode(false) as Element;
  if (out instanceof HTMLElement || out instanceof SVGElement) copyStyle(el, out, root, jobs);
  out.removeAttribute('id');
  out.removeAttribute('class');
  if (tag === 'img') {
    const src2 = (el as HTMLImageElement).currentSrc || (el as HTMLImageElement).src;
    out.removeAttribute('srcset');
    if (src2) jobs.push(toDataURL(src2).then((d) => { if (d) out.setAttribute('src', d); else out.removeAttribute('src'); }));
  } else if (tag === 'input') {
    const i = el as HTMLInputElement;
    if (i.type === 'checkbox' || i.type === 'radio') {
      if (i.checked) out.setAttribute('checked', '');
      else out.removeAttribute('checked');
    } else out.setAttribute('value', i.value);
  } else if (tag === 'textarea') {
    out.textContent = (el as HTMLTextAreaElement).value;
    return out;
  } else if (tag === 'select') {
    const s = el as HTMLSelectElement;
    for (const o of Array.from(s.options)) {
      const c = o.cloneNode(true) as HTMLOptionElement;
      if (o.selected) c.setAttribute('selected', '');
      else c.removeAttribute('selected');
      out.appendChild(c);
    }
    return out;
  }
  const kids = (el as HTMLElement).shadowRoot?.childNodes ?? el.childNodes;
  for (const k of Array.from(kids)) {
    const c = cloneNode(k, false, jobs);
    if (c) out.appendChild(c);
  }
  return out;
}

/** Serialise `el` into an SVG data URL (fonts and same-origin resources embedded). */
export async function htmlToSvg(el: Element): Promise<{ url: string; width: number; height: number }> {
  const r = el.getBoundingClientRect();
  const w = Math.max(1, Math.ceil((el as HTMLElement).offsetWidth || r.width));
  const h = Math.max(1, Math.ceil((el as HTMLElement).offsetHeight || r.height));
  const jobs: Array<Promise<void>> = [];
  const clone = cloneNode(el, true, jobs) as Element;
  const fonts = await fontFaces();
  await Promise.all(jobs);
  clone.setAttribute('xmlns', 'http://www.w3.org/1999/xhtml');
  const xhtml = new XMLSerializer().serializeToString(clone);
  const style = fonts ? `<style>${fonts.replace(/]]>/g, '')}</style>` : '';
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}">${style}<foreignObject x="0" y="0" width="100%" height="100%">${xhtml}</foreignObject></svg>`;
  return { url: 'data:image/svg+xml;charset=utf-8,' + encodeURIComponent(svg), width: w, height: h };
}

// ---------------------------------------------------------------------------

class HtmlCanvasImpl implements HtmlCanvas {
  readonly element: Element;
  canvas: HTMLCanvasElement;
  texture: WebGLTexture | null = null;
  native = false;
  width = 0;
  height = 0;
  version = 0;
  ready: Promise<HtmlCanvas>;
  private gl: GL | null = null;
  private ctx: CanvasRenderingContext2D | null = null;
  private scale: number;
  private throttle: number;
  private onUpdate?: (hc: HtmlCanvas) => void;
  private mo: MutationObserver | null = null;
  private ro: ResizeObserver | null = null;
  private timer = 0;
  private last = 0;
  private busy: Promise<void> | null = null;
  private again = false;
  private dead = false;
  private img: HTMLImageElement | null = null;
  private paintFn: (() => void) | null = null;
  private resolveReady!: (h: HtmlCanvas) => void;
  private texW = 0;
  private live = true;
  private texH = 0;

  constructor(el: Element, o: HtmlCanvasOptions) {
    this.element = el;
    this.scale = o.scale ?? Math.min(2, (typeof devicePixelRatio === 'number' && devicePixelRatio) || 1);
    this.throttle = o.throttle ?? 50;
    this.onUpdate = o.onUpdate;
    this.ready = new Promise((r) => (this.resolveReady = r));
    const t = o.target;
    if (isGL(t)) {
      this.gl = t;
      this.texture = t.createTexture();
      t.bindTexture(t.TEXTURE_2D, this.texture);
      t.texParameteri(t.TEXTURE_2D, t.TEXTURE_MIN_FILTER, t.LINEAR);
      t.texParameteri(t.TEXTURE_2D, t.TEXTURE_MAG_FILTER, t.LINEAR);
      t.texParameteri(t.TEXTURE_2D, t.TEXTURE_WRAP_S, t.CLAMP_TO_EDGE);
      t.texParameteri(t.TEXTURE_2D, t.TEXTURE_WRAP_T, t.CLAMP_TO_EDGE);
    }
    const parent = el.parentElement;
    const nativeOk = o.native !== false && htmlCanvasSupported() && parent instanceof HTMLCanvasElement &&
      (parent.hasAttribute('layoutsubtree') || parent.getAttribute('content') === 'drawable') &&
      // WebGL: the element must belong to the GL canvas; Chrome 153's texElementImage2D needs WebGL 2 (RGBA8).
      (!this.gl || ((this.gl.canvas as Element) === parent &&
        (typeof (this.gl as unknown as Record<string, unknown>).texElementSubImage2D === 'function' ||
          (typeof WebGL2RenderingContext !== 'undefined' && this.gl instanceof WebGL2RenderingContext))));
    if (nativeOk) {
      this.native = true;
      this.canvas = parent as HTMLCanvasElement;
      el.setAttribute('drawable', '');
      if (!this.gl) {
        const c = t instanceof HTMLCanvasElement ? t : this.canvas;
        this.ctx = c.getContext('2d');
        this.canvas = c;
      }
      this.paintFn = () => this.paintNative();
      parent.addEventListener('paint', this.paintFn);
      this.live = o.live !== false;
      this.requestPaint();
    } else {
      this.canvas = t instanceof HTMLCanvasElement ? t : document.createElement('canvas');
      this.startFallback(o.live !== false);
    }
  }

  private startFallback(live: boolean) {
    const el = this.element;
    this.native = false;
    this.ctx = this.canvas.getContext('2d');
    if (live) {
      this.mo = new MutationObserver(() => this.schedule());
      this.mo.observe(el, { subtree: true, childList: true, attributes: true, characterData: true });
      if (typeof ResizeObserver !== 'undefined') {
        this.ro = new ResizeObserver(() => this.schedule());
        this.ro.observe(el);
      }
      document.fonts?.addEventListener?.('loadingdone', this.onFonts);
      el.addEventListener('input', this.onInput, true);
      el.addEventListener('change', this.onInput, true);
    }
    void this.refresh();
  }

  private onFonts = () => {
    fontCss = null;
    this.schedule();
  };
  private onInput = () => this.schedule();

  private requestPaint() {
    const p = this.element.parentElement as HTMLCanvasElement & { requestPaint?: () => void };
    p?.requestPaint?.();
  }

  private paintNative() {
    if (this.dead) return;
    const el = this.element as HTMLElement;
    const w = el.offsetWidth, h = el.offsetHeight;
    this.width = w;
    this.height = h;
    try {
      if (this.gl) {
        const gl = this.gl as GL & Record<string, (...a: unknown[]) => void>;
        gl.bindTexture(gl.TEXTURE_2D, this.texture);
        if (typeof gl.texElementSubImage2D === 'function') {
          const pw = Math.ceil(w * this.scale), ph = Math.ceil(h * this.scale);
          if (pw !== this.texW || ph !== this.texH) {
            gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, pw, ph, 0, gl.RGBA, gl.UNSIGNED_BYTE, null);
            this.texW = pw;
            this.texH = ph;
          }
          gl.texElementSubImage2D(gl.TEXTURE_2D, 0, 0, 0, el);
        } else {
          // Earlier Chrome shape (measured in Chrome 153): texElementImage2D(target, internalformat, element).
          gl.texElementImage2D(gl.TEXTURE_2D, 0x8058 /* RGBA8 */, el);
        }
      } else if (this.ctx) {
        const c = this.ctx.canvas;
        this.ctx.clearRect(0, 0, c.width, c.height);
        (this.ctx as unknown as { drawElementImage(e: Element, x: number, y: number, w: number, h: number): unknown })
          .drawElementImage(el, 0, 0, c.width, c.height);
      }
    } catch (e) {
      warn(`htmlCanvas: native draw failed (${(e as Error).message}); using the foreignObject fallback.`);
      if (this.paintFn) this.element.parentElement?.removeEventListener('paint', this.paintFn);
      this.paintFn = null;
      this.canvas = document.createElement('canvas');
      this.startFallback(this.live);
      return;
    }
    this.version++;
    this.onUpdate?.(this);
    this.resolveReady(this);
  }

  private schedule() {
    if (this.dead || this.timer) return;
    const wait = Math.max(0, this.throttle - (performance.now() - this.last));
    this.timer = setTimeout(() => {
      this.timer = 0;
      void this.refresh();
    }, wait) as unknown as number;
  }

  refresh(): Promise<void> {
    if (this.native) {
      this.requestPaint();
      return this.ready.then(() => undefined);
    }
    if (this.busy) {
      this.again = true;
      return this.busy;
    }
    this.busy = this.raster().finally(() => {
      this.busy = null;
      if (this.again && !this.dead) {
        this.again = false;
        this.schedule();
      }
    });
    return this.busy;
  }

  private async raster(): Promise<void> {
    if (this.dead) return;
    this.last = performance.now();
    const { url, width, height } = await htmlToSvg(this.element);
    if (this.dead) return;
    const img = this.img ?? (this.img = new Image());
    img.src = url;
    try {
      await img.decode();
    } catch {
      warn('htmlCanvas: the element could not be rasterised (SVG image failed to decode).');
      return;
    }
    if (this.dead) return;
    const s = this.scale, c = this.canvas;
    const pw = Math.ceil(width * s), ph = Math.ceil(height * s);
    if (c.width !== pw || c.height !== ph) {
      c.width = pw;
      c.height = ph;
    } else this.ctx!.clearRect(0, 0, pw, ph);
    this.ctx!.drawImage(img, 0, 0, pw, ph);
    this.width = width;
    this.height = height;
    this.upload();
    this.version++;
    this.onUpdate?.(this);
    this.resolveReady(this);
  }

  private upload() {
    const gl = this.gl;
    if (!gl || !this.texture) return;
    const prev = gl.getParameter(gl.TEXTURE_BINDING_2D) as WebGLTexture | null;
    gl.bindTexture(gl.TEXTURE_2D, this.texture);
    gl.pixelStorei(gl.UNPACK_PREMULTIPLY_ALPHA_WEBGL, true);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, this.canvas);
    gl.pixelStorei(gl.UNPACK_PREMULTIPLY_ALPHA_WEBGL, false);
    gl.bindTexture(gl.TEXTURE_2D, prev);
  }

  dispose() {
    if (this.dead) return;
    this.dead = true;
    if (this.timer) clearTimeout(this.timer);
    this.mo?.disconnect();
    this.ro?.disconnect();
    document.fonts?.removeEventListener?.('loadingdone', this.onFonts);
    this.element.removeEventListener('input', this.onInput, true);
    this.element.removeEventListener('change', this.onInput, true);
    if (this.paintFn) this.element.parentElement?.removeEventListener('paint', this.paintFn);
    if (this.gl && this.texture) this.gl.deleteTexture(this.texture);
    this.texture = null;
  }
  revert() {
    this.dispose();
  }
  kill() {
    this.dispose();
  }
}

/**
 * Draw a live DOM element into a canvas or a WebGL texture.
 *
 * ```ts
 * const hc = htmlCanvas('#card', { target: gl });
 * await hc.ready;
 * gl.bindTexture(gl.TEXTURE_2D, hc.texture); // updated whenever #card changes
 * ```
 */
export function htmlCanvas(element: Element | string, options: HtmlCanvasOptions = {}): HtmlCanvas {
  checkKeys('htmlCanvas()', options, HTML_CANVAS_KEYS);
  const el = toEl(element);
  if (!el) throw new Error(`htmlCanvas(): ${typeof element === 'string' ? `selector "${element}" matched nothing` : 'no element'}.`);
  if (options.target && !(options.target instanceof HTMLCanvasElement) && !isGL(options.target)) {
    warn('htmlCanvas(): `target` should be a <canvas> or a WebGL context; a new canvas is used instead.');
    options = { ...options, target: undefined };
  }
  return track(new HtmlCanvasImpl(el, options));
}

// ---------------------------------------------------------------------------
// Effects.

/** The effects {@link htmlEffect} knows. */
export type HtmlEffectName = 'ripple' | 'shatter' | 'dissolve' | 'curl' | 'pixelate' | 'wave' | 'glass';

/** Options for {@link htmlEffect}. */
export interface HtmlEffectOptions {
  /** Seconds (default 1.2). */
  duration?: number;
  /** Start delay in seconds (default 0). */
  delay?: number;
  /** Easing or spring for the progress (default: per effect). */
  ease?: EasingInput;
  /**
   * 'out': the element leaves through the effect and stays hidden (`revert()` shows it).
   * 'in': the effect plays backwards and reveals the element.
   * 'pulse': 0→1→0, the element is unchanged afterwards.
   * Default: 'out' for shatter/dissolve/curl, 'pulse' for the rest.
   */
  mode?: 'out' | 'in' | 'pulse';
  /** Effect strength multiplier (default 1). */
  intensity?: number;
  /** Effect origin in element-relative 0–1 coordinates (default [0.5, 0.5]; curl: [1, 1]). */
  origin?: [number, number];
  /** Extra room around the element for pieces that fly out, in px (default: shatter 400, curl 200, wave 40, else 0). */
  padding?: number;
  /** Raster scale (default `devicePixelRatio`, capped at 2). */
  scale?: number;
  /** Tint for dissolve edges / curl backside, CSS colour (default '#ffb347'). */
  color?: string;
  /** Called when the effect ends. */
  onComplete?: () => void;
}

const HTML_EFFECT_KEYS: OptionKeys<HtmlEffectOptions> = {
  duration: 0, delay: 0, ease: 0, mode: 0, intensity: 0, origin: 0, padding: 0, scale: 0, color: 0, onComplete: 0,
};

/** Controller returned by {@link htmlEffect}. */
export interface HtmlEffect {
  /** The overlay canvas (fixed-position, pointer-events: none). */
  readonly canvas: HTMLCanvasElement;
  /** Resolves when the effect has finished. */
  readonly finished: Promise<void>;
  /** Progress 0–1 (writable: set it to scrub; call `render()` after). */
  progress: number;
  /** Draw the current progress (called for you while the effect plays). */
  render(): void;
  /** Stop, remove the overlay and restore the element. */
  revert(): void;
  /** Stop and remove the overlay, leaving the element as the effect left it. */
  kill(): void;
}

const EFFECTS: HtmlEffectName[] = ['ripple', 'shatter', 'dissolve', 'curl', 'pixelate', 'wave', 'glass'];
const EFFECT_ID: Record<HtmlEffectName, number> = { ripple: 0, shatter: 1, dissolve: 2, curl: 3, pixelate: 4, wave: 5, glass: 6 };

const VS = `
precision highp float; precision highp int;
attribute vec2 a_uv; attribute vec2 a_c; attribute float a_r;
uniform vec2 u_size; uniform vec2 u_pad; uniform vec2 u_view; uniform float u_p; uniform float u_k; uniform int u_fx; uniform vec2 u_o; uniform float u_t;
varying vec2 v_uv; varying float v_a; varying float v_shade;
void main(){
  vec2 uv=a_uv; vec2 p=uv*u_size; float z=0.0; v_a=1.0; v_shade=1.0;
  float e=u_p;
  if(u_fx==1){ // shatter
    vec2 c=a_c*u_size; vec2 o=u_o*u_size; vec2 d=c-o; float l=length(d)+1.0;
    float t=max(0.0,e*1.4-l/length(u_size)*0.4);
    vec2 dir=d/l; float sp=(600.0+a_r*900.0)*u_k;
    vec2 q=p-c; float ang=(a_r-0.5)*12.0*t*u_k; float cs=cos(ang), sn=sin(ang);
    q=vec2(q.x*cs-q.y*sn,q.x*sn+q.y*cs)*(1.0-0.3*t);
    p=c+q+dir*sp*t+vec2(0.0,700.0*t*t);
    z=(a_r-0.3)*600.0*t*u_k; v_a=clamp(1.0-t*1.1,0.0,1.0);
  } else if(u_fx==5){ // wave
    float env=sin(3.14159*e)*u_k;
    p.y+=sin(uv.x*12.0-u_t*8.0)*18.0*env; p.x+=sin(uv.y*9.0-u_t*6.0)*6.0*env; z=sin(uv.x*12.0-u_t*8.0)*20.0*env;
  } else if(u_fx==3){ // curl: fold along a line moving from the origin corner
    vec2 o=u_o*u_size; vec2 dir=normalize(vec2(0.5)*u_size-o+0.001); float span=dot(u_size,abs(dir))+1.0;
    float r=max(20.0,0.08*length(u_size))*u_k; float lineD=-r*3.14159+e*(span+r*6.3);
    float d=dot(p-o,dir); float behind=lineD-d;
    if(behind>0.0){ float a=behind/r; vec2 base=p+dir*behind;
      if(a<3.14159){ p=base-dir*r*sin(a); z=r*(1.0-cos(a)); v_shade=0.6+0.4*cos(a);}
      else { p=base+dir*(behind-3.14159*r); z=2.0*r; v_shade=-1.0; } }
  }
  vec2 sc=p+u_pad; float persp=1000.0/(1000.0-z);
  vec2 ctr=u_pad+u_size*0.5; sc=ctr+(sc-ctr)*persp;
  gl_Position=vec4(sc/u_view*2.0-1.0,-z/4000.0,1.0); gl_Position.y=-gl_Position.y;
  v_uv=uv;
}`;

const FS = `
precision highp float; precision highp int;
uniform sampler2D u_tex; uniform float u_p; uniform float u_k; uniform int u_fx; uniform vec2 u_size; uniform vec2 u_o; uniform float u_t; uniform vec3 u_col;
varying vec2 v_uv; varying float v_a; varying float v_shade;
float h(vec2 p){return fract(sin(dot(p,vec2(127.1,311.7)))*43758.5453);}
float n(vec2 p){vec2 i=floor(p),f=fract(p);f=f*f*(3.0-2.0*f);return mix(mix(h(i),h(i+vec2(1,0)),f.x),mix(h(i+vec2(0,1)),h(i+1.0),f.x),f.y);}
void main(){
  vec2 uv=v_uv; float env=sin(3.14159*u_p)*u_k; vec4 c;
  if(u_fx==0){ vec2 d=(uv-u_o)*u_size; float l=length(d); float w=sin(l*0.08-u_t*14.0)*exp(-l*0.004)*env*14.0; uv+=d/(l+1.0)*w/u_size; c=texture2D(u_tex,uv); c.rgb+=w*0.01; }
  else if(u_fx==4){ float b=max(1.0,env*48.0); vec2 px=u_size/b; uv=(floor(uv*px)+0.5)/px; c=texture2D(u_tex,uv); }
  else if(u_fx==6){ vec2 q=uv*u_size*0.02; vec2 g=vec2(n(q+u_t)-n(q+vec2(3.1,0)+u_t),n(q+vec2(0,5.7)-u_t)-n(q))*0.06*env;
    c=vec4(texture2D(u_tex,uv+g*1.2).r,texture2D(u_tex,uv+g).g,texture2D(u_tex,uv+g*0.8).b,texture2D(u_tex,uv+g).a);
    c.rgb+=vec3(0.08)*env*(1.0-uv.y); }
  else if(u_fx==2){ float v=n(uv*u_size*0.03)*0.7+n(uv*u_size*0.12)*0.3; float th=u_p*1.15-0.05; c=texture2D(u_tex,uv);
    if(v<th) discard; float edge=1.0-smoothstep(0.0,0.06,v-th); c.rgb=mix(c.rgb,u_col*c.a,edge*step(0.001,u_p)); }
  else { c=texture2D(u_tex,uv); }
  if(v_shade<0.0){ c=vec4(mix(u_col,vec3(1.0),0.6)*c.a,c.a); } else c.rgb*=v_shade;
  gl_FragColor=c*v_a;
}`;

function compile(gl: WebGLRenderingContext, type: number, src: string): WebGLShader {
  const s = gl.createShader(type)!;
  gl.shaderSource(s, src);
  gl.compileShader(s);
  if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error('htmlEffect shader: ' + gl.getShaderInfoLog(s));
  return s;
}

function parseColor(c: string): [number, number, number] {
  const m = /^#?([0-9a-f]{6})$/i.exec(c.trim());
  if (m) {
    const n = parseInt(m[1], 16);
    return [(n >> 16) / 255, ((n >> 8) & 255) / 255, (n & 255) / 255];
  }
  const ctx = document.createElement('canvas').getContext('2d');
  if (!ctx) return [1, 0.7, 0.28];
  ctx.fillStyle = c;
  const s = ctx.fillStyle as string;
  return s.startsWith('#') ? parseColor(s) : [1, 0.7, 0.28];
}

class HtmlEffectImpl implements HtmlEffect {
  canvas: HTMLCanvasElement;
  finished: Promise<void>;
  progress = 0;
  private gl: WebGLRenderingContext | null;
  private hc: HtmlCanvas | null = null;
  private anim: Animation | null = null;
  private el: HTMLElement;
  private prevOpacity: string;
  private prevVisibility: string;
  private loc: Record<string, WebGLUniformLocation | null> = {};
  private count = 0;
  private fx: number;
  private o: HtmlEffectOptions;
  private pad: number;
  private rect = { x: 0, y: 0, w: 0, h: 0 };
  private t0 = 0;
  private done = false;
  private resolve!: () => void;
  private col: [number, number, number];
  private state = { p: 0 };
  private readonly mode: 'out' | 'in' | 'pulse';

  constructor(el: HTMLElement, name: HtmlEffectName, o: HtmlEffectOptions) {
    this.el = el;
    this.o = o;
    this.fx = EFFECT_ID[name];
    this.mode = o.mode ?? (name === 'shatter' || name === 'dissolve' || name === 'curl' ? 'out' : 'pulse');
    this.pad = o.padding ?? (name === 'shatter' ? 400 : name === 'curl' ? 200 : name === 'wave' ? 40 : 0);
    this.col = parseColor(o.color ?? '#ffb347');
    this.prevOpacity = el.style.opacity;
    this.prevVisibility = el.style.visibility;
    this.finished = new Promise((r) => (this.resolve = r));
    const c = (this.canvas = document.createElement('canvas'));
    c.setAttribute('aria-hidden', 'true');
    c.style.cssText = 'position:fixed;pointer-events:none;z-index:2147483646;left:0;top:0';
    this.gl = c.getContext('webgl', { premultipliedAlpha: true, alpha: true, antialias: true });
    if (!this.gl) {
      warn('htmlEffect(): WebGL is unavailable; the effect is skipped.');
      this.finish();
      return;
    }
    this.setup(name);
  }

  private setup(name: HtmlEffectName) {
    const gl = this.gl!;
    const prog = gl.createProgram()!;
    gl.attachShader(prog, compile(gl, gl.VERTEX_SHADER, VS));
    gl.attachShader(prog, compile(gl, gl.FRAGMENT_SHADER, FS));
    gl.linkProgram(prog);
    if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) throw new Error('htmlEffect shader: ' + gl.getProgramInfoLog(prog));
    gl.useProgram(prog);
    for (const u of ['u_size', 'u_pad', 'u_view', 'u_p', 'u_k', 'u_fx', 'u_o', 'u_t', 'u_col', 'u_tex']) this.loc[u] = gl.getUniformLocation(prog, u);
    // A grid of independent triangles: uv, triangle centroid, random.
    const N = name === 'shatter' ? 24 : name === 'curl' || name === 'wave' ? 48 : 1;
    const data = new Float32Array(N * N * 6 * 5);
    let k = 0, seed = 1234567;
    const rnd = () => ((seed = (Math.imul(seed ^ (seed >>> 15), 0x2c1b3c6d) + 0x6d2b79f5) >>> 0) / 4294967296);
    const jit = (i: number, j: number) => (i > 0 && i < N && j > 0 && j < N && name === 'shatter' ? (Math.sin(i * 12.9898 + j * 78.233) * 43758.5453 % 1) * 0.35 / N : 0);
    for (let j = 0; j < N; j++) for (let i = 0; i < N; i++) {
      const P = (a: number, b: number): [number, number] => [(i + a) / N + jit(i + a, j + b), (j + b) / N + jit(j + b, i + a)];
      const p00 = P(0, 0), p10 = P(1, 0), p01 = P(0, 1), p11 = P(1, 1);
      for (const tri of [[p00, p10, p11], [p00, p11, p01]]) {
        const cx = (tri[0][0] + tri[1][0] + tri[2][0]) / 3, cy = (tri[0][1] + tri[1][1] + tri[2][1]) / 3, r = rnd();
        for (const v of tri) { data[k++] = v[0]; data[k++] = v[1]; data[k++] = cx; data[k++] = cy; data[k++] = r; }
      }
    }
    this.count = k / 5;
    const buf = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, buf);
    gl.bufferData(gl.ARRAY_BUFFER, data, gl.STATIC_DRAW);
    const attr = (n: string, size: number, off: number) => {
      const l = gl.getAttribLocation(prog, n);
      if (l < 0) return;
      gl.enableVertexAttribArray(l);
      gl.vertexAttribPointer(l, size, gl.FLOAT, false, 20, off * 4);
    };
    attr('a_uv', 2, 0);
    attr('a_c', 2, 2);
    attr('a_r', 1, 4);
    gl.enable(gl.BLEND);
    gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
    gl.enable(gl.DEPTH_TEST);
    gl.uniform1i(this.loc.u_tex, 0);
    gl.uniform1i(this.loc.u_fx, this.fx);
    gl.uniform1f(this.loc.u_k, this.o.intensity ?? 1);
    const origin = this.o.origin ?? (this.fx === 3 ? [1, 1] : [0.5, 0.5]);
    gl.uniform2f(this.loc.u_o, origin[0], origin[1]);
    gl.uniform3f(this.loc.u_col, this.col[0], this.col[1], this.col[2]);
  }

  start(): void {
    if (this.done) return;
    this.hc = htmlCanvas(this.el, { target: this.gl!, live: true, scale: this.o.scale, onUpdate: () => this.render() });
    this.hc.ready.then(() => {
      if (this.done) return;
      this.place();
      document.body.appendChild(this.canvas);
      // The real element stays in place (opacity 0) so it keeps receiving pointer events.
      this.el.style.opacity = '0';
      if (this.mode === 'in') this.el.style.visibility = '';
      this.t0 = performance.now();
      const from = this.mode === 'in' ? 1 : 0, to = this.mode === 'in' ? 0 : 1;
      this.state.p = from;
      this.progress = from;
      this.render();
      this.anim = animate(this.state, { p: to }, {
        duration: this.o.duration ?? 1.2,
        delay: this.o.delay ?? 0,
        ease: this.o.ease ?? (this.mode === 'pulse' ? 'linear' : this.fx === 1 ? 'linear' : 'inOutSine'),
        onUpdate: this.tick,
        onComplete: () => this.complete(),
      });
    });
  }

  private tick = () => {
    this.progress = this.state.p;
    this.render();
  };

  private place() {
    const r = this.el.getBoundingClientRect();
    const pad = this.pad, s = this.o.scale ?? Math.min(2, devicePixelRatio || 1);
    this.rect.x = r.left;
    this.rect.y = r.top;
    this.rect.w = r.width;
    this.rect.h = r.height;
    const W = r.width + pad * 2, H = r.height + pad * 2;
    const c = this.canvas;
    c.style.left = `${r.left - pad}px`;
    c.style.top = `${r.top - pad}px`;
    c.style.width = `${W}px`;
    c.style.height = `${H}px`;
    const pw = Math.ceil(W * s), ph = Math.ceil(H * s);
    if (c.width !== pw || c.height !== ph) {
      c.width = pw;
      c.height = ph;
    }
  }

  render(): void {
    const gl = this.gl;
    if (!gl || !this.hc || this.done) return;
    const L = this.loc, pad = this.pad, r = this.rect;
    gl.viewport(0, 0, this.canvas.width, this.canvas.height);
    gl.clearColor(0, 0, 0, 0);
    gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, this.hc.texture);
    gl.uniform2f(L.u_size, r.w, r.h);
    gl.uniform2f(L.u_pad, pad, pad);
    gl.uniform2f(L.u_view, r.w + pad * 2, r.h + pad * 2);
    gl.uniform1f(L.u_p, this.progress);
    gl.uniform1f(L.u_t, (performance.now() - this.t0) / 1000);
    gl.drawArrays(gl.TRIANGLES, 0, this.count);
  }

  private complete() {
    if (this.mode === 'out') this.el.style.visibility = 'hidden';
    this.el.style.opacity = this.prevOpacity;
    this.finish();
    this.o.onComplete?.();
  }

  private finish() {
    if (this.done) return;
    this.done = true;
    this.anim?.kill();
    this.hc?.dispose();
    this.canvas.remove();
    const ext = this.gl?.getExtension('WEBGL_lose_context');
    ext?.loseContext();
    this.resolve();
  }

  revert(): void {
    this.finish();
    this.el.style.opacity = this.prevOpacity;
    this.el.style.visibility = this.prevVisibility;
  }

  kill(): void {
    this.finish();
    if (this.el.style.opacity === '0') this.el.style.opacity = this.prevOpacity;
  }
}

/**
 * One-line shader effects on real DOM content. The element is rasterised
 * into a WebGL overlay, the effect is animated by anikit, and the real element
 * is restored afterwards ('out' effects leave it `visibility: hidden` until
 * `revert()`). While the effect plays the element stays in place at opacity 0,
 * so clicks and hovers still reach it.
 *
 * ```ts
 * await htmlEffect('#card', 'shatter', { duration: 1.4 }).finished;
 * htmlEffect('.hero', 'ripple', { origin: [0.3, 0.6] });
 * htmlEffect('#modal', 'dissolve', { mode: 'in', color: '#7cf' });
 * ```
 */
export function htmlEffect(element: Element | string, effect: HtmlEffectName, options: HtmlEffectOptions = {}): HtmlEffect {
  checkKeys('htmlEffect()', options, HTML_EFFECT_KEYS);
  checkSeconds('htmlEffect()', 'duration', options.duration);
  checkSeconds('htmlEffect()', 'delay', options.delay);
  let name = effect;
  if (!EFFECTS.includes(name)) {
    warn(`htmlEffect(): unknown effect "${effect}"; using "ripple". Effects: ${EFFECTS.join(', ')}.`);
    name = 'ripple';
  }
  const el = toEl(element) as HTMLElement | null;
  if (!el) throw new Error(`htmlEffect(): ${typeof element === 'string' ? `selector "${element}" matched nothing` : 'no element'}.`);
  const fx = new HtmlEffectImpl(el, name, options);
  fx.start();
  return track(fx);
}

/** @internal test hook: forget cached resources and warnings. */
export function _resetHtmlCanvasCache(): void {
  dataCache.clear();
  fontCss = null;
  warnedCross = new Set();
}
