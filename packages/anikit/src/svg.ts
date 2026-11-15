/**
 * SVG helpers: shape → path conversion, morph plans, motion-path lookups,
 * path measurement (all geometry runs in the core).
 */
import { core, writeStr, f64Buf, F64, readStr, views } from './wasm';
import { isElement } from './props';
import { r3 } from './values';

/** Anything that resolves to path data. */
export type PathSource = string | Element;

const PATH_CMD = /^\s*[mM]/;

/** Resolve a path string, a CSS selector, or an SVG shape element to path data. */
export function pathData(src: PathSource): string {
  if (typeof src === 'string') {
    if (PATH_CMD.test(src)) return src;
    if (typeof document !== 'undefined') {
      const el = document.querySelector(src);
      if (el) return shapeToPath(el);
    }
    return src;
  }
  return shapeToPath(src);
}

const num = (el: Element, a: string) => parseFloat(el.getAttribute(a) || '0') || 0;

/** Convert `<path>`, `<circle>`, `<ellipse>`, `<rect>`, `<line>`, `<polyline>`, `<polygon>` to path data. */
export function shapeToPath(el: Element): string {
  const tag = el.tagName.toLowerCase();
  switch (tag) {
    case 'path':
      return el.getAttribute('d') || '';
    case 'circle':
    case 'ellipse': {
      const cx = num(el, 'cx');
      const cy = num(el, 'cy');
      const rx = tag === 'circle' ? num(el, 'r') : num(el, 'rx');
      const ry = tag === 'circle' ? rx : num(el, 'ry');
      return ellipse(cx, cy, rx, ry);
    }
    case 'rect':
      return rect(num(el, 'x'), num(el, 'y'), num(el, 'width'), num(el, 'height'), num(el, 'rx'), num(el, 'ry'));
    case 'line':
      return `M${num(el, 'x1')} ${num(el, 'y1')}L${num(el, 'x2')} ${num(el, 'y2')}`;
    case 'polyline':
    case 'polygon': {
      const pts = (el.getAttribute('points') || '').trim().split(/[\s,]+/).map(parseFloat);
      let d = '';
      for (let i = 0; i + 1 < pts.length; i += 2) d += (i ? 'L' : 'M') + pts[i] + ' ' + pts[i + 1];
      return tag === 'polygon' ? d + 'Z' : d;
    }
    default:
      return el.getAttribute('d') || '';
  }
}

// ---------------------------------------------------------------------------
// Shape builders (handy morph targets)
// ---------------------------------------------------------------------------

export function ellipse(cx: number, cy: number, rx: number, ry = rx): string {
  return `M${cx + rx} ${cy}A${rx} ${ry} 0 1 1 ${cx - rx} ${cy}A${rx} ${ry} 0 1 1 ${cx + rx} ${cy}Z`;
}
export const circle = (cx: number, cy: number, r: number) => ellipse(cx, cy, r, r);
export function rect(x: number, y: number, w: number, h: number, rx = 0, ry = rx): string {
  rx = Math.min(rx, w / 2);
  ry = Math.min(ry || rx, h / 2);
  if (!rx || !ry) return `M${x} ${y}H${x + w}V${y + h}H${x}Z`;
  return (
    `M${x + rx} ${y}H${x + w - rx}A${rx} ${ry} 0 0 1 ${x + w} ${y + ry}V${y + h - ry}` +
    `A${rx} ${ry} 0 0 1 ${x + w - rx} ${y + h}H${x + rx}A${rx} ${ry} 0 0 1 ${x} ${y + h - ry}` +
    `V${y + ry}A${rx} ${ry} 0 0 1 ${x + rx} ${y}Z`
  );
}
export function polygon(points: Array<[number, number]>, closed = true): string {
  let d = '';
  points.forEach(([x, y], i) => (d += (i ? 'L' : 'M') + x + ' ' + y));
  return closed ? d + 'Z' : d;
}
/** Star (or regular polygon when inner === outer). */
export function star(cx: number, cy: number, points: number, outer: number, inner = outer / 2, rotation = 0): string {
  const pts: Array<[number, number]> = [];
  const step = Math.PI / points;
  for (let i = 0; i < points * 2; i++) {
    const r = i % 2 ? inner : outer;
    const a = -Math.PI / 2 + i * step + (rotation * Math.PI) / 180;
    pts.push([r3(cx + r * Math.cos(a)), r3(cy + r * Math.sin(a))]);
  }
  return polygon(pts);
}

