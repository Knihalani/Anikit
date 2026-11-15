/**
 * `stagger()` — distribute delays (or any numeric value) across targets.
 * Computed in the core: 1D or 2D grid distance, any origin, eased spread.
 */
import { core, F64, views } from './wasm';
import { easingId, EasingInput, LINEAR } from './core';
import { warn, warningsOn, checkKeys, didYouMean } from './debug';
import type { OptionKeys } from './debug';

export interface StaggerOptions {
  /** Value added to every result (e.g. a base delay in seconds). */
  start?: number;
  /** Spread the whole distribution over this total instead of `each` per step. */
  total?: number;
  /** Origin: index, 'first' | 'last' | 'center' | 'edges' | 'random'. */
  from?: number | 'first' | 'last' | 'center' | 'edges' | 'random';
  /** `[columns, rows]` for 2D grid distances. */
  grid?: [number, number];
  /** With `grid`: measure distance along one axis only. */
  axis?: 'x' | 'y';
  /** Easing applied across the distribution. */
  ease?: EasingInput;
  /** Seed for `from: 'random'`. */
  seed?: number;
}

/** A function value: called per target with `(target, index, count)`. */
export type StaggerFn = ((target: unknown, index: number, count: number) => number) & { __stagger: true };

const FROM: Record<string, number> = { first: 0, last: 1, center: 2, edges: 3, random: 4 };
const STAGGER_KEYS: OptionKeys<StaggerOptions> = { start: 0, total: 0, from: 0, grid: 0, axis: 0, ease: 0, seed: 0 };

/** Compute stagger values for `count` items (seconds in, seconds out). */
export function staggerValues(count: number, each: number, o: StaggerOptions = {}): number[] {
  const w = core();
  const from = typeof o.from === 'number' ? 5 : FROM[o.from ?? 'first'] ?? 0;
  const fromValue = typeof o.from === 'number' ? o.from : o.from === 'random' ? (o.seed ?? 1) : 0;
  const ease = o.ease === undefined ? LINEAR : easingId(o.ease);
  const p = w.ak_stagger(
    count, each, o.total ?? NaN, o.start ?? 0, from, fromValue,
    o.grid ? o.grid[0] : 0, o.grid ? o.grid[1] : 0, o.axis === 'x' ? 1 : o.axis === 'y' ? 2 : 0, ease,
  ) >>> 3;
  views();
  return Array.from(F64.subarray(p, p + count));
}

/**
 * `delay: stagger(0.05)` → each target starts 50ms after the previous one.
 * Works for any numeric option or property value, e.g. `x: stagger(20)`.
 */
export function stagger(each = 0.1, options: StaggerOptions = {}): StaggerFn {
  if (typeof each === 'object' && each !== null) {
    // GSAP habit: `stagger({ each: 0.1, from: 'center' })`.
    const o = each as StaggerOptions & { each?: number; amount?: number };
    warn(`stagger() takes (each, options): write stagger(${o.each ?? 0.1}, { … }) instead of stagger({ each, … }).`);
    options = { ...o, total: o.total ?? o.amount };
    delete (options as any).each;
    delete (options as any).amount;
    each = o.each ?? 0.1;
  }
  if (warningsOn()) {
    checkKeys('stagger()', options, STAGGER_KEYS);
    if (typeof options.from === 'string' && !(options.from in FROM)) {
      warn(`stagger(): unknown from "${options.from}"; using "first". Use an index or one of: ${Object.keys(FROM).join(', ')}.${didYouMean(options.from, Object.keys(FROM))}`);
    }
  }
  let cacheN = -1;
  let cache: number[] = [];
  const fn = ((_t: unknown, i: number, n: number) => {
    if (n !== cacheN) {
      cache = staggerValues(n, each, options);
      cacheN = n;
    }
    return cache[i] ?? 0;
  }) as StaggerFn;
  fn.__stagger = true;
  return fn;
}
