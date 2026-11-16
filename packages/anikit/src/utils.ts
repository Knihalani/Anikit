/**
 * Utilities: evaluate easings, mix colours, and small math helpers that are
 * handy in `onUpdate` callbacks and function-based values.
 */
import { core, writeStr, F64, f64Buf, views } from './wasm';
import { easingId, EasingInput } from './core';
import { parseColor, COLOR_SPACES, ColorSpaceName, r3 } from './values';
import { stagger, StaggerOptions } from './stagger';

export { cssEasing } from './compositor';
export { parseColor };

/** Evaluate an easing at `t`. */
export function ease(e: EasingInput, t: number): number {
  return core().ak_ease(easingId(e), t);
}

/** Get an easing as a plain function `(t) => number`. */
export function easing(e: EasingInput): (t: number) => number {
  const id = easingId(e);
  const w = core();
  return (t) => w.ak_ease(id, t);
}

/** Mix two CSS colours (default in OKLab) → `rgb()/rgba()` string. */
export function mixColor(a: string, b: string, t: number, space: ColorSpaceName = 'oklab'): string {
  const ca = parseColor(a);
  const cb = parseColor(b);
  if (!ca || !cb) throw new Error(`anikit: cannot parse colour "${!ca ? a : b}"`);
  const at = f64Buf(8);
  F64.set(ca, at);
  F64.set(cb, at + 4);
  const p = core().ak_color_mix(t, COLOR_SPACES[space]) >>> 3;
  views();
  const [r, g, bl, al] = [F64[p], F64[p + 1], F64[p + 2], F64[p + 3]];
  return al >= 1
    ? `rgb(${Math.round(r)},${Math.round(g)},${Math.round(bl)})`
    : `rgba(${Math.round(r)},${Math.round(g)},${Math.round(bl)},${r3(al)})`;
}

export const clamp = (v: number, min: number, max: number) => (v < min ? min : v > max ? max : v);
export const lerp = (a: number, b: number, t: number) => a + (b - a) * t;
/** Map `v` from `[inMin, inMax]` to `[outMin, outMax]`. */
export const mapRange = (v: number, inMin: number, inMax: number, outMin: number, outMax: number) =>
  outMin + ((v - inMin) / (inMax - inMin || 1)) * (outMax - outMin);
/** Wrap `v` into `[min, max)`. */
export const wrap = (v: number, min: number, max: number) => {
  const r = max - min;
  return ((((v - min) % r) + r) % r) + min;
};
/** Snap to a step or to the nearest value of a list. */
export const snap = (v: number, to: number | number[]) =>
  Array.isArray(to) ? to.reduce((a, b) => (Math.abs(b - v) < Math.abs(a - v) ? b : a)) : Math.round(v / to) * to;
/** Seeded random in `[min, max)` (mulberry32) — deterministic when `seed` is given. */
export function random(min = 0, max = 1, seed?: number): number {
  if (seed === undefined) return min + Math.random() * (max - min);
  let t = (seed + 0x6d2b79f5) | 0;
  t = Math.imul(t ^ (t >>> 15), t | 1);
  t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
  return min + (((t ^ (t >>> 14)) >>> 0) / 4294967296) * (max - min);
}

void writeStr;

// ---------------------------------------------------------------------------
// Interpolation & collection helpers
// ---------------------------------------------------------------------------

type Interp = number | string;

/**
 * Map an input through stops: `interpolate([0, 100, 200], [0, 1, 0])(50)` → 0.5.
 * Outputs may be numbers, colours (mixed in OKLab) or numbers with units
 * (`'10px'`, `'45deg'`). Options: `clamp` (default true), `ease` per segment.
 */
export function interpolate(
  input: number[],
  output: Interp[],
  options: {
    /** Hold the end values outside the input range (default true). */
    clamp?: boolean;
    /** Easing for every segment, or one per segment. */
    ease?: EasingInput | EasingInput[];
    /** Colour mixing space (default `'oklab'`). */
    colorSpace?: ColorSpaceName;
  } = {},
): (v: number) => Interp {
  if (input.length !== output.length || input.length < 2) throw new Error('anikit: interpolate needs ≥2 matching stops');
  const clampIt = options.clamp !== false;
  const eases = Array.isArray(options.ease) && typeof options.ease[0] !== 'number'
    ? (options.ease as EasingInput[]).map((e) => easing(e))
    : options.ease ? [easing(options.ease as EasingInput)] : [];
  const isColor = typeof output[0] === 'string' && parseColor(output[0] as string) !== null;
  const unit = typeof output[0] === 'string' && !isColor ? /[a-z%]+$/i.exec(output[0] as string)?.[0] ?? '' : '';
  const nums = isColor ? [] : output.map((o) => (typeof o === 'number' ? o : parseFloat(o)));
  return (v: number) => {
    let i = 0;
    while (i < input.length - 2 && v > input[i + 1]) i++;
    let t = (v - input[i]) / (input[i + 1] - input[i] || 1);
    if (clampIt) t = Math.min(1, Math.max(0, t));
    if (eases.length) t = (eases[i] ?? eases[0])(t);
    if (isColor) return mixColor(output[i] as string, output[i + 1] as string, t, options.colorSpace);
    const r = nums[i] + (nums[i + 1] - nums[i]) * t;
    return unit ? r3(r) + unit : r;
  };
}

