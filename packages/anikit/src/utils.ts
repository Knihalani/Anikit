/**
 * Utilities: evaluate easings, mix colours, and small math helpers that are
 * handy in `onUpdate` callbacks and function-based values.
 */
import { core, writeStr, F64, f64Buf, views } from './wasm';
import { easingId, EasingInput } from './core';
import { parseColor, COLOR_SPACES, ColorSpaceName, r3 } from './values';

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
