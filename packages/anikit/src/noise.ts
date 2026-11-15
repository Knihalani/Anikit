/**
 * Seeded simplex noise (computed in the core) for organic motion:
 * drifting particles, idle "breathing", procedural wiggles.
 *
 * ```ts
 * engine.onFrame((t) => set(el, { x: noise2(t / 1000, 0) * 20, y: noise2(0, t / 1000) * 20 }));
 * ```
 */
import { core, f64Buf, F64, views } from './wasm';

/** 2D simplex noise in ≈[-1, 1]. */
export const noise2 = (x: number, y: number, seed = 0): number => core().ak_noise2(seed, x, y);
/** 3D simplex noise in ≈[-1, 1] (use z as time for evolving fields). */
export const noise3 = (x: number, y: number, z: number, seed = 0): number => core().ak_noise3(seed, x, y, z);
/** Fractal noise: several octaves of 2D/3D simplex noise. */
export function fbm(x: number, y: number, z?: number, o: {
  /** Layers of detail (default 4). */
  octaves?: number;
  /** Frequency multiplier per octave (default 2). */
  lacunarity?: number;
  /** Amplitude multiplier per octave (default 0.5). */
  gain?: number;
  /** Noise seed (default 0). */
  seed?: number;
} = {}): number {
  const w = core();
  const [oc, la, ga, sd] = [o.octaves ?? 4, o.lacunarity ?? 2, o.gain ?? 0.5, o.seed ?? 0];
  return z === undefined ? w.ak_fbm2(sd, x, y, oc, la, ga) : w.ak_fbm3(sd, x, y, z, oc, la, ga);
}
/** Evaluate 3D noise for many points at once (`[x, y, z, x, y, z, …]`). */
export function noiseMany(xyz: ArrayLike<number>, seed = 0): Float64Array {
  const n = Math.floor(xyz.length / 3);
  const at = f64Buf(n * 3);
  for (let i = 0; i < n * 3; i++) F64[at + i] = xyz[i];
  const p = core().ak_noise3_bulk(seed, n) >>> 3;
  views();
  return F64.slice(p, p + n);
}
/** A smooth wandering value: `wander(t)` drifts in [-amplitude, amplitude]. */
export function wander(options: {
  /** How fast it drifts, per unit of `t` (default 1). */
  speed?: number;
  /** Output range ±amplitude (default 1). */
  amplitude?: number;
  /** Seed (default random). */
  seed?: number;
} = {}): (t: number) => number {
  const speed = options.speed ?? 1;
  const amp = options.amplitude ?? 1;
  const seed = options.seed ?? Math.floor(Math.random() * 1e6);
  return (t) => core().ak_noise2(seed, t * speed, 0.5) * amp;
}