// ---------------------------------------------------------------------------
// Measurement & sampling
// ---------------------------------------------------------------------------

/** Total length of a path. */
export function pathLength(src: PathSource): number {
  return core().ak_path_length(writeStr(pathData(src)));
}

/** `[minX, minY, maxX, maxY]` exact bounds of a path. */
export function pathBounds(src: PathSource): [number, number, number, number] {
  const p = core().ak_path_bounds(writeStr(pathData(src))) >>> 3;
  views();
  return [F64[p], F64[p + 1], F64[p + 2], F64[p + 3]];
}

/** Normalise any path to absolute cubic Bézier commands. */
export function normalizePath(src: PathSource, decimals = 3): string {
  return readStr(core().ak_path_normalize(writeStr(pathData(src)), decimals));
}

/** Reusable arc-length lookup for a path (O(1) sampling). Call `dispose()` when done. */
export class MotionPath {
  private id: number;
  readonly length: number;
  constructor(src: PathSource, samples = 0) {
    const w = core();
    this.id = w.ak_path_lut(writeStr(pathData(src)), samples);
    this.length = w.ak_path_lut_length(this.id);
  }
  /** Point and tangent angle (degrees) at progress `t ∈ [0, 1]` by distance. */
  at(t: number): { x: number; y: number; angle: number } {
    const p = core().ak_path_lut_sample(this.id, t) >>> 3;
    views();
    return { x: F64[p], y: F64[p + 1], angle: F64[p + 2] };
  }
  dispose(): void {
    core().ak_path_lut_free(this.id);
  }
}

// ---------------------------------------------------------------------------
// Morphing
// ---------------------------------------------------------------------------

export interface MorphPlan {
  /** `[nSub, (nSeg, closed) * nSub]` */
  template: number[];
  from: number[];
  to: number[];
}

/** Normalise two paths to a common structure (computed in the core). */
export function morphPlan(a: PathSource, b: PathSource): MorphPlan {
  const w = core();
  const la = writeStr(pathData(a));
  const lb = writeStr(pathData(b), la);
  w.ak_morph(la, lb);
  views();
  const at = w.ak_f64_buf(0) >>> 3;
  const ch = F64[at];
  const tl = F64[at + 1];
  const template = Array.from(F64.subarray(at + 2, at + 2 + tl));
  const from = Array.from(F64.subarray(at + 2 + tl, at + 2 + tl + ch));
  const to = Array.from(F64.subarray(at + 2 + tl + ch, at + 2 + tl + 2 * ch));
  return { template, from, to };
}

/** Serialise interpolated morph channels back to path data. */
export function writeMorph(template: number[], v: ArrayLike<number>, at = 0): string {
  let s = '';
  let i = at;
  const nsub = template[0];
  for (let k = 0; k < nsub; k++) {
    const nseg = template[1 + k * 2];
    const closed = template[2 + k * 2];
    s += 'M' + r3(v[i]) + ' ' + r3(v[i + 1]);
    i += 2;
    if (nseg) s += 'C';
    for (let j = 0; j < nseg * 6; j++) {
      s += (j ? ' ' : '') + r3(v[i++]);
    }
    if (closed) s += 'Z';
  }
  return s;
}

/** Interpolator between two shapes: `morph(a, b)(0.5)` → path data. */
export function morph(a: PathSource, b: PathSource): (t: number) => string {
  const plan = morphPlan(a, b);
  const tmp = new Float64Array(plan.from.length);
  return (t: number) => {
    for (let i = 0; i < tmp.length; i++) tmp[i] = plan.from[i] + (plan.to[i] - plan.from[i]) * t;
    return writeMorph(plan.template, tmp);
  };
}

/** Length of an SVG geometry element (falls back to the core for path data). */
export function elementLength(el: Element): number {
  const g = el as SVGGeometryElement;
  if (typeof g.getTotalLength === 'function') {
    try {
      const l = g.getTotalLength();
      if (l > 0) return l;
    } catch {
      /* not rendered */
    }
  }
  return pathLength(shapeToPath(el));
}

export { isElement };
export { f64Buf };
