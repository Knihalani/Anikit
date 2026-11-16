/**
 * Shaped easings in the spirit of GSAP's CustomBounce and CustomWiggle: pure
 * functions of progress you pass as `ease`.
 *
 * ```ts
 * const { ease, squash } = customBounce({ strength: 0.6, squash: 2 });
 * animate(ball, { y: [-200, 0] }, { duration: 1.6, ease });
 * animate(ball, { scaleY: [1, 0.6], scaleX: [1, 1.3] }, { duration: 1.6, ease: squash });
 * animate(bell, { rotate: 20 }, { duration: 1, ease: customWiggle(6, { type: 'easeOut' }) });
 * ```
 */
import type { EasingFn } from './core';
import { checkKeys, warn } from './debug';
import type { OptionKeys } from './debug';

/** Options of `customBounce()`. */
export interface CustomBounceOptions {
  /** Bounciness 0–1: each bounce lasts `strength` times the previous one (default 0.7). */
  strength?: number;
  /** Length of the squash pulse at each impact, as a fraction of a bounce (0 for none; default 1, typical 1–4). */
  squash?: number;
  /** Start at the ground (1) and bounce up off it first, instead of falling in from 0 (default false). */
  endAtStart?: boolean;
}
const BOUNCE_KEYS: OptionKeys<CustomBounceOptions> = { strength: 0, squash: 0, endAtStart: 0 };

/**
 * A bounce ease plus its companion `squash` ease (0 except a quick pulse to 1
 * at each impact, scaled by the impact speed) for squash-and-stretch.
 */
export function customBounce(options: CustomBounceOptions = {}): { ease: EasingFn; squash: EasingFn } {
  checkKeys('customBounce()', options, BOUNCE_KEYS);
  const s = Math.min(Math.max(options.strength ?? 0.7, 0), 0.99);
  if (options.strength !== undefined && (options.strength < 0 || options.strength >= 1)) warn(`customBounce(): strength is ${options.strength}; use a value from 0 to below 1. Clamped to ${s}.`);
  // Fall (1 time unit) then bounces of half-length a_i = s^i, until imperceptible.
  const halves: number[] = [];
  for (let a = s; a > 0.004 && halves.length < 30; a *= s) halves.push(a);
  const total = 1 + 2 * halves.reduce((x, y) => x + y, 0);
  const impacts = [1];
  for (const a of halves) impacts.push(impacts[impacts.length - 1] + 2 * a);
  const bounce: EasingFn = (p) => {
    if (p <= 0) return 0;
    if (p >= 1) return 1;
    const t = p * total;
    if (t < 1) return t * t;
    for (let i = 0; i < halves.length; i++) {
      const a = halves[i], u = t - impacts[i];
      if (u <= 2 * a) return 1 - (a * a - (u - a) * (u - a));
    }
    return 1;
  };
  const ease: EasingFn = options.endAtStart ? (p) => 1 - bounce(p) : bounce;
  const width = (options.squash ?? 1) * 0.12;
  const squash: EasingFn = (p) => {
    if (width <= 0) return 0;
    const t = p * total;
    let v = 0;
    for (let i = 0; i < impacts.length; i++) {
      const w = width * (i === 0 ? 1 : halves[i - 1] ?? 1);
      const d = Math.abs(t - impacts[i]);
      if (d < w) v = Math.max(v, (1 - d / w) * (i === 0 ? 1 : halves[i - 1]));
    }
    return v;
  };
  return { ease, squash };
}

/** Options of `customWiggle()`. */
export interface CustomWiggleOptions {
  /**
   * Amplitude envelope: `'easeOut'` (strong, then dying away; default),
   * `'easeInOut'` (swells and fades), `'anticipate'` (a pull back, then the
   * swing), `'uniform'` (constant) or `'random'` (irregular, seeded).
   */
  type?: 'easeOut' | 'easeInOut' | 'anticipate' | 'uniform' | 'random';
  /** Seed for `'random'` (default 1). */
  seed?: number;
}
const WIGGLE_KEYS: OptionKeys<CustomWiggleOptions> = { type: 0, seed: 0 };
const TYPES = ['easeOut', 'easeInOut', 'anticipate', 'uniform', 'random'];

/**
 * An ease that oscillates `wiggles` times around the start value, ending
 * there: animate to the peak offset (`rotate: 20`) and it swings ±20 and settles.
 */
export function customWiggle(wiggles = 6, options: CustomWiggleOptions = {}): EasingFn {
  checkKeys('customWiggle()', options, WIGGLE_KEYS);
  const type = options.type ?? 'easeOut';
  if (!TYPES.includes(type)) warn(`customWiggle(): unknown type "${type}"; use one of ${TYPES.join(', ')}.`);
  const n = Math.max(1, Math.round(wiggles));
  let seed = (options.seed ?? 1) * 9301 + 49297;
  const amps = Array.from({ length: 2 * n + 1 }, () => { seed = (seed * 9301 + 49297) % 233280; return 0.35 + 0.65 * (seed / 233280); });
  return (p) => {
    if (p <= 0 || p >= 1) return 0;
    const wave = Math.sin(2 * Math.PI * n * p);
    switch (type) {
      case 'uniform': return wave;
      case 'easeInOut': return wave * Math.sin(Math.PI * p);
      case 'anticipate': return -Math.sin(2 * Math.PI * n * p) * (1 - p) * (1 - p) * (p < 0.5 / n ? 0.4 : 1);
      case 'random': return wave * amps[Math.floor(p * 2 * n)] * (1 - p);
      default: return wave * (1 - p);
    }
  };
}