/** Normalise `v` from `[min, max]` to `[0, 1]`. */
export const normalize = (v: number, min: number, max: number) => (v - min) / (max - min || 1);
/** Round to `decimals` places. */
export const roundTo = (v: number, decimals = 0) => {
  const f = 10 ** decimals;
  return Math.round(v * f) / f;
};
/** Compose functions left to right: `pipe(clampFn, snapFn)(v)`. */
export const pipe = <T>(...fns: Array<(v: T) => T>) => (v: T): T => fns.reduce((a, f) => f(a), v);
/** Shuffle an array in place (Fisher–Yates); returns it. */
export function shuffle<T>(arr: T[], seed?: number): T[] {
  let s = seed;
  for (let i = arr.length - 1; i > 0; i--) {
    const r = s === undefined ? Math.random() : random(0, 1, (s = (s * 1664525 + 1013904223) >>> 0));
    const j = Math.floor(r * (i + 1));
    [arr[i], arr[j]] = [arr[j], arr[i]];
  }
  return arr;
}
/** `[r, g, b, a]` (0–255, alpha 0–1) of any CSS colour. */
export function splitColor(c: string): [number, number, number, number] {
  const v = parseColor(c);
  if (!v) throw new Error(`anikit: cannot parse colour "${c}"`);
  return v;
}
/** Unit of a CSS value (`'12px'` → `'px'`). */
export const getUnit = (v: string | number) => (typeof v === 'number' ? '' : /[a-z%]+$/i.exec(v.trim())?.[0] ?? '');
/** Resolve selectors / lists / NodeLists into an array. */
export { toTargets as toArray } from './props';
/**
 * Like `stagger()` but for any value (GSAP's `distribute`): spread `amount`
 * across targets from `base`, e.g. `scale: distribute({ base: 0.5, amount: 1, from: 'center' })`.
 */
export function distribute(o: {
  /** Value of the first target (default 0). */
  base?: number;
  /** Total spread across all targets (default 1). */
  amount?: number;
  /** Step per target instead of `amount`. */
  each?: number;
  /** Origin, as in `stagger()`. */
  from?: StaggerOptions['from'];
  /** `[columns, rows]` for 2D distance. */
  grid?: [number, number];
  /** With `grid`: one axis only. */
  axis?: 'x' | 'y';
  /** Easing across the distribution. */
  ease?: EasingInput;
} = {}) {
  return stagger(o.each ?? 0, { start: o.base ?? 0, total: o.each === undefined ? o.amount ?? 1 : undefined, from: o.from, grid: o.grid, axis: o.axis, ease: o.ease });
}

/** Tracks a value's velocity (units/s) over the last ~100ms of samples. */
export class VelocityTracker {
  // Samples live in two plain arrays from `head` on (no pair per sample, no
  // shift(): add() runs on every pointer move).
  private ts: number[] = [];
  private vs: number[] = [];
  private head = 0;
  constructor(private window = 100) {}
  /** Record a value (timestamp defaults to now). */
  add(value: number, time = typeof performance !== 'undefined' ? performance.now() : Date.now()): void {
    const ts = this.ts;
    const vs = this.vs;
    ts.push(time);
    vs.push(value);
    const cutoff = time - this.window;
    let h = this.head;
    while (ts.length - h > 2 && ts[h] < cutoff) h++;
    if (h > 32 && h * 2 > ts.length) {
      ts.copyWithin(0, h);
      vs.copyWithin(0, h);
      ts.length = vs.length = ts.length - h;
      h = 0;
    }
    this.head = h;
  }
  get velocity(): number {
    const ts = this.ts;
    const h = this.head;
    const n = ts.length - 1;
    if (n - h < 1) return 0;
    const t0 = ts[h];
    const t1 = ts[n];
    return t1 > t0 ? ((this.vs[n] - this.vs[h]) / (t1 - t0)) * 1000 : 0;
  }
  reset(): void {
    this.ts.length = this.vs.length = 0;
    this.head = 0;
  }
}
